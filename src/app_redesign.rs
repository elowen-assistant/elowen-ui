use gloo_timers::future::TimeoutFuture;
use leptos::{ev, html, prelude::*, task::spawn_local};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource, MessageEvent};

use crate::{
    api::{
        create_job, create_thread, dispatch_thread_message, events_url, fetch_auth_session,
        fetch_job, fetch_jobs, fetch_thread, fetch_threads, login as login_session,
        logout as logout_session, promote_job_note, resolve_approval, send_thread_chat_message,
    },
    format::{
        approval_status_note, execution_intent_label, format_json_value, format_string_list,
        message_execution_draft, message_mode_badge, message_mode_class, message_result_details,
        report_array_strings, report_diff_stat, report_last_message, report_status_label,
        status_badge_class,
    },
    models::*,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavMode {
    Chats,
    Jobs,
    Details,
}

fn connect_ui_event_stream(handles: UiEventSyncHandles) {
    let Ok(source) = EventSource::new(&events_url()) else {
        handles.set_realtime_status.set(RealtimeStatus::Degraded);
        handles
            .set_status_text
            .set("Realtime stream unavailable. Polling fallback remains active.".to_string());
        return;
    };

    let on_open = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        handles.set_realtime_status.set(RealtimeStatus::Connected);
    }));
    source.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    let on_message =
        Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |message: MessageEvent| {
            let Some(data) = message.data().as_string() else {
                return;
            };
            let Ok(ui_event) = serde_json::from_str::<UiEvent>(&data) else {
                return;
            };

            spawn_local(async move {
                if let Err(error) = refresh_for_ui_event(ui_event, handles).await {
                    if is_auth_error(&error) {
                        handles.set_auth_session.set(Some(AuthSessionStatus {
                            enabled: true,
                            authenticated: false,
                            operator_label: None,
                        }));
                        handles.set_selected_thread.set(None);
                        handles.set_selected_job_detail.set(None);
                        handles
                            .set_status_text
                            .set("Session expired. Sign in again.".to_string());
                    } else {
                        handles
                            .set_status_text
                            .set(format!("Failed to process realtime update: {error}"));
                    }
                }
            });
        }));
    source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let on_error = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        handles.set_realtime_status.set(RealtimeStatus::Degraded);
        handles
            .set_status_text
            .set("Realtime stream unavailable. Polling fallback remains active.".to_string());
    }));
    source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    handles.set_event_source.set(Some(source));
}

async fn refresh_for_ui_event(event: UiEvent, handles: UiEventSyncHandles) -> Result<(), String> {
    match event.event_type.as_str() {
        "thread.changed" => {
            if event.thread_id.as_ref() == handles.selected_thread_id.get_untracked().as_ref()
                && let Some(thread_id) = event.thread_id
            {
                sync_selected_thread_quiet(thread_id, handles.set_selected_thread).await?;
            }
            sync_thread_list_quiet(
                handles.set_threads,
                handles.selected_thread_id,
                handles.set_selected_thread_id,
            )
            .await?;
        }
        "job.changed" => {
            sync_job_list(handles.set_jobs).await?;

            if event.thread_id.as_ref() == handles.selected_thread_id.get_untracked().as_ref()
                && let Some(thread_id) = event.thread_id
            {
                sync_selected_thread_quiet(thread_id, handles.set_selected_thread).await?;
                sync_thread_list_quiet(
                    handles.set_threads,
                    handles.selected_thread_id,
                    handles.set_selected_thread_id,
                )
                .await?;
            }

            if event.job_id.as_ref() == handles.selected_job_id.get_untracked().as_ref()
                && let Some(job_id) = event.job_id
            {
                sync_selected_job_quiet(job_id, handles.set_selected_job_detail).await?;
            }
        }
        "device.changed" => {
            sync_job_list(handles.set_jobs).await?;
        }
        _ => {
            sync_thread_list_quiet(
                handles.set_threads,
                handles.selected_thread_id,
                handles.set_selected_thread_id,
            )
            .await?;
        }
    }

    Ok(())
}

async fn sync_thread_list(
    set_threads: WriteSignal<Vec<ThreadSummary>>,
    selected_thread_id: ReadSignal<Option<String>>,
    set_selected_thread_id: WriteSignal<Option<String>>,
    set_status_text: WriteSignal<String>,
) -> Result<(), String> {
    sync_thread_list_internal(
        set_threads,
        selected_thread_id,
        set_selected_thread_id,
        Some(set_status_text),
    )
    .await
}

async fn sync_thread_list_quiet(
    set_threads: WriteSignal<Vec<ThreadSummary>>,
    selected_thread_id: ReadSignal<Option<String>>,
    set_selected_thread_id: WriteSignal<Option<String>>,
) -> Result<(), String> {
    sync_thread_list_internal(
        set_threads,
        selected_thread_id,
        set_selected_thread_id,
        None,
    )
    .await
}

async fn sync_thread_list_internal(
    set_threads: WriteSignal<Vec<ThreadSummary>>,
    selected_thread_id: ReadSignal<Option<String>>,
    set_selected_thread_id: WriteSignal<Option<String>>,
    set_status_text: Option<WriteSignal<String>>,
) -> Result<(), String> {
    let fetched_threads = fetch_threads().await?;
    let current_selected = selected_thread_id.get_untracked();

    if fetched_threads.is_empty() {
        set_selected_thread_id.set(None);
        if let Some(set_status_text) = set_status_text {
            set_status_text.set("No threads yet. Create one to start.".to_string());
        }
    } else {
        let selected_exists = current_selected
            .as_ref()
            .map(|id| fetched_threads.iter().any(|thread| thread.id == *id))
            .unwrap_or(false);

        if !selected_exists {
            set_selected_thread_id.set(fetched_threads.first().map(|thread| thread.id.clone()));
        }

        if let Some(set_status_text) = set_status_text {
            set_status_text.set("Thread state synced.".to_string());
        }
    }

    set_threads.set(fetched_threads);
    Ok(())
}

async fn sync_job_list(set_jobs: WriteSignal<Vec<JobRecord>>) -> Result<(), String> {
    set_jobs.set(fetch_jobs().await?);
    Ok(())
}

async fn sync_selected_thread(
    thread_id: String,
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    set_status_text: WriteSignal<String>,
) -> Result<(), String> {
    sync_selected_thread_quiet(thread_id, set_selected_thread).await?;
    set_status_text.set("Thread detail loaded.".to_string());
    Ok(())
}

async fn sync_selected_thread_quiet(
    thread_id: String,
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
) -> Result<(), String> {
    let thread = fetch_thread(&thread_id).await?;
    set_selected_thread.update(|current| {
        if current.as_ref() != Some(&thread) {
            *current = Some(thread);
        }
    });
    Ok(())
}

async fn sync_selected_job(
    job_id: String,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    set_status_text: WriteSignal<String>,
) -> Result<(), String> {
    sync_selected_job_quiet(job_id, set_selected_job_detail).await?;
    set_status_text.set("Job detail loaded.".to_string());
    Ok(())
}

async fn sync_selected_job_quiet(
    job_id: String,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
) -> Result<(), String> {
    let job = fetch_job(&job_id).await?;
    set_selected_job_detail.update(|current| {
        if current.as_ref() != Some(&job) {
            *current = Some(job);
        }
    });
    Ok(())
}

fn is_auth_error(error: &str) -> bool {
    error.contains("sign in required") || error.contains("status 401")
}

fn read_storage(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .filter(|value| !value.is_empty())
}

fn read_bool_storage(key: &str) -> Option<bool> {
    read_storage(key).map(|value| value == "true")
}

fn write_storage(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

fn write_optional_storage(key: &str, value: Option<&str>) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            let _ = storage.set_item(key, value);
        } else {
            let _ = storage.remove_item(key);
        }
    }
}

impl NavMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Chats => "chats",
            Self::Jobs => "jobs",
            Self::Details => "details",
        }
    }

    fn from_storage(value: &str) -> Self {
        match value {
            "jobs" => Self::Jobs,
            "details" => Self::Details,
            _ => Self::Chats,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RealtimeStatus {
    Connecting,
    Connected,
    Degraded,
    Disconnected,
}

impl RealtimeStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Realtime connecting",
            Self::Connected => "Realtime connected",
            Self::Degraded => "Realtime degraded",
            Self::Disconnected => "Realtime offline",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Degraded => "degraded",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Clone, Copy)]
struct UiEventSyncHandles {
    selected_thread_id: ReadSignal<Option<String>>,
    selected_job_id: ReadSignal<Option<String>>,
    set_threads: WriteSignal<Vec<ThreadSummary>>,
    set_selected_thread_id: WriteSignal<Option<String>>,
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    set_jobs: WriteSignal<Vec<JobRecord>>,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    set_auth_session: WriteSignal<Option<AuthSessionStatus>>,
    set_status_text: WriteSignal<String>,
    set_realtime_status: WriteSignal<RealtimeStatus>,
    set_event_source: WriteSignal<Option<EventSource>>,
}

