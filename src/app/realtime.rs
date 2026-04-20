use gloo_timers::callback::Timeout;
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource, MessageEvent};

use crate::{
    api::{
        events_url, fetch_devices, fetch_job, fetch_jobs, fetch_repositories, fetch_thread,
        fetch_threads,
    },
    models::{
        AuthSessionStatus, DeviceRecord, JobDetail, JobRecord, RepositoryOption, ThreadDetail,
        ThreadSummary, UiEvent,
    },
};

use super::state::{RealtimeStatus, UiEventSyncHandles};

const REALTIME_DEGRADED_MESSAGE: &str =
    "Realtime stream unavailable. Polling fallback remains active.";

pub(super) fn connect_ui_event_stream(handles: UiEventSyncHandles) {
    close_active_event_source(&handles);

    let Ok(source) = EventSource::new(&events_url()) else {
        schedule_reconnect(handles, REALTIME_DEGRADED_MESSAGE.to_string());
        return;
    };

    let on_open_handles = handles.clone();
    let on_open = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        let handles = on_open_handles.clone();
        spawn_local(async move {
            match sync_realtime_catch_up(handles.clone()).await {
                Ok(()) => {
                    handles.runtime.reconnect_controller.borrow_mut().on_open();
                    handles.set_realtime_status.set(RealtimeStatus::Connected);
                    handles
                        .set_status_text
                        .set("Realtime state synced.".to_string());
                }
                Err(error) => {
                    if is_auth_error(&error) {
                        expire_auth_session(handles);
                    } else {
                        schedule_reconnect(
                            handles,
                            format!("Failed to restore realtime state: {error}"),
                        );
                    }
                }
            }
        });
    }));
    source.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    let on_message_handles = handles.clone();
    let on_message =
        Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |message: MessageEvent| {
            let Some(data) = message.data().as_string() else {
                return;
            };
            let Ok(ui_event) = serde_json::from_str::<UiEvent>(&data) else {
                return;
            };

            let handles = on_message_handles.clone();
            spawn_local(async move {
                if let Err(error) = refresh_for_ui_event(ui_event, handles.clone()).await {
                    if is_auth_error(&error) {
                        expire_auth_session(handles);
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

    let on_error_handles = handles.clone();
    let on_error = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        schedule_reconnect(
            on_error_handles.clone(),
            REALTIME_DEGRADED_MESSAGE.to_string(),
        );
    }));
    source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    handles.set_event_source.set(Some(source));
}

pub(super) fn stop_realtime_updates(handles: UiEventSyncHandles, status: RealtimeStatus) {
    cancel_reconnect_timer(&handles);
    close_active_event_source(&handles);
    handles
        .runtime
        .reconnect_controller
        .borrow_mut()
        .disconnect();
    handles.set_realtime_status.set(status);
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
            sync_device_list(handles.set_devices).await?;
            sync_repository_list(handles.set_repositories).await?;
            sync_selected_resources_for_current_thread(handles).await?;
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

async fn sync_realtime_catch_up(handles: UiEventSyncHandles) -> Result<(), String> {
    sync_thread_list_quiet(
        handles.set_threads,
        handles.selected_thread_id,
        handles.set_selected_thread_id,
    )
    .await?;
    sync_job_list(handles.set_jobs).await?;
    sync_device_list(handles.set_devices).await?;
    sync_repository_list(handles.set_repositories).await?;

    if let Some(thread_id) = handles.selected_thread_id.get_untracked() {
        sync_selected_thread_quiet(thread_id, handles.set_selected_thread).await?;
    }

    if let Some(job_id) = handles.selected_job_id.get_untracked() {
        sync_selected_job_quiet(job_id, handles.set_selected_job_detail).await?;
    }

    Ok(())
}

async fn sync_selected_resources_for_current_thread(
    handles: UiEventSyncHandles,
) -> Result<(), String> {
    if let Some(thread_id) = handles.selected_thread_id.get_untracked() {
        sync_selected_thread_quiet(thread_id, handles.set_selected_thread).await?;
        sync_thread_list_quiet(
            handles.set_threads,
            handles.selected_thread_id,
            handles.set_selected_thread_id,
        )
        .await?;
    }

    if let Some(job_id) = handles.selected_job_id.get_untracked() {
        sync_selected_job_quiet(job_id, handles.set_selected_job_detail).await?;
    }

    Ok(())
}

fn schedule_reconnect(handles: UiEventSyncHandles, message: String) {
    close_active_event_source(&handles);
    handles.set_realtime_status.set(RealtimeStatus::Degraded);
    handles.set_status_text.set(message);

    let delay = {
        handles
            .runtime
            .reconnect_controller
            .borrow_mut()
            .schedule_retry()
    };

    let Some(delay) = delay else {
        return;
    };

    cancel_reconnect_timer(&handles);

    let timer_handles = handles.clone();
    let timer = Timeout::new(delay, move || {
        timer_handles.runtime.reconnect_timer.borrow_mut().take();

        let should_connect = {
            timer_handles
                .runtime
                .reconnect_controller
                .borrow_mut()
                .retry_fired()
        };

        if !should_connect {
            return;
        }

        timer_handles
            .set_realtime_status
            .set(RealtimeStatus::Connecting);
        connect_ui_event_stream(timer_handles.clone());
    });

    *handles.runtime.reconnect_timer.borrow_mut() = Some(timer);
}

fn cancel_reconnect_timer(handles: &UiEventSyncHandles) {
    handles.runtime.reconnect_timer.borrow_mut().take();
}

fn close_active_event_source(handles: &UiEventSyncHandles) {
    handles.event_source.with_untracked(|source| {
        if let Some(source) = source {
            source.close();
        }
    });
    handles.set_event_source.set(None);
}

fn expire_auth_session(handles: UiEventSyncHandles) {
    stop_realtime_updates(handles.clone(), RealtimeStatus::Disconnected);
    handles.set_auth_session.set(Some(AuthSessionStatus {
        enabled: true,
        auth_mode: crate::models::AuthMode::LocalAccounts,
        authenticated: false,
        actor: None,
        permissions: Vec::new(),
    }));
    handles.set_selected_thread.set(None);
    handles.set_selected_job_id.set(None);
    handles.set_selected_job_detail.set(None);
    handles.set_devices.set(Vec::new());
    handles.set_repositories.set(Vec::new());
    handles
        .set_status_text
        .set("Session expired. Sign in again.".to_string());
}

pub(super) async fn sync_thread_list(
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

pub(super) async fn sync_job_list(set_jobs: WriteSignal<Vec<JobRecord>>) -> Result<(), String> {
    set_jobs.set(fetch_jobs().await?);
    Ok(())
}

pub(super) async fn sync_device_list(
    set_devices: WriteSignal<Vec<DeviceRecord>>,
) -> Result<(), String> {
    set_devices.set(fetch_devices().await?);
    Ok(())
}

pub(super) async fn sync_repository_list(
    set_repositories: WriteSignal<Vec<RepositoryOption>>,
) -> Result<(), String> {
    let repositories = fetch_repositories().await?;
    set_repositories.set(repositories);
    Ok(())
}

pub(super) async fn sync_selected_thread(
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

pub(super) async fn sync_selected_job(
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

pub(super) fn is_auth_error(error: &str) -> bool {
    error.contains("sign in required") || error.contains("status 401")
}
