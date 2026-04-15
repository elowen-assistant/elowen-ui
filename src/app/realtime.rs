use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource, MessageEvent};

use crate::{
    api::{events_url, fetch_job, fetch_jobs, fetch_thread, fetch_threads},
    models::{JobDetail, JobRecord, ThreadDetail, ThreadSummary, UiEvent},
};

use super::{
    session::apply_session_expired_state,
    state::{RealtimeStatus, UiEventSyncHandles},
};

pub(super) fn connect_ui_event_stream(handles: UiEventSyncHandles) {
    let Ok(source) = EventSource::new(&events_url()) else {
        handles.set_realtime_status.set(RealtimeStatus::Degraded);
        handles
            .set_status_text
            .set("Realtime stream unavailable. Polling fallback remains active.".to_string());
        return;
    };

    let on_open = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        spawn_local(async move {
            if let Err(error) = sync_realtime_catch_up(handles).await {
                if is_auth_error(&error) {
                    apply_session_expired_state(
                        handles.set_auth_session,
                        handles.set_selected_thread,
                        handles.set_selected_job_detail,
                        handles.set_status_text,
                    );
                } else {
                    handles.set_realtime_status.set(RealtimeStatus::Degraded);
                    handles
                        .set_status_text
                        .set(format!("Failed to refresh realtime state: {error}"));
                }
            } else {
                handles.set_realtime_status.set(RealtimeStatus::Connected);
            }
        });
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
                        apply_session_expired_state(
                            handles.set_auth_session,
                            handles.set_selected_thread,
                            handles.set_selected_job_detail,
                            handles.set_status_text,
                        );
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

            if let Some(thread_id) = handles.selected_thread_id.get_untracked()
                && event.thread_id.as_ref() == Some(&thread_id)
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
    sync_selected_resources_for_current_thread(handles).await?;
    Ok(())
}

async fn sync_selected_resources_for_current_thread(
    handles: UiEventSyncHandles,
) -> Result<(), String> {
    if let Some(thread_id) = handles.selected_thread_id.get_untracked() {
        sync_selected_thread_quiet(thread_id, handles.set_selected_thread).await?;
    }

    if let Some(job_id) = handles.selected_job_id.get_untracked() {
        sync_selected_job_quiet(job_id, handles.set_selected_job_detail).await?;
    }

    Ok(())
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