const STORAGE_SELECTED_THREAD_ID: &str = "elowen.selected_thread_id";
const STORAGE_SELECTED_JOB_ID: &str = "elowen.selected_job_id";
const STORAGE_CONTEXT_OPEN: &str = "elowen.context_open";
const STORAGE_NAV_MODE: &str = "elowen.nav_mode";
const STORAGE_COMPOSER_TEXT: &str = "elowen.composer_text";
const POLL_FALLBACK_MS: u32 = 30_000;

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (jobs, set_jobs) = signal(Vec::<JobRecord>::new());
    let (sidebar_open, set_sidebar_open) = signal(false);
    let (context_open, set_context_open) =
        signal(read_bool_storage(STORAGE_CONTEXT_OPEN).unwrap_or(false));
    let (nav_mode, set_nav_mode) = signal(
        read_storage(STORAGE_NAV_MODE)
            .as_deref()
            .map(NavMode::from_storage)
            .unwrap_or(NavMode::Chats),
    );
    let (auth_session, set_auth_session) = signal(None::<AuthSessionStatus>);
    let (auth_password, set_auth_password) = signal(String::new());
    let (auth_error, set_auth_error) = signal(String::new());
    let (selected_thread_id, set_selected_thread_id) =
        signal(read_storage(STORAGE_SELECTED_THREAD_ID));
    let (selected_thread, set_selected_thread) = signal(None::<ThreadDetail>);
    let (preferred_job_id, set_preferred_job_id) = signal(None::<String>);
    let (selected_job_id, set_selected_job_id) = signal(read_storage(STORAGE_SELECTED_JOB_ID));
    let (selected_job_detail, set_selected_job_detail) = signal(None::<JobDetail>);
    let (new_thread_title, set_new_thread_title) = signal(String::new());
    let (new_message_content, set_new_message_content) =
        signal(read_storage(STORAGE_COMPOSER_TEXT).unwrap_or_default());
    let (new_job_title, set_new_job_title) = signal(String::new());
    let (new_job_repo, set_new_job_repo) = signal(String::from("elowen-api"));
    let (new_job_base_branch, set_new_job_base_branch) = signal(String::from("main"));
    let (new_job_request_text, set_new_job_request_text) = signal(String::new());
    let (status_text, set_status_text) = signal(String::from("Loading threads and jobs..."));
    let (message_pane_pinned, set_message_pane_pinned) = signal(true);
    let (realtime_status, set_realtime_status) = signal(RealtimeStatus::Connecting);
    let (event_source, set_event_source) = signal(None::<EventSource>);
    let message_pane_ref = NodeRef::<html::Div>::new();

    spawn_local(async move {
        match fetch_auth_session().await {
            Ok(session) => {
                let can_access = !session.enabled || session.authenticated;
                set_auth_session.set(Some(session));
                if can_access {
                    if let Err(error) = sync_thread_list(
                        set_threads,
                        selected_thread_id,
                        set_selected_thread_id,
                        set_status_text,
                    )
                    .await
                    {
                        set_status_text.set(format!("Failed to load threads: {error}"));
                    }

                    if let Err(error) = sync_job_list(set_jobs).await {
                        set_status_text.set(format!("Failed to load jobs: {error}"));
                    }

                    if let Some(thread_id) = selected_thread_id.get_untracked()
                        && let Err(error) =
                            sync_selected_thread(thread_id, set_selected_thread, set_status_text)
                                .await
                    {
                        set_status_text.set(format!("Failed to load thread: {error}"));
                    }
                } else {
                    set_status_text.set("Sign in required.".to_string());
                }
            }
            Err(error) => {
                set_status_text.set(format!("Failed to check auth session: {error}"));
            }
        }

        loop {
            TimeoutFuture::new(POLL_FALLBACK_MS).await;

            let can_access = auth_session
                .get_untracked()
                .map(|session| !session.enabled || session.authenticated)
                .unwrap_or(false);

            if !can_access {
                continue;
            }

            if let Err(error) = sync_thread_list(
                set_threads,
                selected_thread_id,
                set_selected_thread_id,
                set_status_text,
            )
            .await
            {
                if is_auth_error(&error) {
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll threads: {error}"));
            }

            if let Err(error) = sync_job_list(set_jobs).await {
                if is_auth_error(&error) {
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll jobs: {error}"));
            }

            if let Some(thread_id) = selected_thread_id.get_untracked()
                && let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
            {
                if is_auth_error(&error) {
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to refresh thread: {error}"));
            }

            if let Some(job_id) = selected_job_id.get_untracked()
                && let Err(error) =
                    sync_selected_job(job_id, set_selected_job_detail, set_status_text).await
            {
                if is_auth_error(&error) {
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to refresh job: {error}"));
            }
        }
    });

    Effect::new(move |_| {
        let can_access = auth_session
            .get()
            .map(|session| !session.enabled || session.authenticated)
            .unwrap_or(false);

        let needs_stream = event_source.with_untracked(Option::is_none);
        if can_access && needs_stream {
            set_realtime_status.set(RealtimeStatus::Connecting);
            connect_ui_event_stream(UiEventSyncHandles {
                selected_thread_id,
                selected_job_id,
                set_threads,
                set_selected_thread_id,
                set_selected_thread,
                set_jobs,
                set_selected_job_detail,
                set_auth_session,
                set_status_text,
                set_realtime_status,
                set_event_source,
            });
        }
    });

    Effect::new(move |_| {
        if let Some(thread_id) = selected_thread_id.get() {
            set_selected_job_detail.set(None);
            set_message_pane_pinned.set(true);

            spawn_local(async move {
                if let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
                {
                    set_status_text.set(format!("Failed to load thread: {error}"));
                }
            });
        } else {
            set_selected_thread.set(None);
            set_selected_job_id.set(None);
            set_selected_job_detail.set(None);
        }
    });

    Effect::new(move |_| {
        write_optional_storage(
            STORAGE_SELECTED_THREAD_ID,
            selected_thread_id.get().as_deref(),
        );
    });

    Effect::new(move |_| {
        write_optional_storage(STORAGE_SELECTED_JOB_ID, selected_job_id.get().as_deref());
    });

    Effect::new(move |_| {
        write_storage(
            STORAGE_CONTEXT_OPEN,
            if context_open.get() { "true" } else { "false" },
        );
    });

    Effect::new(move |_| {
        write_storage(STORAGE_NAV_MODE, nav_mode.get().as_str());
    });

    Effect::new(move |_| {
        write_storage(STORAGE_COMPOSER_TEXT, &new_message_content.get());
    });

    Effect::new(move |_| {
        if let Some(thread) = selected_thread.get() {
            let preferred_job_id = preferred_job_id.get_untracked();
            let current_job_id = selected_job_id.get_untracked();
            let next_job_id = if preferred_job_id
                .as_ref()
                .is_some_and(|job_id| thread.jobs.iter().any(|job| job.id == *job_id))
            {
                preferred_job_id.clone()
            } else if current_job_id
                .as_ref()
                .is_some_and(|job_id| thread.jobs.iter().any(|job| job.id == *job_id))
            {
                current_job_id.clone()
            } else {
                thread.jobs.first().map(|job| job.id.clone())
            };
            if next_job_id != current_job_id {
                set_selected_job_id.set(next_job_id);
            }
            if preferred_job_id.is_some() {
                set_preferred_job_id.set(None);
            }
        } else {
            set_selected_job_id.set(None);
            set_selected_job_detail.set(None);
        }
    });

    Effect::new(move |_| {
        if let Some(job_id) = selected_job_id.get() {
            spawn_local(async move {
                if let Err(error) =
                    sync_selected_job(job_id, set_selected_job_detail, set_status_text).await
                {
                    set_status_text.set(format!("Failed to load job: {error}"));
                }
            });
        } else {
            set_selected_job_detail.set(None);
        }
    });

    Effect::new(move |_| {
        let _ = selected_thread_id.get();
        let _message_count = selected_thread
            .get()
            .map(|thread| thread.messages.len())
            .unwrap_or_default();

        if message_pane_pinned.get_untracked()
            && let Some(message_pane) = message_pane_ref.get()
        {
            message_pane.set_scroll_top(message_pane.scroll_height());
        }
    });

    view! {
        <main class="app-shell">
            {move || {
                match auth_session.get() {
                    None => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <p class="status">"Checking auth session..."</p>
                            </section>
                        </div>
                    }.into_any(),
                    Some(session) if session.enabled && !session.authenticated => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <p class="eyebrow">"Protected Workspace"</p>
                                <h1>"Sign In To Elowen"</h1>
                                <p class="status">"Enter the shared workspace password to access threads, jobs, and notes."</p>
                                <form on:submit=move |ev: ev::SubmitEvent| {
                                    ev.prevent_default();
                                    let password = auth_password.get_untracked();
                                    if password.trim().is_empty() {
                                        set_auth_error.set("Password is required.".to_string());
                                        return;
                                    }

                                    spawn_local({
                                        let set_auth_error = set_auth_error;
                                        let set_auth_session = set_auth_session;
                                        let set_status_text = set_status_text;
                                        let set_threads = set_threads;
                                        let set_jobs = set_jobs;
                                        let selected_thread_id = selected_thread_id;
                                        let set_selected_thread_id = set_selected_thread_id;
                                        let set_selected_thread = set_selected_thread;
                                        async move {
                                            match login_session(&password).await {
                                                Ok(session) => {
                                                    set_auth_error.set(String::new());
                                                    set_auth_session.set(Some(session));
                                                    set_status_text.set("Signed in.".to_string());

                                                    if let Err(error) = sync_thread_list(
                                                        set_threads,
                                                        selected_thread_id,
                                                        set_selected_thread_id,
                                                        set_status_text,
                                                    )
                                                    .await
                                                    {
                                                        set_status_text.set(format!("Failed to load threads: {error}"));
                                                    }

                                                    if let Err(error) = sync_job_list(set_jobs).await {
                                                        set_status_text.set(format!("Failed to load jobs: {error}"));
                                                    }

                                                    if let Some(thread_id) = selected_thread_id.get_untracked()
                                                        && let Err(error) = sync_selected_thread(
                                                            thread_id,
                                                            set_selected_thread,
                                                            set_status_text,
                                                        )
                                                        .await
                                                    {
                                                        set_status_text.set(format!("Failed to load thread: {error}"));
                                                    }
                                                }
                                                Err(error) => {
                                                    set_auth_error.set(error);
                                                }
                                            }
                                        }
                                    });
                                }>
                                    <input
                                        type="password"
                                        placeholder="Workspace password"
                                        prop:value=move || auth_password.get()
                                        on:input=move |ev| set_auth_password.set(event_target_value(&ev))
                                    />
                                    {move || {
                                        let error = auth_error.get();
                                        if error.is_empty() {
                                            ().into_any()
                                        } else {
                                            view! { <p class="auth-error">{error}</p> }.into_any()
                                        }
                                    }}
                                    <div class="auth-actions">
                                        <p class="status">"Authentication is enabled for this deployment."</p>
                                        <button type="submit">"Sign In"</button>
                                    </div>
                                </form>
                            </section>
                        </div>
                    }.into_any(),
                    Some(_) => view! {
                        <div class="workspace-shell">
                            <header class="workspace-header">
                                <div class="header-leading">
                                    <button
                                        type="button"
                                        class="header-button"
                                        on:click=move |_| {
                                            set_nav_mode.set(NavMode::Chats);
                                            set_sidebar_open.set(true);
                                        }
                                    >
                                        <svg class="header-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                            <path d="M4 7h16M4 12h16M4 17h16" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
                                        </svg>
                                        <span>"Threads"</span>
                                    </button>
                                    <div class="header-copy">
                                        <p class="eyebrow">"Elowen"</p>
                                        <h2>{move || {
                                            selected_thread
                                                .get()
                                                .map(|thread| thread.thread.title)
                                                .unwrap_or_else(|| "Chat-first orchestration".to_string())
                                        }}</h2>
                                        <p class="header-subtitle">
                                            {move || {
                                                if selected_thread.get().is_some() {
                                                    status_text.get()
                                                } else {
                                                    "Threads, chat, and explicit laptop handoff.".to_string()
                                                }
                                            }}
                                        </p>
                                    </div>
                                </div>
                                <div class="header-actions">
                                    <button
                                        type="button"
                                        class="header-button"
                                        on:click=move |_| {
                                            set_nav_mode.set(NavMode::Jobs);
                                            set_sidebar_open.set(true);
                                        }
                                    >
                                        <svg class="header-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                            <path d="M5 6h14v12H5zM9 10h6M9 14h4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                                        </svg>
                                        <span>"Jobs"</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="header-button"
                                        data-testid="nav-details"
                                        disabled=move || selected_thread.get().is_none()
                                        on:click=move |_| {
                                            if selected_thread.get_untracked().is_none() {
                                                return;
                                            }
                                            if context_open.get_untracked() {
                                                set_context_open.set(false);
                                                set_nav_mode.set(NavMode::Chats);
                                            } else {
                                                set_context_open.set(true);
                                                set_nav_mode.set(NavMode::Details);
                                            }
                                        }
                                    >
                                        <svg class="header-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                            <path d="M12 8h.01M11.2 12h.8v4h.8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                                            <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.8"/>
                                        </svg>
                                        <span>{move || if context_open.get() { "Hide Details" } else { "Details" }}</span>
                                    </button>
                                    {move || {
                                        match auth_session.get().and_then(|session| session.operator_label) {
                                            Some(operator_label) => view! {
                                                <>
                                                    <span class="topbar-chip operator">{operator_label}</span>
                                                    <span class=move || format!("topbar-chip realtime {}", realtime_status.get().class())>
                                                        {move || realtime_status.get().label()}
                                                    </span>
                                                </>
                                            }.into_any(),
                                            None => view! { <span class="topbar-chip">"Protected workspace"</span> }.into_any(),
                                        }
                                    }}
                                    <button
                                        type="button"
                                        class="logout-button"
                                        on:click=move |_| {
                                            spawn_local({
                                                let set_auth_session = set_auth_session;
                                                let set_status_text = set_status_text;
                                                let set_selected_thread = set_selected_thread;
                                                let set_selected_thread_id = set_selected_thread_id;
                                                let set_selected_job_detail = set_selected_job_detail;
                                                let set_selected_job_id = set_selected_job_id;
                                                let set_threads = set_threads;
                                                let set_jobs = set_jobs;
                                                let set_realtime_status = set_realtime_status;
                                                let set_event_source = set_event_source;
                                                async move {
                                                    match logout_session().await {
                                                        Ok(session) => {
                                                            event_source.with_untracked(|source| {
                                                                if let Some(source) = source {
                                                                    source.close();
                                                                }
                                                            });
                                                            set_event_source.set(None);
                                                            set_realtime_status.set(RealtimeStatus::Disconnected);
                                                            set_auth_session.set(Some(session));
                                                            set_status_text.set("Signed out.".to_string());
                                                            set_selected_thread.set(None);
                                                            set_selected_thread_id.set(None);
                                                            set_selected_job_detail.set(None);
                                                            set_selected_job_id.set(None);
                                                            set_threads.set(Vec::new());
                                                            set_jobs.set(Vec::new());
                                                            set_sidebar_open.set(false);
                                                            set_context_open.set(false);
                                                        }
                                                        Err(error) => {
                                                            set_status_text.set(format!("Failed to sign out: {error}"));
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    >
                                        "Sign Out"
                                    </button>
                                </div>
                            </header>

                            <div class="workspace-body">
                                <button
                                    type="button"
                                    class="drawer-backdrop"
                                    class:open=move || sidebar_open.get()
                                    on:click=move |_| set_sidebar_open.set(false)
                                ></button>
                                <button
                                    type="button"
                                    class="context-backdrop"
                                    class:open=move || context_open.get()
                                    on:click=move |_| {
                                        set_context_open.set(false);
                                        set_nav_mode.set(NavMode::Chats);
                                    }
                                ></button>

                                <aside class="drawer-shell" class:open=move || sidebar_open.get()>
                                    <div class="drawer-header">
                                        <div>
                                            <p class="eyebrow">"Workspace"</p>
                                            <h2>{move || match nav_mode.get() {
                                                NavMode::Jobs => "Job browser",
                                                _ => "Thread browser",
                                            }}</h2>
                                            <p class="status">{move || status_text.get()}</p>
                                        </div>
                                        <button
                                            type="button"
                                            class="drawer-close"
                                            on:click=move |_| set_sidebar_open.set(false)
                                        >
                                            "Close"
                                        </button>
                                    </div>

                                    <div class="drawer-segments" data-testid="nav-rail">
                                        <button
                                            type="button"
                                            class="drawer-segment"
                                            class:active=move || nav_mode.get() == NavMode::Chats
                                            data-testid="nav-chats"
                                            on:click=move |_| set_nav_mode.set(NavMode::Chats)
                                        >
                                            "Threads"
                                        </button>
                                        <button
                                            type="button"
                                            class="drawer-segment"
                                            class:active=move || nav_mode.get() == NavMode::Jobs
                                            data-testid="nav-jobs"
                                            on:click=move |_| set_nav_mode.set(NavMode::Jobs)
                                        >
                                            "Jobs"
                                        </button>
                                    </div>

                                    <div class="drawer-utility-stack">
                                        <button
                                            type="button"
                                            class="button-secondary drawer-utility-button"
                                            data-testid="drawer-details"
                                            disabled=move || selected_thread.get().is_none()
                                            on:click=move |_| {
                                                if selected_thread.get_untracked().is_none() {
                                                    return;
                                                }
                                                set_sidebar_open.set(false);
                                                if context_open.get_untracked() {
                                                    set_context_open.set(false);
                                                    set_nav_mode.set(NavMode::Chats);
                                                } else {
                                                    set_context_open.set(true);
                                                    set_nav_mode.set(NavMode::Details);
                                                }
                                            }
                                        >
                                            {move || {
                                                if context_open.get() {
                                                    "Hide Details"
                                                } else {
                                                    "Conversation Details"
                                                }
                                            }}
                                        </button>
                                        <div class="drawer-status-stack">
                                            {move || {
                                                match auth_session.get().and_then(|session| session.operator_label) {
                                                    Some(operator_label) => view! {
                                                        <div class="drawer-chip-row">
                                                            <span class="topbar-chip operator">{operator_label}</span>
                                                            <span class=move || format!("topbar-chip realtime {}", realtime_status.get().class())>
                                                                {move || realtime_status.get().label()}
                                                            </span>
                                                        </div>
                                                    }.into_any(),
                                                    None => view! { <span class="topbar-chip">"Protected workspace"</span> }.into_any(),
                                                }
                                            }}
                                            <p class="drawer-status-copy">{move || status_text.get()}</p>
                                        </div>
                                        <button
                                            type="button"
                                            class="button-secondary drawer-signout"
                                            on:click=move |_| {
                                                spawn_local({
                                                    let set_auth_session = set_auth_session;
                                                    let set_status_text = set_status_text;
                                                    let set_selected_thread = set_selected_thread;
                                                    let set_selected_thread_id = set_selected_thread_id;
                                                    let set_selected_job_detail = set_selected_job_detail;
                                                    let set_selected_job_id = set_selected_job_id;
                                                    let set_threads = set_threads;
                                                    let set_jobs = set_jobs;
                                                    let set_realtime_status = set_realtime_status;
                                                    let set_event_source = set_event_source;
                                                    async move {
                                                        match logout_session().await {
                                                            Ok(session) => {
                                                                event_source.with_untracked(|source| {
                                                                    if let Some(source) = source {
                                                                        source.close();
                                                                    }
                                                                });
                                                                set_event_source.set(None);
                                                                set_realtime_status.set(RealtimeStatus::Disconnected);
                                                                set_auth_session.set(Some(session));
                                                                set_status_text.set("Signed out.".to_string());
                                                                set_selected_thread.set(None);
                                                                set_selected_thread_id.set(None);
                                                                set_selected_job_detail.set(None);
                                                                set_selected_job_id.set(None);
                                                                set_threads.set(Vec::new());
                                                                set_jobs.set(Vec::new());
                                                                set_sidebar_open.set(false);
                                                                set_context_open.set(false);
                                                            }
                                                            Err(error) => {
                                                                set_status_text.set(format!("Failed to sign out: {error}"));
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        >
                                            "Sign Out"
                                        </button>
                                    </div>

                                    <div class="drawer-body">
                                        <div class="sidebar-view" class:hidden=move || nav_mode.get() != NavMode::Chats data-testid="thread-nav-panel">
                                            <details class="context-panel" open>
                                                <summary>"New Thread"</summary>
                                                <div class="context-panel-body">
                                                    <form on:submit=move |ev: ev::SubmitEvent| {
                                                        ev.prevent_default();
                                                        let title = new_thread_title.get_untracked().trim().to_string();
                                                        if title.is_empty() {
                                                            set_status_text.set("Thread title is required.".to_string());
                                                            return;
                                                        }

                                                        spawn_local({
                                                            let set_new_thread_title = set_new_thread_title;
                                                            let set_selected_thread = set_selected_thread;
                                                            let set_selected_thread_id = set_selected_thread_id;
                                                            let set_sidebar_open = set_sidebar_open;
                                                            let set_status_text = set_status_text;
                                                            let set_threads = set_threads;
                                                            let selected_thread_id = selected_thread_id;
                                                            async move {
                                                                match create_thread(&title).await {
                                                                    Ok(thread) => {
                                                                        let thread_id = thread.thread.id.clone();
                                                                        set_new_thread_title.set(String::new());
                                                                        set_selected_thread.set(Some(thread));
                                                                        set_selected_thread_id.set(Some(thread_id));
                                                                        set_sidebar_open.set(false);
                                                                        set_status_text.set("Thread created.".to_string());
                                                                        let _ = sync_thread_list(
                                                                            set_threads,
                                                                            selected_thread_id,
                                                                            set_selected_thread_id,
                                                                            set_status_text,
                                                                        )
                                                                        .await;
                                                                    }
                                                                    Err(error) => {
                                                                        set_status_text.set(format!("Failed to create thread: {error}"));
                                                                    }
                                                                }
                                                            }
                                                        });
                                                    }>
                                                        <input
                                                            type="text"
                                                            placeholder="New thread title"
                                                            prop:value=move || new_thread_title.get()
                                                            on:input=move |ev| set_new_thread_title.set(event_target_value(&ev))
                                                        />
                                                        <button type="submit">"Create Thread"</button>
                                                    </form>
                                                </div>
                                            </details>
                                            <div class="sidebar-section">
                                                <p class="eyebrow">"Recent Threads"</p>
                                                <div class="thread-list" data-testid="thread-list">
                                                    <For
                                                        each=move || threads.get()
                                                        key=|thread| thread.id.clone()
                                                        children=move |thread| {
                                                            let active_thread_id = thread.id.clone();
                                                            let click_thread_id = thread.id.clone();
                                                            let updated_at = thread.updated_at.clone();
                                                            view! {
                                                                <article
                                                                    class=("thread-card", true)
                                                                    class:active=move || selected_thread_id.get() == Some(active_thread_id.clone())
                                                                    on:click=move |_| {
                                                                        set_selected_thread_id.set(Some(click_thread_id.clone()));
                                                                        set_context_open.set(false);
                                                                        set_nav_mode.set(NavMode::Chats);
                                                                        set_sidebar_open.set(false);
                                                                    }
                                                                >
                                                                    <div class="thread-card-header">
                                                                        <h3>{thread.title.clone()}</h3>
                                                                        <span class="thread-card-time">{updated_at.clone()}</span>
                                                                    </div>
                                                                    <div class="thread-meta">
                                                                        <span class="thread-status-dot"></span>
                                                                        <span>{thread.status.clone()}</span>
                                                                        <span>{format!("{} messages", thread.message_count)}</span>
                                                                    </div>
                                                                </article>
                                                            }
                                                        }
                                                    />
                                                </div>
                                            </div>
                                        </div>

                                        <div class="sidebar-view" class:hidden=move || nav_mode.get() != NavMode::Jobs data-testid="job-nav-panel">
                                            <div class="job-browser" data-testid="job-browser">
                                                <div class="job-browser-header">
                                                    <div>
                                                        <p class="eyebrow">"Global Jobs"</p>
                                                        <h2>"Recent execution history"</h2>
                                                        <p class="status">"Select a job to jump back into its thread and open details."</p>
                                                    </div>
                                                    <span class="thread-pill">{move || format!("{} jobs", jobs.get().len())}</span>
                                                </div>
                                                <div class="job-list">
                                                    <For
                                                        each=move || jobs.get()
                                                        key=|job| job.id.clone()
                                                        children=move |job| {
                                                            let active_job_id = job.id.clone();
                                                            let click_job_id = job.id.clone();
                                                            let click_thread_id = job.thread_id.clone();
                                                            let thread_label = if job.thread_id.len() > 8 {
                                                                job.thread_id[..8].to_string()
                                                            } else {
                                                                job.thread_id.clone()
                                                            };
                                                            view! {
                                                                <article
                                                                    class=("job-card", true)
                                                                    class:active=move || selected_job_id.get() == Some(active_job_id.clone())
                                                                    on:click=move |_| {
                                                                        set_preferred_job_id.set(Some(click_job_id.clone()));
                                                                        set_selected_job_id.set(Some(click_job_id.clone()));
                                                                        set_selected_thread_id.set(Some(click_thread_id.clone()));
                                                                        set_nav_mode.set(NavMode::Chats);
                                                                        set_context_open.set(true);
                                                                        set_sidebar_open.set(false);
                                                                    }
                                                                >
                                                                    <header>
                                                                        <div>
                                                                            <h3>{job.title.clone()}</h3>
                                                                            <div class="status-row">
                                                                                <span class=format!(
                                                                                    "status-badge {}",
                                                                                    status_badge_class(&job.status)
                                                                                )>
                                                                                    {job.status.clone()}
                                                                                </span>
                                                                                <span class="status">{job.short_id.clone()}</span>
                                                                            </div>
                                                                        </div>
                                                                        <strong>{job.repo_name.clone()}</strong>
                                                                    </header>
                                                                    <div class="job-meta">
                                                                        <span>{format!("Thread: {}", thread_label)}</span>
                                                                        <span>{format!("Updated: {}", job.updated_at.clone())}</span>
                                                                    </div>
                                                                </article>
                                                            }
                                                        }
                                                    />
                                                    {move || {
                                                        if jobs.get().is_empty() {
                                                            view! {
                                                                <div class="empty">
                                                                    <p class="eyebrow">"No Jobs Yet"</p>
                                                                    <p>"Jobs will appear here after a conversation is explicitly handed off to the laptop edge."</p>
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }
                                                    }}
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </aside>

                                <section class="workspace-canvas">
                                    {move || {
                                        if let Some(thread) = selected_thread.get() {
                                            let thread_id = thread.thread.id.clone();
                                            let message_actions_thread_id = thread_id.clone();
                                            let chat_submit_thread_id = thread_id.clone();
                                            let thread_record = thread.thread.clone();
                                            let jobs = thread.jobs.clone();
                                            let messages = thread.messages.clone();
                                            let thread_notes = thread.related_notes.clone();

                                            view! {
                                                <div class="thread-focus">
                                                    <section class="thread-hero">
                                                        <div class="thread-hero-copy">
                                                            <p class="eyebrow">"Conversation"</p>
                                                            <h2>{thread_record.title.clone()}</h2>
                                                            <div class="thread-summary-row">
                                                                <span class="thread-pill">{format!("{} messages", messages.len())}</span>
                                                                <span class="thread-pill">{format!("{} jobs", jobs.len())}</span>
                                                                <span class="thread-pill">{format!("{} notes", thread_notes.len())}</span>
                                                                <span class="thread-pill">{format!("Updated {}", thread_record.updated_at)}</span>
                                                            </div>
                                                        </div>
                                                        <div class="thread-hero-actions">
                                                            <span class=format!(
                                                                "status-badge {}",
                                                                status_badge_class(&thread_record.status)
                                                            )>
                                                                {thread_record.status.clone()}
                                                            </span>
                                                            <button
                                                                type="button"
                                                                class="header-button"
                                                                on:click=move |_| {
                                                                    if context_open.get_untracked() {
                                                                        set_context_open.set(false);
                                                                        set_nav_mode.set(NavMode::Chats);
                                                                    } else {
                                                                        set_context_open.set(true);
                                                                        set_nav_mode.set(NavMode::Details);
                                                                    }
                                                                }
                                                            >
                                                                {move || if context_open.get() { "Hide Details" } else { "Show Details" }}
                                                            </button>
                                                        </div>
                                                    </section>

                                                    <div class="thread-primary">
                                                        <div
                                                            class="message-pane"
                                                            data-testid="message-pane"
                                                            node_ref=message_pane_ref
                                                            on:scroll=move |_| {
                                                                if let Some(message_pane) = message_pane_ref.get() {
                                                                    let distance_from_bottom = message_pane.scroll_height()
                                                                        - message_pane.scroll_top()
                                                                        - message_pane.client_height();
                                                                    set_message_pane_pinned.set(distance_from_bottom <= 96);
                                                                }
                                                            }
                                                        >
                                                            <div class="message-list">
                                                                <For
                                                                    each=move || messages.clone()
                                                                    key=|message| message.id.clone()
                                                                    children=move |message| {
                                                                        let message_id = message.id.clone();
                                                                        let message_role = message.role.clone();
                                                                        let execution_draft = message_execution_draft(&message);
                                                                        let result_details = message_result_details(&message);
                                                                        let message_mode_class = message_mode_class(&message);
                                                                        let message_mode_badge = message_mode_badge(&message);
                                                                        let can_dispatch = message_role == "user"
                                                                            || (message_role == "assistant"
                                                                                && message.status == "conversation.reply"
                                                                                && execution_draft.is_none());
                                                                        let dispatch_label = if message_role == "assistant" {
                                                                            "Dispatch This Plan"
                                                                        } else {
                                                                            "Dispatch This Request"
                                                                        };
                                                                        let has_inspector =
                                                                            result_details.is_some() || execution_draft.is_some() || can_dispatch;
                                                                        let row_class = if message.role == "user" {
                                                                            "message-row outgoing"
                                                                        } else {
                                                                            "message-row incoming"
                                                                        };
                                                                        let role_label = match message.role.as_str() {
                                                                            "assistant" => "Elowen",
                                                                            "system" => "System",
                                                                            _ => "You",
                                                                        };

                                                                        view! {
                                                                            <div class=row_class>
                                                                                <article class=format!("message {} {}", message.role, message_mode_class)>
                                                                                    <header class="message-header">
                                                                                        <div class="message-header-main">
                                                                                            <span class="message-role">{role_label}</span>
                                                                                            {message_mode_badge.map(|(badge_class, label)| {
                                                                                                view! { <span class=format!("mode-badge {}", badge_class)>{label}</span> }
                                                                                            })}
                                                                                        </div>
                                                                                        <span class="message-time">{message.created_at.clone()}</span>
                                                                                    </header>
                                                                                    <p class="message-body">{message.content.clone()}</p>
                                                                                    {if has_inspector {
                                                                                        view! {
                                                                                            <details class="message-inspector">
                                                                                                <summary>"Inspect"</summary>
                                                                                                <div class="message-inspector-body">
                                                                                                    {result_details.clone().map(|details| {
                                                                                                        view! {
                                                                                                            <section class="summary-block">
                                                                                                                <p class="eyebrow">"Result Details"</p>
                                                                                                                <pre>{details}</pre>
                                                                                                            </section>
                                                                                                        }
                                                                                                    })}
                                                                                                    {execution_draft.clone().map(|draft| {
                                                                                                        let thread_id = message_actions_thread_id.clone();
                                                                                                        let source_message_id = message_id.clone();
                                                                                                        let source_role = draft.source_role.clone();
                                                                                                        let title = draft.title.clone();
                                                                                                        let repo_name = draft.repo_name.unwrap_or_default();
                                                                                                        let base_branch = draft.base_branch.clone();
                                                                                                        let request_text = draft.request_text.clone();
                                                                                                        let execution_intent = draft.execution_intent.clone();
                                                                                                        view! {
                                                                                                            <section class="execution-draft">
                                                                                                                <header>
                                                                                                                    <div>
                                                                                                                        <h4>"Execution Draft"</h4>
                                                                                                                        <p class="draft-rationale">{draft.rationale}</p>
                                                                                                                    </div>
                                                                                                                    <span>{format!("From {} message {}", draft.source_role, draft.source_message_id)}</span>
                                                                                                                </header>
                                                                                                                <div class="job-meta">
                                                                                                                    <span>{title.clone()}</span>
                                                                                                                    <span>{repo_name.clone()}</span>
                                                                                                                    <span>{base_branch.clone()}</span>
                                                                                                                    <span>{execution_intent_label(&execution_intent)}</span>
                                                                                                                </div>
                                                                                                                <pre>{request_text.clone()}</pre>
                                                                                                                <div class="draft-actions">
                                                                                                                    <button
                                                                                                                        type="button"
                                                                                                                        on:click=move |_| {
                                                                                                                            if repo_name.trim().is_empty() || request_text.trim().is_empty() {
                                                                                                                                set_status_text.set("Draft repository and request text are required before dispatching.".to_string());
                                                                                                                                return;
                                                                                                                            }
                                                                                                                            spawn_local({
                                                                                                                                let set_selected_thread = set_selected_thread;
                                                                                                                                let set_preferred_job_id = set_preferred_job_id;
                                                                                                                                let set_selected_job_id = set_selected_job_id;
                                                                                                                                let set_status_text = set_status_text;
                                                                                                                                let set_threads = set_threads;
                                                                                                                                let set_jobs = set_jobs;
                                                                                                                                let selected_thread_id = selected_thread_id;
                                                                                                                                let set_selected_thread_id = set_selected_thread_id;
                                                                                                                                let thread_id = thread_id.clone();
                                                                                                                                let source_message_id = source_message_id.clone();
                                                                                                                                let source_role = source_role.clone();
                                                                                                                                let title = title.clone();
                                                                                                                                let repo_name = repo_name.clone();
                                                                                                                                let base_branch = base_branch.clone();
                                                                                                                                let request_text = request_text.clone();
                                                                                                                                let execution_intent = execution_intent.clone();
                                                                                                                                async move {
                                                                                                                                    match dispatch_thread_message(&thread_id, &source_message_id, &title, &repo_name, &base_branch, Some(request_text), Some(execution_intent)).await {
                                                                                                                                        Ok(job) => {
                                                                                                                                            set_preferred_job_id.set(Some(job.id.clone()));
                                                                                                                                            set_selected_job_id.set(Some(job.id.clone()));
                                                                                                                                            set_status_text.set(format!("Promoted {} draft into job {}.", source_role, job.short_id));
                                                                                                                                            let _ = sync_selected_thread(thread_id.clone(), set_selected_thread, set_status_text).await;
                                                                                                                                            let _ = sync_thread_list(set_threads, selected_thread_id, set_selected_thread_id, set_status_text).await;
                                                                                                                                            let _ = sync_job_list(set_jobs).await;
                                                                                                                                        }
                                                                                                                                        Err(error) => set_status_text.set(format!("Failed to dispatch execution draft: {error}")),
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        }
                                                                                                                    >
                                                                                                                        "Dispatch Draft"
                                                                                                                    </button>
                                                                                                                </div>
                                                                                                            </section>
                                                                                                        }
                                                                                                    })}
                                                                                                    {if can_dispatch {
                                                                                                        view! {
                                                                                                            <div class="thread-meta">
                                                                                                                <span>"Explicit handoff"</span>
                                                                                                                <button
                                                                                                                    type="button"
                                                                                                                    on:click={
                                                                                                                        let thread_id = message_actions_thread_id.clone();
                                                                                                                        let source_message_id = message_id.clone();
                                                                                                                        let source_role = message_role.clone();
                                                                                                                        move |_| {
                                                                                                                            let repo_name = new_job_repo.get_untracked().trim().to_string();
                                                                                                                            let title = new_job_title.get_untracked().trim().to_string();
                                                                                                                            let base_branch = new_job_base_branch.get_untracked().trim().to_string();
                                                                                                                            if repo_name.is_empty() {
                                                                                                                                set_status_text.set("Repository is required for laptop dispatch.".to_string());
                                                                                                                                return;
                                                                                                                            }
                                                                                                                            spawn_local({
                                                                                                                                let set_selected_thread = set_selected_thread;
                                                                                                                                let set_preferred_job_id = set_preferred_job_id;
                                                                                                                                let set_selected_job_id = set_selected_job_id;
                                                                                                                                let set_status_text = set_status_text;
                                                                                                                                let set_threads = set_threads;
                                                                                                                                let set_jobs = set_jobs;
                                                                                                                                let selected_thread_id = selected_thread_id;
                                                                                                                                let set_selected_thread_id = set_selected_thread_id;
                                                                                                                                let thread_id = thread_id.clone();
                                                                                                                                let source_message_id = source_message_id.clone();
                                                                                                                                let source_role = source_role.clone();
                                                                                                                                async move {
                                                                                                                                    match dispatch_thread_message(&thread_id, &source_message_id, &title, &repo_name, &base_branch, None, None).await {
                                                                                                                                        Ok(job) => {
                                                                                                                                            set_preferred_job_id.set(Some(job.id.clone()));
                                                                                                                                            set_selected_job_id.set(Some(job.id.clone()));
                                                                                                                                            set_status_text.set(format!("Escalated {} message into job {}.", source_role, job.short_id));
                                                                                                                                            let _ = sync_selected_thread(thread_id.clone(), set_selected_thread, set_status_text).await;
                                                                                                                                            let _ = sync_thread_list(set_threads, selected_thread_id, set_selected_thread_id, set_status_text).await;
                                                                                                                                            let _ = sync_job_list(set_jobs).await;
                                                                                                                                        }
                                                                                                                                        Err(error) => set_status_text.set(format!("Failed to dispatch selected message: {error}")),
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        }
                                                                                                                    }
                                                                                                                >
                                                                                                                    {dispatch_label}
                                                                                                                </button>
                                                                                                            </div>
                                                                                                        }.into_any()
                                                                                                    } else {
                                                                                                        ().into_any()
                                                                                                    }}
                                                                                                </div>
                                                                                            </details>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        ().into_any()
                                                                                    }}
                                                                                </article>
                                                                            </div>
                                                                        }
                                                                    }
                                                                />
                                                            </div>
                                                        </div>

                                                        <div class="composer-dock">
                                                            <form class="thread-composer" data-testid="thread-composer" on:submit=move |ev: ev::SubmitEvent| {
                                                                ev.prevent_default();
                                                                let content = new_message_content.get_untracked().trim().to_string();
                                                                if content.is_empty() {
                                                                    set_status_text.set("Message content is required.".to_string());
                                                                    return;
                                                                }

                                                                spawn_local({
                                                                    let set_new_message_content = set_new_message_content;
                                                                    let set_selected_thread = set_selected_thread;
                                                                    let set_status_text = set_status_text;
                                                                    let set_threads = set_threads;
                                                                    let selected_thread_id = selected_thread_id;
                                                                    let set_selected_thread_id = set_selected_thread_id;
                                                                    let thread_id = chat_submit_thread_id.clone();

                                                                    async move {
                                                                        match send_thread_chat_message(&thread_id, &content).await {
                                                                            Ok(()) => {
                                                                                set_new_message_content.set(String::new());
                                                                                set_status_text.set("Assistant replied in chat.".to_string());
                                                                                let _ = sync_selected_thread(
                                                                                    thread_id.clone(),
                                                                                    set_selected_thread,
                                                                                    set_status_text,
                                                                                )
                                                                                .await;
                                                                                let _ = sync_thread_list(
                                                                                    set_threads,
                                                                                    selected_thread_id,
                                                                                    set_selected_thread_id,
                                                                                    set_status_text,
                                                                                )
                                                                                .await;
                                                                            }
                                                                            Err(error) => {
                                                                                set_status_text.set(format!("Failed to send chat message: {error}"));
                                                                            }
                                                                        }
                                                                    }
                                                                });
                                                            }>
                                                                <div class="composer-input-wrap">
                                                                    <textarea
                                                                        rows="1"
                                                                        placeholder="Message Elowen"
                                                                        prop:value=move || new_message_content.get()
                                                                        on:input=move |ev| set_new_message_content.set(event_target_value(&ev))
                                                                        on:keydown=move |ev: ev::KeyboardEvent| {
                                                                            if ev.ctrl_key() && ev.key() == "Enter" {
                                                                                ev.prevent_default();
                                                                                if let Some(form) = ev
                                                                                    .target()
                                                                                    .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                                                                                    .and_then(|textarea| textarea.form())
                                                                                {
                                                                                    let _ = form.request_submit();
                                                                                }
                                                                            }
                                                                        }
                                                                    />
                                                                    <button type="submit" class="composer-send" aria-label="Send message">
                                                                        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                                                            <path d="M5 12h12m0 0-5-5m5 5-5 5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                                                                        </svg>
                                                                    </button>
                                                                </div>
                                                            </form>
                                                        </div>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div class="thread-focus">
                                                    <div class="empty">
                                                        <p class="eyebrow">"No Thread Selected"</p>
                                                        <h2>"Choose a thread to continue"</h2>
                                                        <p>"Open the drawer to start a new conversation or jump into an existing job thread."</p>
                                                        <button
                                                            type="button"
                                                            class="header-button primary"
                                                            on:click=move |_| {
                                                                set_nav_mode.set(NavMode::Chats);
                                                                set_sidebar_open.set(true);
                                                            }
                                                        >
                                                            "Open Threads"
                                                        </button>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }
                                    }}
                                </section>

                                <aside class="context-sheet" class:open=move || context_open.get() data-testid="context-sheet">
                                    <div class="context-shell-header">
                                        <div>
                                            <p class="eyebrow">"Conversation Details"</p>
                                            <h3>{move || {
                                                selected_thread
                                                    .get()
                                                    .map(|thread| thread.thread.title)
                                                    .unwrap_or_else(|| "No thread selected".to_string())
                                            }}</h3>
                                        </div>
                                        <button
                                            type="button"
                                            class="context-close"
                                            on:click=move |_| {
                                                set_context_open.set(false);
                                                set_nav_mode.set(NavMode::Chats);
                                            }
                                        >
                                            "Close"
                                        </button>
                                    </div>

                                    <div class="context-body">
                                        {move || {
                                            if let Some(thread) = selected_thread.get() {
                                                let thread_id = thread.thread.id.clone();
                                                let job_thread_id = thread_id.clone();
                                                let jobs = thread.jobs.clone();
                                                let thread_notes = thread.related_notes.clone();
                                                let has_jobs = !jobs.is_empty();
                                                let active_job_id = selected_job_id.get();

                                                view! {
                                                    <details class="context-panel" open>
                                                        <summary>"Thread Context"</summary>
                                                        <div class="context-panel-body">
                                                            <div class="note-list">
                                                                <p class="eyebrow">"Related Notes"</p>
                                                                {if thread_notes.is_empty() {
                                                                    view! {
                                                                        <div class="empty">
                                                                            <p>"No related notes were found for this thread yet."</p>
                                                                        </div>
                                                                    }.into_any()
                                                                } else {
                                                                    view! {
                                                                        <For
                                                                            each=move || thread_notes.clone()
                                                                            key=|note| note.note_id.clone()
                                                                            children=move |note| {
                                                                                view! {
                                                                                    <article class="note-card">
                                                                                        <header>
                                                                                            <strong>{note.title.clone()}</strong>
                                                                                            <span>{note.note_type.clone()}</span>
                                                                                        </header>
                                                                                        <p>{note.summary.clone()}</p>
                                                                                        <div class="job-meta">
                                                                                            <span>{note.slug.clone()}</span>
                                                                                            <span>{format!("Updated: {}", note.updated_at.clone())}</span>
                                                                                        </div>
                                                                                    </article>
                                                                                }
                                                                            }
                                                                        />
                                                                    }.into_any()
                                                                }}
                                                            </div>

                                                            <p class="eyebrow">"Jobs"</p>
                                                            <div class="job-list">
                                                                <For
                                                                    each=move || jobs.clone()
                                                                    key=|job| job.id.clone()
                                                                    children=move |job| {
                                                                        let card_job_id = job.id.clone();
                                                                        let is_active = active_job_id == Some(card_job_id.clone());
                                                                        view! {
                                                                            <article
                                                                                class=("job-card", true)
                                                                                class:active=is_active
                                                                                on:click=move |_| set_selected_job_id.set(Some(card_job_id.clone()))
                                                                            >
                                                                                <header>
                                                                                    <div>
                                                                                        <h3>{job.title.clone()}</h3>
                                                                                        <div class="status-row">
                                                                                            <span class=format!(
                                                                                                "status-badge {}",
                                                                                                status_badge_class(&job.status)
                                                                                            )>
                                                                                                {job.status.clone()}
                                                                                            </span>
                                                                                            <span class="status">{job.short_id.clone()}</span>
                                                                                        </div>
                                                                                    </div>
                                                                                    <strong>{job.repo_name.clone()}</strong>
                                                                                </header>
                                                                                <div class="job-meta">
                                                                                    <span>{format!("Branch: {}", job.branch_name.clone().unwrap_or_else(|| "pending".to_string()))}</span>
                                                                                    <span>{format!("Base: {}", job.base_branch.clone().unwrap_or_else(|| "main".to_string()))}</span>
                                                                                    <span>{format!("Device: {}", job.device_id.clone().unwrap_or_else(|| "unassigned".to_string()))}</span>
                                                                                    <span>{format!("Updated: {}", job.updated_at.clone())}</span>
                                                                                </div>
                                                                            </article>
                                                                        }
                                                                    }
                                                                />
                                                                {if has_jobs {
                                                                    ().into_any()
                                                                } else {
                                                                    view! {
                                                                        <div class="empty">
                                                                            <p class="eyebrow">"No Jobs Yet"</p>
                                                                            <p>"Conversation is the default. Use the explicit dispatch controls when you want to create a laptop job."</p>
                                                                        </div>
                                                                    }.into_any()
                                                                }}
                                                            </div>
                                                        </div>
                                                    </details>
                                                    {move || {
                                                        if let Some(job_detail) = selected_job_detail.get() {
                                                            let execution_report = job_detail.execution_report_json.clone();
                                                            let build_status = report_status_label(&execution_report, "build");
                                                            let test_status = report_status_label(&execution_report, "test");
                                                            let diff_stat = report_diff_stat(&execution_report);
                                                            let last_message = report_last_message(&execution_report);
                                                            let changed_files = report_array_strings(&execution_report, "changed_files");
                                                            let git_status = report_array_strings(&execution_report, "git_status");
                                                            let approvals = job_detail.approvals.clone();
                                                            let related_notes = job_detail.related_notes.clone();
                                                            let summary = job_detail.summary.clone();
                                                            let approval_thread_id = thread_id.clone();

                                                            view! {
                                                                <details class="context-panel" open>
                                                                    <summary>"Selected Job"</summary>
                                                                    <div class="context-panel-body">
                                                                        <section class="job-detail">
                                                                            <p class="eyebrow">"Job Detail"</p>
                                                                            <h3>{job_detail.job.title.clone()}</h3>
                                                                            <div class="status-row">
                                                                                <span class=format!(
                                                                                    "status-badge {}",
                                                                                    status_badge_class(&job_detail.job.status)
                                                                                )>
                                                                                    {job_detail.job.status.clone()}
                                                                                </span>
                                                                                <span class="status">{format!("Job {}", job_detail.job.short_id)}</span>
                                                                                {job_detail.job.result.clone().map(|result| {
                                                                                    let result_class = status_badge_class(&result);
                                                                                    view! { <span class=format!("status-badge {}", result_class)>{result}</span> }
                                                                                })}
                                                                            </div>
                                                                            <div class="job-meta">
                                                                                <span>{format!("Correlation: {}", job_detail.job.correlation_id.clone())}</span>
                                                                                <span>{format!("Updated: {}", job_detail.job.updated_at.clone())}</span>
                                                                            </div>
                                                                            <div class="job-overview">
                                                                                <article>
                                                                                    <p class="eyebrow">"Repository"</p>
                                                                                    <strong>{job_detail.job.repo_name.clone()}</strong>
                                                                                    <span>{format!("Base {}", job_detail.job.base_branch.clone().unwrap_or_else(|| "main".to_string()))}</span>
                                                                                </article>
                                                                                <article>
                                                                                    <p class="eyebrow">"Branch"</p>
                                                                                    <strong>{job_detail.job.branch_name.clone().unwrap_or_else(|| "pending".to_string())}</strong>
                                                                                    <span>{format!("Device {}", job_detail.job.device_id.clone().unwrap_or_else(|| "unassigned".to_string()))}</span>
                                                                                </article>
                                                                                <article>
                                                                                    <p class="eyebrow">"Outcome"</p>
                                                                                    <strong>{job_detail.job.result.clone().unwrap_or_else(|| "pending".to_string())}</strong>
                                                                                    <span>{format!("Failure class {}", job_detail.job.failure_class.clone().unwrap_or_else(|| "none".to_string()))}</span>
                                                                                </article>
                                                                            </div>
                                                                            <div class="report-grid">
                                                                                <article>
                                                                                    <p class="eyebrow">"Build"</p>
                                                                                    <strong>{build_status}</strong>
                                                                                </article>
                                                                                <article>
                                                                                    <p class="eyebrow">"Test"</p>
                                                                                    <strong>{test_status}</strong>
                                                                                </article>
                                                                                <article>
                                                                                    <p class="eyebrow">"Changed Files"</p>
                                                                                    <strong>{changed_files.len()}</strong>
                                                                                </article>
                                                                            </div>
                                                                            <div class="summary-block">
                                                                                <p class="eyebrow">"Outcome Message"</p>
                                                                                {if let Some(last_message) = last_message {
                                                                                    view! {
                                                                                        <article class="result-message">
                                                                                            <p class="eyebrow">"Runner Last Message"</p>
                                                                                            <pre>{last_message}</pre>
                                                                                        </article>
                                                                                    }.into_any()
                                                                                } else {
                                                                                    view! {
                                                                                        <div class="empty">
                                                                                            <p>"No final runner message was captured for this job."</p>
                                                                                        </div>
                                                                                    }.into_any()
                                                                                }}
                                                                            </div>
                                                                            <div class="summary-block">
                                                                                <p class="eyebrow">"Summary"</p>
                                                                                {if let Some(summary) = summary {
                                                                                    let promote_job_id = job_detail.job.id.clone();
                                                                                    let promote_thread_id = thread_id.clone();
                                                                                    view! {
                                                                                        <article>
                                                                                            <div class="job-meta">
                                                                                                <span>{format!("Version {}", summary.version)}</span>
                                                                                                <span>{summary.created_at}</span>
                                                                                            </div>
                                                                                            <div class="summary-body">{summary.content}</div>
                                                                                            <div class="approval-actions">
                                                                                                <button
                                                                                                    type="button"
                                                                                                    on:click=move |_| {
                                                                                                        spawn_local({
                                                                                                            let job_id = promote_job_id.clone();
                                                                                                            let thread_id = promote_thread_id.clone();
                                                                                                            let set_selected_job_detail = set_selected_job_detail;
                                                                                                            let set_selected_thread = set_selected_thread;
                                                                                                            let set_status_text = set_status_text;
                                                                                                            async move {
                                                                                                                match promote_job_note(&job_id).await {
                                                                                                                    Ok(note) => {
                                                                                                                        set_status_text.set(format!("Promoted note: {}", note.title));
                                                                                                                        let _ = sync_selected_job(job_id, set_selected_job_detail, set_status_text).await;
                                                                                                                        let _ = sync_selected_thread(thread_id, set_selected_thread, set_status_text).await;
                                                                                                                    }
                                                                                                                    Err(error) => set_status_text.set(format!("Failed to promote note: {error}")),
                                                                                                                }
                                                                                                            }
                                                                                                        });
                                                                                                    }
                                                                                                >
                                                                                                    "Promote Summary To Notes"
                                                                                                </button>
                                                                                            </div>
                                                                                        </article>
                                                                                    }.into_any()
                                                                                } else {
                                                                                    view! {
                                                                                        <div class="empty">
                                                                                            <p>"No generated summary yet."</p>
                                                                                        </div>
                                                                                    }.into_any()
                                                                                }}
                                                                            </div>
                                                                            <div class="summary-block">
                                                                                <p class="eyebrow">"Execution Report"</p>
                                                                                <article>
                                                                                    <div class="job-meta">
                                                                                        <span>{format!("Diff: {}", diff_stat.unwrap_or_else(|| "no tracked diff".to_string()))}</span>
                                                                                    </div>
                                                                                    <p><strong>"Changed files"</strong></p>
                                                                                    <pre>{format_string_list(&changed_files)}</pre>
                                                                                    <p><strong>"Git status"</strong></p>
                                                                                    <pre>{format_string_list(&git_status)}</pre>
                                                                                </article>
                                                                            </div>
                                                                            <div class="note-list">
                                                                                <p class="eyebrow">"Job Notes"</p>
                                                                                {if related_notes.is_empty() {
                                                                                    view! {
                                                                                        <div class="empty">
                                                                                            <p>"No related notes were found for this job yet."</p>
                                                                                        </div>
                                                                                    }.into_any()
                                                                                } else {
                                                                                    view! {
                                                                                        <For
                                                                                            each=move || related_notes.clone()
                                                                                            key=|note| note.note_id.clone()
                                                                                            children=move |note| {
                                                                                                view! {
                                                                                                    <article class="note-card">
                                                                                                        <header>
                                                                                                            <strong>{note.title.clone()}</strong>
                                                                                                            <span>{note.note_type.clone()}</span>
                                                                                                        </header>
                                                                                                        <p>{note.summary.clone()}</p>
                                                                                                        <div class="job-meta">
                                                                                                            <span>{note.slug.clone()}</span>
                                                                                                            <span>{format!("Updated: {}", note.updated_at.clone())}</span>
                                                                                                        </div>
                                                                                                    </article>
                                                                                                }
                                                                                            }
                                                                                        />
                                                                                    }.into_any()
                                                                                }}
                                                                            </div>
                                                                            <div class="approval-list">
                                                                                <p class="eyebrow">"Approvals"</p>
                                                                                {if approvals.is_empty() {
                                                                                    view! {
                                                                                        <div class="empty">
                                                                                            <p>"No approval gate has been raised for this job."</p>
                                                                                        </div>
                                                                                    }.into_any()
                                                                                } else {
                                                                                    view! {
                                                                                        <For
                                                                                            each=move || approvals.clone()
                                                                                            key=|approval| approval.id.clone()
                                                                                            children=move |approval| {
                                                                                                let approve_id = approval.id.clone();
                                                                                                let approve_job_id = approval.job_id.clone();
                                                                                                let approve_thread_id = approval_thread_id.clone();
                                                                                                let reject_id = approval.id.clone();
                                                                                                let reject_job_id = approval.job_id.clone();
                                                                                                let reject_thread_id = approval_thread_id.clone();
                                                                                                let is_pending = approval.status == "pending";
                                                                                                let approval_status_note =
                                                                                                    approval_status_note(
                                                                                                        &approval.status,
                                                                                                        &job_detail.job.status,
                                                                                                    );
                                                                                                view! {
                                                                                                    <article class=("approval-card", true) class:pending=is_pending>
                                                                                                        <header>
                                                                                                            <div>
                                                                                                                <strong>{approval.summary.clone()}</strong>
                                                                                                                <p class="status">{approval_status_note}</p>
                                                                                                            </div>
                                                                                                            <span class=format!("status-badge {}", status_badge_class(&approval.status))>
                                                                                                                {approval.status.clone()}
                                                                                                            </span>
                                                                                                        </header>
                                                                                                        <div class="job-meta">
                                                                                                            <span>{format!("Requested: {}", approval.created_at.clone())}</span>
                                                                                                            <span>{format!("Updated: {}", approval.updated_at.clone())}</span>
                                                                                                        </div>
                                                                                                        {if is_pending {
                                                                                                            view! {
                                                                                                                <div class="approval-actions">
                                                                                                                    <button
                                                                                                                        type="button"
                                                                                                                        on:click=move |_| {
                                                                                                                            spawn_local({
                                                                                                                                let approval_id = approve_id.clone();
                                                                                                                                let approval_job_id = approve_job_id.clone();
                                                                                                                                let thread_id = approve_thread_id.clone();
                                                                                                                                let set_selected_job_detail = set_selected_job_detail;
                                                                                                                                let set_selected_thread = set_selected_thread;
                                                                                                                                let set_status_text = set_status_text;
                                                                                                                                async move {
                                                                                                                                    match resolve_approval(&approval_id, "approved", "user", "Push approved from UI").await {
                                                                                                                                        Ok(_) => {
                                                                                                                                            set_status_text.set("Push approved. The edge will continue.".to_string());
                                                                                                                                            let _ = sync_selected_job(approval_job_id, set_selected_job_detail, set_status_text).await;
                                                                                                                                            let _ = sync_selected_thread(thread_id, set_selected_thread, set_status_text).await;
                                                                                                                                        }
                                                                                                                                        Err(error) => set_status_text.set(format!("Failed to approve: {error}")),
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        }
                                                                                                                    >
                                                                                                                        "Approve And Push"
                                                                                                                    </button>
                                                                                                                    <button
                                                                                                                        type="button"
                                                                                                                        class="button-secondary"
                                                                                                                        on:click=move |_| {
                                                                                                                            spawn_local({
                                                                                                                                let approval_id = reject_id.clone();
                                                                                                                                let approval_job_id = reject_job_id.clone();
                                                                                                                                let thread_id = reject_thread_id.clone();
                                                                                                                                let set_selected_job_detail = set_selected_job_detail;
                                                                                                                                let set_selected_thread = set_selected_thread;
                                                                                                                                let set_status_text = set_status_text;
                                                                                                                                async move {
                                                                                                                                    match resolve_approval(&approval_id, "rejected", "user", "Push rejected from UI").await {
                                                                                                                                        Ok(_) => {
                                                                                                                                            set_status_text.set("Push rejected. The branch was not pushed.".to_string());
                                                                                                                                            let _ = sync_selected_job(approval_job_id, set_selected_job_detail, set_status_text).await;
                                                                                                                                            let _ = sync_selected_thread(thread_id, set_selected_thread, set_status_text).await;
                                                                                                                                        }
                                                                                                                                        Err(error) => set_status_text.set(format!("Failed to reject: {error}")),
                                                                                                                                    }
                                                                                                                                }
                                                                                                                            });
                                                                                                                        }
                                                                                                                    >
                                                                                                                        "Reject Push"
                                                                                                                    </button>
                                                                                                                </div>
                                                                                                            }.into_any()
                                                                                                        } else {
                                                                                                            ().into_any()
                                                                                                        }}
                                                                                                    </article>
                                                                                                }
                                                                                            }
                                                                                        />
                                                                                    }.into_any()
                                                                                }}
                                                                            </div>
                                                                            <div class="job-event-list">
                                                                                <For
                                                                                    each=move || job_detail.events.clone()
                                                                                    key=|event| event.id.clone()
                                                                                    children=move |event| {
                                                                                        let payload = format_json_value(&event.payload_json);
                                                                                        view! {
                                                                                            <article class="job-event">
                                                                                                <header>
                                                                                                    <strong>{event.event_type.clone()}</strong>
                                                                                                    <span>{event.created_at.clone()}</span>
                                                                                                </header>
                                                                                                <p class="status">{format!("Correlation: {}", event.correlation_id.clone())}</p>
                                                                                                <pre>{payload}</pre>
                                                                                            </article>
                                                                                        }
                                                                                    }
                                                                                />
                                                                            </div>
                                                                        </section>
                                                                    </div>
                                                                </details>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <details class="context-panel" open>
                                                                    <summary>"Selected Job"</summary>
                                                                    <div class="context-panel-body">
                                                                        <div class="empty">
                                                                            <p class="eyebrow">"No Job Selected"</p>
                                                                            <p>"Choose a job to inspect the live execution detail and event history."</p>
                                                                        </div>
                                                                    </div>
                                                                </details>
                                                            }.into_any()
                                                        }
                                                    }}
                                                    <details class="context-panel">
                                                        <summary>"Advanced Manual Job"</summary>
                                                        <div class="context-panel-body">
                                                            <form on:submit=move |ev: ev::SubmitEvent| {
                                                                ev.prevent_default();
                                                                let title = new_job_title.get_untracked().trim().to_string();
                                                                let repo_name = new_job_repo.get_untracked().trim().to_string();
                                                                let base_branch = new_job_base_branch.get_untracked().trim().to_string();
                                                                let request_text = new_job_request_text.get_untracked().trim().to_string();

                                                                if title.is_empty() || repo_name.is_empty() || request_text.is_empty() {
                                                                    set_status_text.set("Job title, repo, and request are required.".to_string());
                                                                    return;
                                                                }

                                                                spawn_local({
                                                                    let set_new_job_title = set_new_job_title;
                                                                    let set_new_job_request_text = set_new_job_request_text;
                                                                    let set_selected_thread = set_selected_thread;
                                                                    let set_preferred_job_id = set_preferred_job_id;
                                                                    let set_selected_job_id = set_selected_job_id;
                                                                    let set_status_text = set_status_text;
                                                                    let set_threads = set_threads;
                                                                    let set_jobs = set_jobs;
                                                                    let selected_thread_id = selected_thread_id;
                                                                    let set_selected_thread_id = set_selected_thread_id;
                                                                    let thread_id = job_thread_id.clone();

                                                                    async move {
                                                                        match create_job(
                                                                            &thread_id,
                                                                            &title,
                                                                            &repo_name,
                                                                            &base_branch,
                                                                            &request_text,
                                                                            None,
                                                                        )
                                                                        .await
                                                                        {
                                                                            Ok(job) => {
                                                                                set_new_job_title.set(String::new());
                                                                                set_new_job_request_text.set(String::new());
                                                                                set_preferred_job_id.set(Some(job.id.clone()));
                                                                                set_selected_job_id.set(Some(job.id.clone()));
                                                                                set_status_text.set(format!("Job {} is {}.", job.short_id, job.status));
                                                                                let _ = sync_selected_thread(
                                                                                    thread_id.clone(),
                                                                                    set_selected_thread,
                                                                                    set_status_text,
                                                                                )
                                                                                .await;
                                                                                let _ = sync_thread_list(
                                                                                    set_threads,
                                                                                    selected_thread_id,
                                                                                    set_selected_thread_id,
                                                                                    set_status_text,
                                                                                )
                                                                                .await;
                                                                                let _ = sync_job_list(set_jobs).await;
                                                                            }
                                                                            Err(error) => {
                                                                                set_status_text.set(format!("Failed to create job: {error}"));
                                                                            }
                                                                        }
                                                                    }
                                                                });
                                                            }>
                                                                <input
                                                                    type="text"
                                                                    placeholder="Job title"
                                                                    prop:value=move || new_job_title.get()
                                                                    on:input=move |ev| set_new_job_title.set(event_target_value(&ev))
                                                                />
                                                                <input
                                                                    type="text"
                                                                    placeholder="Repository"
                                                                    prop:value=move || new_job_repo.get()
                                                                    on:input=move |ev| set_new_job_repo.set(event_target_value(&ev))
                                                                />
                                                                <input
                                                                    type="text"
                                                                    placeholder="Base branch"
                                                                    prop:value=move || new_job_base_branch.get()
                                                                    on:input=move |ev| set_new_job_base_branch.set(event_target_value(&ev))
                                                                />
                                                                <textarea
                                                                    placeholder="Describe the coding task to dispatch"
                                                                    prop:value=move || new_job_request_text.get()
                                                                    on:input=move |ev| set_new_job_request_text.set(event_target_value(&ev))
                                                                />
                                                                <button type="submit">"Create Job"</button>
                                                            </form>
                                                        </div>
                                                    </details>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <div class="empty">
                                                        <p class="eyebrow">"No Thread Selected"</p>
                                                        <p>"Conversation details appear here after you choose a thread."</p>
                                                    </div>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                </aside>
                            </div>
                        </div>
                    }.into_any(),
                }
            }}
        </main>
    }
}
