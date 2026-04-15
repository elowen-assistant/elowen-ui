mod realtime;
mod session;
mod state;
mod storage;
mod styles;

use gloo_timers::future::TimeoutFuture;
use leptos::{ev, html, prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use self::{
    realtime::{
        connect_ui_event_stream, is_auth_error, sync_job_list, sync_selected_job,
        sync_selected_thread, sync_thread_list,
    },
    session::{apply_polled_session_expired_state, apply_signed_out_state},
    state::{
        NavMode, POLL_FALLBACK_MS, RealtimeStatus, STORAGE_COMPOSER_TEXT, STORAGE_CONTEXT_OPEN,
        STORAGE_NAV_MODE, STORAGE_SELECTED_JOB_ID, STORAGE_SELECTED_THREAD_ID, SignedOutHandles,
        UiEventSyncHandles,
    },
    storage::{read_bool_storage, read_storage, write_optional_storage, write_storage},
    styles::APP_STYLE,
};
use crate::{
    api::{
        create_job, create_thread, dispatch_thread_message, fetch_auth_session,
        login as login_session, logout as logout_session, promote_job_note, resolve_approval,
        send_thread_chat_message,
    },
    format::{
        approval_status_note, execution_intent_label, format_json_value, format_string_list,
        message_execution_draft, message_mode_badge, message_mode_class, message_result_details,
        report_array_strings, report_diff_stat, report_last_message, report_status_label,
        status_badge_class,
    },
    models::*,
};

#[derive(Clone)]
struct PendingChatSubmission {
    thread_id: String,
    content: String,
}

fn apply_chat_reply_to_selected_thread(
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    thread_id: &str,
    reply: &ChatReplyResponse,
) {
    set_selected_thread.update(|current| {
        let Some(current) = current.as_mut() else {
            return;
        };

        if current.thread.id != thread_id {
            return;
        }

        append_thread_message_if_missing(&mut current.messages, reply.user_message.clone());
        append_thread_message_if_missing(&mut current.messages, reply.assistant_message.clone());
    });
}

fn append_thread_message_if_missing(messages: &mut Vec<MessageRecord>, message: MessageRecord) {
    if messages.iter().all(|existing| existing.id != message.id) {
        messages.push(message);
    }
}

fn clear_pending_chat_submission(
    set_pending_chat_submission: WriteSignal<Option<PendingChatSubmission>>,
    thread_id: &str,
    content: &str,
) {
    set_pending_chat_submission.update(|pending| {
        if pending
            .as_ref()
            .is_some_and(|pending| pending.thread_id == thread_id && pending.content == content)
        {
            *pending = None;
        }
    });
}

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (jobs, set_jobs) = signal(Vec::<JobRecord>::new());
    let (sidebar_open, set_sidebar_open) = signal(!is_compact_layout());
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
    let (pending_chat_submission, set_pending_chat_submission) =
        signal(None::<PendingChatSubmission>);
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
                    apply_polled_session_expired_state(
                        set_auth_session,
                        set_selected_thread,
                        set_selected_job_id,
                        set_selected_job_detail,
                        set_status_text,
                    );
                    continue;
                }
                set_status_text.set(format!("Failed to poll threads: {error}"));
            }

            if let Err(error) = sync_job_list(set_jobs).await {
                if is_auth_error(&error) {
                    apply_polled_session_expired_state(
                        set_auth_session,
                        set_selected_thread,
                        set_selected_job_id,
                        set_selected_job_detail,
                        set_status_text,
                    );
                    continue;
                }
                set_status_text.set(format!("Failed to poll jobs: {error}"));
            }

            if let Some(thread_id) = selected_thread_id.get_untracked()
                && let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
            {
                if is_auth_error(&error) {
                    apply_polled_session_expired_state(
                        set_auth_session,
                        set_selected_thread,
                        set_selected_job_id,
                        set_selected_job_detail,
                        set_status_text,
                    );
                    continue;
                }
                set_status_text.set(format!("Failed to refresh thread: {error}"));
            }

            if let Some(job_id) = selected_job_id.get_untracked()
                && let Err(error) =
                    sync_selected_job(job_id, set_selected_job_detail, set_status_text).await
            {
                if is_auth_error(&error) {
                    apply_polled_session_expired_state(
                        set_auth_session,
                        set_selected_thread,
                        set_selected_job_id,
                        set_selected_job_detail,
                        set_status_text,
                    );
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
        let _pending_thread = pending_chat_submission
            .get()
            .map(|pending| pending.thread_id);

        if message_pane_pinned.get_untracked()
            && let Some(message_pane) = message_pane_ref.get()
        {
            message_pane.set_scroll_top(message_pane.scroll_height());
        }
    });

    view! {
        <main class="app-shell">
            <style>{APP_STYLE}</style>
            {move || {
                match auth_session.get() {
                    None => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <div>
                                    <p class="eyebrow">"Elowen Workspace"</p>
                                    <h1>"Checking session"</h1>
                                    <p class="status">"Loading authentication state before the workspace UI appears."</p>
                                </div>
                            </section>
                        </div>
                    }.into_any(),
                    Some(session) if session.enabled && !session.authenticated => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <div>
                                    <p class="eyebrow">"Elowen Sign In"</p>
                                    <h1>"Protected workspace"</h1>
                                    <p class="status">"Sign in to browse threads, dispatch jobs, review notes, and approve pushes."</p>
                                </div>
                                <form data-testid="auth-form" on:submit=move |ev: ev::SubmitEvent| {
                                    ev.prevent_default();
                                    let password = auth_password.get_untracked().trim().to_string();
                                    if password.is_empty() {
                                        set_auth_error.set("Password is required.".to_string());
                                        return;
                                    }

                                    spawn_local({
                                        let set_auth_session = set_auth_session;
                                        let set_auth_error = set_auth_error;
                                        let set_auth_password = set_auth_password;
                                        let set_status_text = set_status_text;
                                        let set_threads = set_threads;
                                        let selected_thread_id = selected_thread_id;
                                        let set_selected_thread_id = set_selected_thread_id;
                                        let set_selected_thread = set_selected_thread;
                                        let set_jobs = set_jobs;

                                        async move {
                                            match login_session(&password).await {
                                                Ok(session) => {
                                                    set_auth_error.set(String::new());
                                                    set_auth_password.set(String::new());
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
                                        data-testid="auth-password"
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
                                        <button type="submit" data-testid="auth-submit">"Sign In"</button>
                                    </div>
                                </form>
                            </section>
                        </div>
                    }.into_any(),
                    Some(_) => view! {
            <div class="workspace-shell">
            <section class="panel app-topbar">
                <div class="app-topbar-leading">
                    <button
                        type="button"
                        class="nav-fab"
                        on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                    >
                        {move || if sidebar_open.get() { "Close" } else { "Threads" }}
                    </button>
                    <div class="app-topbar-copy">
                        <p class="eyebrow">"Elowen Assistant"</p>
                        <h2>"Chat-first orchestration"</h2>
                        <p class="topbar-subtitle">"Threads, chat, and explicit laptop handoff."</p>
                    </div>
                </div>
                <div class="app-topbar-actions">
                    {move || {
                        match auth_session.get().and_then(|session| session.operator_label) {
                            Some(operator_label) => view! {
                                <>
                                    <span class="topbar-chip operator">{operator_label}</span>
                                    <span class=move || format!("topbar-chip realtime {}", realtime_status.get().class())>
                                        {move || realtime_status.get().label()}
                                    </span>
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
                                                            apply_signed_out_state(
                                                                session,
                                                                SignedOutHandles {
                                                                    set_auth_session,
                                                                    set_status_text,
                                                                    set_selected_thread,
                                                                    set_selected_thread_id,
                                                                    set_selected_job_detail,
                                                                    set_selected_job_id,
                                                                    set_threads,
                                                                    set_jobs,
                                                                },
                                                            );
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
                                </>
                            }.into_any(),
                            None => view! { <span class="topbar-chip">"Protected workspace"</span> }.into_any(),
                        }
                    }}
                </div>
            </section>
            <div class="frame">
                <nav class="panel nav-rail" data-testid="nav-rail" aria-label="Primary navigation">
                    <div class="nav-rail-brand" aria-hidden="true">
                        <span class="material-symbols-rounded">"auto_awesome"</span>
                    </div>
                    <div class="nav-rail-items">
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Chats
                            data-testid="nav-chats"
                            on:click=move |_| {
                                set_nav_mode.set(NavMode::Chats);
                                set_context_open.set(false);
                                set_sidebar_open.set(true);
                            }
                        >
                            <span class="material-symbols-rounded" aria-hidden="true">"chat"</span>
                            <span>"Chats"</span>
                        </button>
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Details && context_open.get()
                            data-testid="nav-details"
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
                            <span class="material-symbols-rounded" aria-hidden="true">"info"</span>
                            <span>"Details"</span>
                        </button>
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Jobs
                            data-testid="nav-jobs"
                            on:click=move |_| {
                                set_nav_mode.set(NavMode::Jobs);
                                set_context_open.set(false);
                                set_sidebar_open.set(true);
                            }
                        >
                            <span class="material-symbols-rounded" aria-hidden="true">"work_history"</span>
                            <span>"Jobs"</span>
                        </button>
                    </div>
                    <div class="nav-rail-spacer"></div>
                </nav>
                <button
                    type="button"
                    class="sidebar-backdrop"
                    data-testid="sidebar-backdrop"
                    class:open=move || sidebar_open.get()
                    on:click=move |_| set_sidebar_open.set(false)
                ></button>
                <div class="sidebar-shell" class:open=move || sidebar_open.get()>
                <section class="panel sidebar">
                    <div class="sidebar-header">
                        <div>
                            <h1>{move || match nav_mode.get() {
                                NavMode::Jobs => "Jobs",
                                _ => "Chats",
                            }}</h1>
                        </div>
                        <div class="sidebar-status compact">
                            <p class="status">{move || status_text.get()}</p>
                        </div>
                        <button
                            type="button"
                            class="sidebar-close"
                            on:click=move |_| set_sidebar_open.set(false)
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="sidebar-view" class:hidden=move || nav_mode.get() != NavMode::Chats data-testid="thread-nav-panel">
                    <details class="context-panel">
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
                                        if is_compact_layout() {
                                            set_sidebar_open.set(false);
                                        }
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
                                        set_status_text
                                            .set(format!("Failed to create thread: {error}"));
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
                                                if is_compact_layout() {
                                                    set_sidebar_open.set(false);
                                                }
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
                    <details class="context-panel" open>
                        <summary>{move || format!("Global Jobs ({})", jobs.get().len())}</summary>
                        <div class="context-panel-body">
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
                                            class=("compact", true)
                                            class:active=move || selected_job_id.get() == Some(active_job_id.clone())
                                            on:click=move |_| {
                                                set_preferred_job_id.set(Some(click_job_id.clone()));
                                                set_selected_job_id.set(Some(click_job_id.clone()));
                                                set_selected_thread_id.set(Some(click_thread_id.clone()));
                                                set_nav_mode.set(NavMode::Chats);
                                                set_context_open.set(true);
                                                if is_compact_layout() {
                                                    set_sidebar_open.set(false);
                                                }
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
                                            <p>"No jobs have been created yet."</p>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }
                            }}
                        </div>
                        </div>
                    </details>
                    </div>
                </section>
                </div>
                <section class="panel content">
                    <button
                        type="button"
                        class="context-backdrop"
                        data-testid="context-backdrop"
                        class:open=move || context_open.get()
                        on:click=move |_| {
                            set_context_open.set(false);
                            set_nav_mode.set(NavMode::Chats);
                        }
                    ></button>
                    <div class="content-toolbar">
                        <button
                            type="button"
                            class="sidebar-toggle"
                            on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                        >
                            {move || if sidebar_open.get() { "Hide Threads" } else { "Show Threads" }}
                        </button>
                        {move || {
                            if selected_thread.get().is_some() {
                                view! {
                                    <button
                                        type="button"
                                        class="context-toggle"
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
                                        {move || if context_open.get() { "Hide Details" } else { "Details" }}
                                    </button>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                    </div>
                    {move || {
                        if nav_mode.get() == NavMode::Jobs {
                            let global_jobs = jobs.get();
                            view! {
                                <section class="job-browser" data-testid="job-browser">
                                    <div class="job-browser-header">
                                        <div>
                                            <p class="eyebrow">"Jobs"</p>
                                            <h2>"Job history"</h2>
                                            <p class="status">"Select a job to return to its chat with details open."</p>
                                        </div>
                                        <span class="thread-pill">{format!("{} jobs", global_jobs.len())}</span>
                                    </div>
                                    <div class="job-browser-grid">
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
                                                        class=("intent-card", true)
                                                        class:active=move || selected_job_id.get() == Some(active_job_id.clone())
                                                        on:click=move |_| {
                                                            set_preferred_job_id.set(Some(click_job_id.clone()));
                                                            set_selected_job_id.set(Some(click_job_id.clone()));
                                                            set_selected_thread_id.set(Some(click_thread_id.clone()));
                                                            set_nav_mode.set(NavMode::Chats);
                                                            set_context_open.set(true);
                                                            if is_compact_layout() {
                                                                set_sidebar_open.set(false);
                                                            }
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
                                </section>
                            }.into_any()
                        } else if let Some(thread) = selected_thread.get() {
                            let thread_id = thread.thread.id.clone();
                            let job_thread_id = thread_id.clone();
                            let message_actions_thread_id = thread_id.clone();
                            let chat_submit_thread_id = thread_id.clone();
                            let pending_message_thread_id = thread_id.clone();
                            let pending_indicator_thread_id = thread_id.clone();
                            let thread_record = thread.thread.clone();
                            let jobs = thread.jobs.clone();
                            let messages = thread.messages.clone();
                            let thread_notes = thread.related_notes.clone();
                            let has_jobs = !jobs.is_empty();
                            let active_job_id = selected_job_id.get();

                            view! {
                                <div class="thread-focus" class:details-open=move || context_open.get()>
                                    <section class="thread-hero">
                                        <div class="thread-hero-header">
                                            <div>
                                                <p class="eyebrow">"Conversation"</p>
                                                <h2>{thread_record.title.clone()}</h2>
                                                <div class="thread-mobile-meta">
                                                    <strong>{format!("{} messages", messages.len())}</strong>
                                                    <span>{format!("{} jobs", jobs.len())}</span>
                                                    <span>{format!("Updated {}", thread_record.updated_at.clone())}</span>
                                                </div>
                                            </div>
                                            <span class=format!(
                                                "status-badge {}",
                                                status_badge_class(&thread_record.status)
                                            )>
                                                {thread_record.status.clone()}
                                            </span>
                                            <button
                                                type="button"
                                                class="thread-details-button"
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
                                                {move || if context_open.get() { "Hide Details" } else { "Details" }}
                                            </button>
                                        </div>
                                        <div class="thread-summary-row">
                                            <span class="thread-pill">{format!("{} messages", messages.len())}</span>
                                            <span class="thread-pill">{format!("{} jobs", jobs.len())}</span>
                                            <span class="thread-pill">{format!("{} notes", thread_notes.len())}</span>
                                            <span class="thread-pill">{format!("Updated {}", thread_record.updated_at)}</span>
                                        </div>
                                    </section>

                                    <div class="context-shell" class:open=move || context_open.get() data-testid="context-sheet">
                                    <div class="context-shell-header">
                                        <h3>"Conversation Details"</h3>
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
                                    <details class="context-panel">
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
                                                    <p>"Conversation is the default. Use the explicit dispatch controls in the thread when you want to create a laptop job."</p>
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
                                                <details class="context-panel">
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
                                                            view! {
                                                                <span class=format!("status-badge {}", result_class)>
                                                                    {result}
                                                                </span>
                                                            }
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
                                                                                                let _ = sync_selected_job(
                                                                                                    job_id,
                                                                                                    set_selected_job_detail,
                                                                                                    set_status_text,
                                                                                                )
                                                                                                .await;
                                                                                                let _ = sync_selected_thread(
                                                                                                    thread_id,
                                                                                                    set_selected_thread,
                                                                                                    set_status_text,
                                                                                                )
                                                                                                .await;
                                                                                            }
                                                                                            Err(error) => {
                                                                                                set_status_text.set(format!("Failed to promote note: {error}"));
                                                                                            }
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
                                                                        let summary_text = approval.summary.clone();
                                                                        let resolved_meta = if let Some(resolved_at) = approval.resolved_at.clone() {
                                                                            format!(
                                                                                "{} by {}",
                                                                                resolved_at,
                                                                                approval.resolved_by.clone().unwrap_or_else(|| "unknown".to_string())
                                                                            )
                                                                        } else {
                                                                            "Awaiting resolution".to_string()
                                                                        };
                                                                        let approval_note = approval_status_note(&approval.status, &job_detail.job.status);

                                                                        view! {
                                                                            <article class=("approval-card", true) class:pending=is_pending>
                                                                                <header>
                                                                                    <strong>{format!("{} approval", approval.action_type)}</strong>
                                                                                    <span class=format!(
                                                                                        "status-badge {}",
                                                                                        status_badge_class(&approval.status)
                                                                                    )>
                                                                                        {approval.status.clone()}
                                                                                    </span>
                                                                                </header>
                                                                                <p>{summary_text}</p>
                                                                                <p class="approval-note">{approval_note}</p>
                                                                                <p class="status">{resolved_meta}</p>
                                                                                <p class="status">
                                                                                    {approval.resolution_reason.clone().unwrap_or_else(|| "No resolution note.".to_string())}
                                                                                </p>
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
                                                                                                                    set_status_text.set("Push approval sent to the laptop. Waiting for the push result.".to_string());
                                                                                                                    let _ = sync_selected_job(
                                                                                                                        approval_job_id,
                                                                                                                        set_selected_job_detail,
                                                                                                                        set_status_text,
                                                                                                                    )
                                                                                                                    .await;
                                                                                                                    let _ = sync_selected_thread(
                                                                                                                        thread_id,
                                                                                                                        set_selected_thread,
                                                                                                                        set_status_text,
                                                                                                                    )
                                                                                                                    .await;
                                                                                                                }
                                                                                                                Err(error) => {
                                                                                                                    set_status_text.set(format!("Failed to approve: {error}"));
                                                                                                                }
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
                                                                                                                    let _ = sync_selected_job(
                                                                                                                        approval_job_id,
                                                                                                                        set_selected_job_detail,
                                                                                                                        set_status_text,
                                                                                                                    )
                                                                                                                    .await;
                                                                                                                    let _ = sync_selected_thread(
                                                                                                                        thread_id,
                                                                                                                        set_selected_thread,
                                                                                                                        set_status_text,
                                                                                                                    )
                                                                                                                    .await;
                                                                                                                }
                                                                                                                Err(error) => {
                                                                                                                    set_status_text.set(format!("Failed to reject: {error}"));
                                                                                                                }
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
                                                <details class="context-panel">
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
                                                        set_status_text.set(format!(
                                                            "Job {} is {}.",
                                                            job.short_id, job.status
                                                        ));

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
                                                        set_status_text.set(format!(
                                                            "Failed to create job: {error}"
                                                        ));
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

                                    </div>

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
                                                let (draft_title, set_draft_title) = signal(
                                                    execution_draft
                                                        .as_ref()
                                                        .map(|draft| draft.title.clone())
                                                        .unwrap_or_default(),
                                                );
                                                let (draft_repo_name, set_draft_repo_name) = signal(
                                                    execution_draft
                                                        .as_ref()
                                                        .and_then(|draft| draft.repo_name.clone())
                                                        .unwrap_or_default(),
                                                );
                                                let (draft_base_branch, set_draft_base_branch) = signal(
                                                    execution_draft
                                                        .as_ref()
                                                        .map(|draft| draft.base_branch.clone())
                                                        .unwrap_or_else(|| "main".to_string()),
                                                );
                                                let (draft_request_text, set_draft_request_text) = signal(
                                                    execution_draft
                                                        .as_ref()
                                                        .map(|draft| draft.request_text.clone())
                                                        .unwrap_or_default(),
                                                );
                                                let draft_execution_intent = execution_draft
                                                    .as_ref()
                                                    .map(|draft| draft.execution_intent.clone())
                                                    .unwrap_or(ExecutionIntent::WorkspaceChange);
                                                view! {
                                                    <article class=format!(
                                                        "message {} {}",
                                                        message.role, message_mode_class
                                                    )>
                                                        <header class="message-header">
                                                            <div class="message-header-main">
                                                                <strong>{message.role.clone()}</strong>
                                                                {message_mode_badge.map(|(badge_class, label)| {
                                                                    view! {
                                                                        <span class=format!(
                                                                            "mode-badge {}",
                                                                            badge_class
                                                                        )>
                                                                            {label}
                                                                        </span>
                                                                    }
                                                                })}
                                                            </div>
                                                            <span>{message.created_at.clone()}</span>
                                                        </header>
                                                        <p class="message-body">{message.content.clone()}</p>
                                                        {result_details.map(|details| {
                                                            view! {
                                                                <details class="message-details">
                                                                    <summary>"More Details"</summary>
                                                                    <pre>{details}</pre>
                                                                </details>
                                                            }
                                                        })}
                                                        {execution_draft
                                                            .clone()
                                                            .map(|draft| {
                                                                let message_thread_id = message_actions_thread_id.clone();
                                                                let source_message_id = message_id.clone();
                                                                let source_role = draft.source_role.clone();
                                                                let rationale = draft.rationale.clone();
                                                                view! {
                                                                    <section class="execution-draft">
                                                                        <header>
                                                                            <div>
                                                                                <h4>"Execution Draft"</h4>
                                                                                <p class="draft-rationale">{rationale}</p>
                                                                            </div>
                                                                            <span>{format!(
                                                                                "From {} message {}",
                                                                                draft.source_role,
                                                                                draft.source_message_id
                                                                            )}</span>
                                                                        </header>
                                                                        <div class="thread-meta">
                                                                            <span>"Execution Mode"</span>
                                                                            <span>{execution_intent_label(&draft_execution_intent)}</span>
                                                                        </div>
                                                                        <div class="draft-grid">
                                                                            <label class="draft-field">
                                                                                <strong>"Title"</strong>
                                                                                <input
                                                                                    type="text"
                                                                                    prop:value=move || draft_title.get()
                                                                                    on:input=move |ev| set_draft_title.set(event_target_value(&ev))
                                                                                />
                                                                            </label>
                                                                            <label class="draft-field">
                                                                                <strong>"Repository"</strong>
                                                                                <input
                                                                                    type="text"
                                                                                    prop:value=move || draft_repo_name.get()
                                                                                    on:input=move |ev| set_draft_repo_name.set(event_target_value(&ev))
                                                                                />
                                                                            </label>
                                                                            <label class="draft-field">
                                                                                <strong>"Base Branch"</strong>
                                                                                <input
                                                                                    type="text"
                                                                                    prop:value=move || draft_base_branch.get()
                                                                                    on:input=move |ev| set_draft_base_branch.set(event_target_value(&ev))
                                                                                />
                                                                            </label>
                                                                        </div>
                                                                        <label class="draft-field">
                                                                            <strong>"Request Text"</strong>
                                                                            <textarea
                                                                                prop:value=move || draft_request_text.get()
                                                                                on:input=move |ev| set_draft_request_text.set(event_target_value(&ev))
                                                                            />
                                                                        </label>
                                                                        <div class="draft-actions">
                                                                            <button
                                                                                type="button"
                                                                                on:click={
                                                                                    move |_| {
                                                                                        let title = draft_title.get_untracked().trim().to_string();
                                                                                        let repo_name = draft_repo_name.get_untracked().trim().to_string();
                                                                                        let base_branch = draft_base_branch.get_untracked().trim().to_string();
                                                                                        let request_text = draft_request_text.get_untracked().trim().to_string();
                                                                                        if repo_name.is_empty() {
                                                                                            set_status_text.set("Repository is required before dispatching a draft.".to_string());
                                                                                            return;
                                                                                        }
                                                                                        if request_text.is_empty() {
                                                                                            set_status_text.set("Request text is required before dispatching a draft.".to_string());
                                                                                            return;
                                                                                        }
                                                                                        let execution_intent = draft_execution_intent.clone();

                                                                                        spawn_local({
                                                                                            let set_selected_thread = set_selected_thread;
                                                                                            let set_preferred_job_id = set_preferred_job_id;
                                                                                            let set_selected_job_id = set_selected_job_id;
                                                                                            let set_status_text = set_status_text;
                                                                                            let set_threads = set_threads;
                                                                                            let set_jobs = set_jobs;
                                                                                            let selected_thread_id = selected_thread_id;
                                                                                            let set_selected_thread_id = set_selected_thread_id;
                                                                                            let thread_id = message_thread_id.clone();
                                                                                            let source_message_id = source_message_id.clone();
                                                                                            let source_role = source_role.clone();

                                                                                            async move {
                                                                                                match dispatch_thread_message(
                                                                                                    &thread_id,
                                                                                                    &source_message_id,
                                                                                                    &title,
                                                                                                    &repo_name,
                                                                                                    &base_branch,
                                                                                                    Some(request_text),
                                                                                                    Some(execution_intent),
                                                                                                )
                                                                                                .await
                                                                                                {
                                                                                                    Ok(job) => {
                                                                                                        set_preferred_job_id.set(Some(job.id.clone()));
                                                                                                        set_selected_job_id.set(Some(job.id.clone()));
                                                                                                        set_status_text.set(format!(
                                                                                                            "Promoted {} draft into job {}.",
                                                                                                            source_role,
                                                                                                            job.short_id
                                                                                                        ));
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
                                                                                                        set_status_text.set(format!(
                                                                                                            "Failed to dispatch execution draft: {error}"
                                                                                                        ));
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                    }
                                                                                }
                                                                            >
                                                                                "Dispatch Draft"
                                                                            </button>
                                                                            <span>"Review the fields here, then explicitly promote the draft into Workflow #1."</span>
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
                                                                            let message_thread_id = message_actions_thread_id.clone();
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
                                                                                    let thread_id = message_thread_id.clone();
                                                                                    let source_message_id = source_message_id.clone();
                                                                                    let source_role = source_role.clone();

                                                                                    async move {
                                                                                        match dispatch_thread_message(&thread_id, &source_message_id, &title, &repo_name, &base_branch, None, None).await {
                                                                                            Ok(job) => {
                                                                                                set_preferred_job_id.set(Some(job.id.clone()));
                                                                                                set_selected_job_id.set(Some(job.id.clone()));
                                                                                                set_status_text.set(format!(
                                                                                                    "Escalated {} message into job {}.",
                                                                                                    source_role,
                                                                                                    job.short_id
                                                                                                ));
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
                                                                                                set_status_text.set(format!(
                                                                                                    "Failed to dispatch selected message: {error}"
                                                                                                ));
                                                                                            }
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
                                                    </article>
                                                }
                                            }
                                        />
                                        {move || {
                                            pending_chat_submission
                                                .get()
                                                .filter(|pending| pending.thread_id == pending_message_thread_id)
                                                .map(|pending| {
                                                    view! {
                                                        <article class="message user pending">
                                                            <header class="message-header">
                                                                <div class="message-header-main">
                                                                    <strong>"user"</strong>
                                                                </div>
                                                                <span>"Sending..."</span>
                                                            </header>
                                                            <p class="message-body">{pending.content}</p>
                                                        </article>
                                                    }
                                                })
                                        }}
                                        {move || {
                                            pending_chat_submission
                                                .get()
                                                .filter(|pending| pending.thread_id == pending_indicator_thread_id)
                                                .map(|_| {
                                                    view! {
                                                        <div class="chat-response-indicator" role="status" aria-live="polite">
                                                            <span>"Elowen is responding"</span>
                                                            <span class="chat-response-indicator-dots" aria-hidden="true">
                                                                <span class="chat-response-indicator-dot"></span>
                                                                <span class="chat-response-indicator-dot"></span>
                                                                <span class="chat-response-indicator-dot"></span>
                                                            </span>
                                                        </div>
                                                    }
                                                })
                                        }}
                                    </div>
                                    </div>
                                    <form class="thread-composer" data-testid="thread-composer" on:submit=move |ev: ev::SubmitEvent| {
                                        ev.prevent_default();
                                        let content = new_message_content.get_untracked().trim().to_string();
                                        if content.is_empty() {
                                            set_status_text.set("Message content is required.".to_string());
                                            return;
                                        }

                                        set_new_message_content.set(String::new());
                                        set_pending_chat_submission.set(Some(PendingChatSubmission {
                                            thread_id: chat_submit_thread_id.clone(),
                                            content: content.clone(),
                                        }));
                                        set_status_text.set("Waiting for Elowen to respond...".to_string());

                                        spawn_local({
                                            let set_new_message_content = set_new_message_content;
                                            let set_pending_chat_submission = set_pending_chat_submission;
                                            let set_selected_thread = set_selected_thread;
                                            let set_status_text = set_status_text;
                                            let set_threads = set_threads;
                                            let selected_thread_id = selected_thread_id;
                                            let set_selected_thread_id = set_selected_thread_id;
                                            let thread_id = chat_submit_thread_id.clone();
                                            let request_content = content.clone();

                                            async move {
                                                match send_thread_chat_message(&thread_id, &content).await {
                                                    Ok(reply) => {
                                                        apply_chat_reply_to_selected_thread(
                                                            set_selected_thread,
                                                            &thread_id,
                                                            &reply,
                                                        );
                                                        clear_pending_chat_submission(
                                                            set_pending_chat_submission,
                                                            &thread_id,
                                                            &request_content,
                                                        );
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
                                                        clear_pending_chat_submission(
                                                            set_pending_chat_submission,
                                                            &thread_id,
                                                            &request_content,
                                                        );
                                                        set_new_message_content.set(request_content);
                                                        set_status_text
                                                            .set(format!("Failed to send chat message: {error}"));
                                                    }
                                                }
                                            }
                                        });
                                    }>
                                        <div class="composer-input-wrap">
                                            <textarea
                                                placeholder="Message Elowen"
                                                prop:value=move || new_message_content.get()
                                                prop:disabled=move || pending_chat_submission.get().is_some()
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
                                            <button
                                                type="submit"
                                                class="composer-send"
                                                aria-label="Send message"
                                                prop:disabled=move || pending_chat_submission.get().is_some()
                                            >
                                                <span class="material-symbols-rounded" aria-hidden="true">"arrow_circle_up"</span>
                                            </button>
                                        </div>
                                    </form>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="empty">
                                    <p class="eyebrow">"No Thread Selected"</p>
                                    <h2>"Create or choose a thread"</h2>
                                    <p>"Select a thread or choose a job from the global jobs list to view runtime execution progress."</p>
                                </div>
                            }.into_any()
                        }
                    }}
                </section>
            </div>
            </div>
                    }.into_any(),
                }
            }}
        </main>
    }
}

fn is_compact_layout() -> bool {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .map(|width| width <= 920.0)
        .unwrap_or(false)
}
