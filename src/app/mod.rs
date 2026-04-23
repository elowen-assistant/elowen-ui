mod auth;
mod details;
mod jobs;
mod layout;
mod realtime;
mod reconnect;
mod state;
mod storage;
mod threads;

use gloo_timers::future::TimeoutFuture;
use leptos::{ev, html, prelude::*, task::spawn_local};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use crate::{
    api::{
        create_job, create_thread, dispatch_thread_message, fetch_auth_session,
        login as login_session, logout as logout_session, promote_job_note, resolve_approval,
        send_thread_chat_message,
    },
    format::{
        approval_status_note, execution_intent_label, format_json_value, format_string_list,
        job_target_label, message_execution_draft, message_is_result_surface, message_mode_badge,
        message_mode_class, message_result_details, message_timestamp_label, report_array_strings,
        report_diff_stat, report_last_message, report_status_label, status_badge_class,
    },
    models::*,
};

use self::{
    auth::{
        auth_loading_message, auth_prompt, password_placeholder, protected_workspace_label,
        username_placeholder,
    },
    details::details_toggle_label,
    jobs::{job_count_label, short_thread_label},
    layout::{default_workspace_subtitle, default_workspace_title},
    realtime::{
        connect_ui_event_stream, is_auth_error, stop_realtime_updates, sync_device_list,
        sync_job_list, sync_repository_list, sync_selected_job, sync_selected_thread,
        sync_thread_list,
    },
    state::{
        NavMode, POLL_FALLBACK_MS, RealtimeRuntime, RealtimeStatus, STORAGE_COMPOSER_TEXT,
        STORAGE_CONTEXT_OPEN, STORAGE_NAV_MODE, STORAGE_SELECTED_JOB_ID,
        STORAGE_SELECTED_THREAD_ID, UiEventSyncHandles,
    },
    storage::{read_bool_storage, read_storage, write_optional_storage, write_storage},
    threads::thread_message_count_label,
};

#[derive(Clone)]
struct PendingChatSubmission {
    thread_id: String,
    content: String,
}

#[derive(Clone)]
struct DraftEditState {
    device_id: String,
    target_name: String,
    base_branch: String,
    prompt: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextTab {
    Thread,
    Devices,
    Job,
    Manual,
}

fn session_can_access(session: &AuthSessionStatus) -> bool {
    !session.enabled || session.authenticated
}

fn session_has_permission(session: &AuthSessionStatus, permission: AuthPermission) -> bool {
    !session.enabled || session.permissions.contains(&permission)
}

fn session_can_operate(session: &AuthSessionStatus) -> bool {
    session_has_permission(session, AuthPermission::Operate)
}

fn session_can_admin(session: &AuthSessionStatus) -> bool {
    session_has_permission(session, AuthPermission::Admin)
}

fn auth_role_label(role: &AuthRole) -> &'static str {
    match role {
        AuthRole::Viewer => "Viewer",
        AuthRole::Operator => "Operator",
        AuthRole::Admin => "Admin",
    }
}

fn actor_chip_label(actor: &SessionActor) -> String {
    format!("{} · {}", actor.display_name, auth_role_label(&actor.role))
}

fn unauthenticated_session(mode: AuthMode) -> AuthSessionStatus {
    AuthSessionStatus {
        enabled: true,
        auth_mode: mode,
        authenticated: false,
        actor: None,
        permissions: Vec::new(),
    }
}

fn device_option_exists(devices: &[DeviceRecord], device_id: &str) -> bool {
    devices.iter().any(|device| device.id == device_id)
}

fn selected_device(devices: &[DeviceRecord], device_id: &str) -> Option<DeviceRecord> {
    devices
        .iter()
        .find(|device| device.id == device_id)
        .cloned()
}

fn device_trust_status_key(trust: &DeviceTrustRecord) -> &str {
    let status = trust.status.trim();
    if status.is_empty() {
        "unreported"
    } else {
        status
    }
}

fn device_trust_status_class(trust: &DeviceTrustRecord) -> &'static str {
    match device_trust_status_key(trust) {
        "trusted" => "trusted",
        "rotated" => "rotated",
        "revoked" => "revoked",
        "untrusted" => "untrusted",
        "attention_needed" | "needs_attention" => "attention",
        _ => "unreported",
    }
}

fn device_trust_status_label(trust: &DeviceTrustRecord) -> String {
    if let Some(label) = trust
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
    {
        return label.to_string();
    }

    match device_trust_status_key(trust) {
        "trusted" => "Trusted".to_string(),
        "rotated" => "Rotated".to_string(),
        "revoked" => "Revoked".to_string(),
        "untrusted" => "Untrusted".to_string(),
        "attention_needed" | "needs_attention" => "Needs Attention".to_string(),
        _ => "Trust Unreported".to_string(),
    }
}

fn device_requires_trust_attention(device: &DeviceRecord) -> bool {
    device.trust.requires_attention
        || matches!(
            device_trust_status_key(&device.trust),
            "revoked" | "untrusted" | "attention_needed" | "needs_attention"
        )
        || matches!(device.trust.can_dispatch, Some(false))
}

fn device_trust_summary(device: &DeviceRecord) -> String {
    device
        .trust
        .summary
        .as_deref()
        .or(device.trust.detail.as_deref())
        .or(device.trust.reason.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| match device_trust_status_key(&device.trust) {
            "trusted" => "Trusted enrollment is active for this edge.".to_string(),
            "rotated" => {
                "Trust material was rotated and should be verified before dispatch.".to_string()
            }
            "revoked" => {
                "Trust has been revoked. Do not rely on this edge until it is re-enrolled."
                    .to_string()
            }
            "untrusted" => "This edge has not completed trusted enrollment yet.".to_string(),
            "attention_needed" | "needs_attention" => {
                "This edge needs trust review before it should be used for sensitive work."
                    .to_string()
            }
            _ => "The API did not include a detailed trust summary for this edge.".to_string(),
        })
}

fn device_enrollment_label(device: &DeviceRecord) -> String {
    match device
        .trust
        .enrollment_kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some("primary") if device.primary_flag => "Primary edge".to_string(),
        Some("primary") => "Primary enrollment".to_string(),
        Some("additional_edge") => "Additional edge".to_string(),
        Some("re_enrollment") => "Re-enrollment".to_string(),
        Some("rotation") => "Key rotation".to_string(),
        Some(value) => value.replace('_', " "),
        None if device.primary_flag => "Primary edge".to_string(),
        None => "Additional edge".to_string(),
    }
}

fn device_trust_timestamps(device: &DeviceRecord) -> Vec<(String, String)> {
    let mut items = vec![("Seen".to_string(), device.last_seen_at.clone())];

    if let Some(value) = device
        .trust
        .last_trusted_registration_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        items.push(("Trusted".to_string(), value.to_string()));
    }

    if let Some(value) = device
        .trust
        .rotated_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        items.push(("Rotated".to_string(), value.to_string()));
    }

    if let Some(value) = device
        .trust
        .revoked_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        items.push(("Revoked".to_string(), value.to_string()));
    }

    items
}

fn device_option_label(device: &DeviceRecord) -> String {
    let mut segments = vec![format!("{} ({})", device.name, device.id)];

    segments.push(device_enrollment_label(device));
    segments.push(device_trust_status_label(&device.trust));

    segments.join(" · ")
}

fn device_trust_card(
    device: Option<DeviceRecord>,
    compact: bool,
    test_id: Option<&'static str>,
) -> impl IntoView {
    match device {
        Some(device) => {
            let trust_label = device_trust_status_label(&device.trust);
            let trust_class = device_trust_status_class(&device.trust);
            let enrollment_label = device_enrollment_label(&device);
            let summary = device_trust_summary(&device);
            let timestamps = device_trust_timestamps(&device);
            let trust_attention = device_requires_trust_attention(&device);
            let dispatch_note = match device.trust.can_dispatch {
                Some(false) => Some("Dispatch should stay blocked until this trust issue is resolved.".to_string()),
                Some(true) if trust_attention => {
                    Some("Dispatch is still possible, but operators should confirm why this edge needs trust attention.".to_string())
                }
                _ => None,
            };

            view! {
                <article
                    class=("device-trust-card", true)
                    class:compact=compact
                    class:attention=trust_attention
                    data-testid=test_id.unwrap_or("")
                >
                    <header>
                        <div>
                            <p class="eyebrow">"Trust State"</p>
                            <h4>{device.name.clone()}</h4>
                            <p class="status">{format!("{} · {}", device.id, enrollment_label)}</p>
                        </div>
                        <span class=format!("trust-badge {}", trust_class)>{trust_label}</span>
                    </header>
                    <p class="device-trust-summary">{summary}</p>
                    <div class="device-trust-meta">
                        <For
                            each=move || timestamps.clone()
                            key=|(label, _)| label.clone()
                            children=move |(label, value)| {
                                view! {
                                    <span>{format!("{label}: {value}")}</span>
                                }
                            }
                        />
                    </div>
                    {dispatch_note.map(|note| {
                        view! { <p class="device-trust-warning">{note}</p> }
                    })}
                </article>
            }
            .into_any()
        }
        None => view! {
            <div class="device-trust-empty">
                <p>"Select a device to review its trust state before dispatching."</p>
            </div>
        }
        .into_any(),
    }
}

fn repositories_for_device(devices: &[DeviceRecord], device_id: &str) -> Vec<DeviceRepository> {
    devices
        .iter()
        .find(|device| device.id == device_id)
        .map(|device| {
            if !device.repositories.is_empty() {
                return device.repositories.clone();
            }

            device
                .discovered_repos
                .iter()
                .map(|repo_name| DeviceRepository {
                    name: repo_name.clone(),
                    branches: Vec::new(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn branches_for_device_repository(
    devices: &[DeviceRecord],
    device_id: &str,
    repo_name: &str,
) -> Vec<String> {
    repositories_for_device(devices, device_id)
        .into_iter()
        .find(|repository| repository.name == repo_name)
        .map(|repository| repository.branches)
        .unwrap_or_default()
}

fn preferred_device_value(
    devices: &[DeviceRecord],
    current_value: &str,
    fallback_repo_name: &str,
) -> String {
    if device_option_exists(devices, current_value) {
        return current_value.to_string();
    }

    if !fallback_repo_name.trim().is_empty()
        && let Some(device) = devices.iter().find(|device| {
            repositories_for_device(devices, &device.id)
                .iter()
                .any(|repository| repository.name == fallback_repo_name)
        })
    {
        return device.id.clone();
    }

    devices
        .first()
        .map(|device| device.id.clone())
        .unwrap_or_default()
}

fn device_has_capability(device: &DeviceRecord, capability_name: &str) -> bool {
    device
        .capabilities
        .iter()
        .any(|capability| capability.trim() == capability_name)
}

fn preferred_capability_device_value(
    devices: &[DeviceRecord],
    current_value: &str,
    capability_name: &str,
) -> String {
    if device_option_exists(devices, current_value)
        && selected_device(devices, current_value)
            .map(|device| device_has_capability(&device, capability_name))
            .unwrap_or(false)
    {
        return current_value.to_string();
    }

    devices
        .iter()
        .find(|device| device_has_capability(device, capability_name))
        .map(|device| device.id.clone())
        .unwrap_or_default()
}

fn preferred_repository_value(
    repositories: &[DeviceRepository],
    current_value: &str,
    fallback_value: &str,
) -> String {
    if repositories
        .iter()
        .any(|repository| repository.name == current_value)
    {
        return current_value.to_string();
    }

    if repositories
        .iter()
        .any(|repository| repository.name == fallback_value)
    {
        return fallback_value.to_string();
    }

    repositories
        .first()
        .map(|repository| repository.name.clone())
        .unwrap_or_default()
}

fn preferred_branch_value(
    branches: &[String],
    current_value: &str,
    fallback_value: &str,
) -> String {
    if branches.iter().any(|branch| branch == current_value) {
        return current_value.to_string();
    }

    if branches.iter().any(|branch| branch == fallback_value) {
        return fallback_value.to_string();
    }

    branches
        .first()
        .cloned()
        .or_else(|| (!fallback_value.trim().is_empty()).then(|| fallback_value.to_string()))
        .unwrap_or_else(|| "main".to_string())
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

fn build_thread_timeline_messages(
    thread: &ThreadDetail,
    selected_job_detail: Option<&JobDetail>,
) -> Vec<MessageRecord> {
    let mut timeline = thread.messages.clone();
    let mut existing_statuses = timeline
        .iter()
        .map(|message| message.status.clone())
        .collect::<HashSet<_>>();

    for job in &thread.jobs {
        if !thread_message_mentions_job(&timeline, job)
            && let Some(message) = synthetic_job_created_message(job, selected_job_detail)
        {
            existing_statuses.insert(message.status.clone());
            timeline.push(message);
        }

        if let Some(job_detail) = selected_job_detail.filter(|detail| detail.job.id == job.id) {
            for event in &job_detail.events {
                let Some(message) = synthetic_job_event_message(job, job_detail, event) else {
                    continue;
                };

                if existing_statuses.insert(message.status.clone()) {
                    timeline.push(message);
                }
            }
        }
    }

    timeline.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    timeline
}

fn thread_message_mentions_job(messages: &[MessageRecord], job: &JobRecord) -> bool {
    let short_id_ref = format!("job `{}`", job.short_id);
    let id_ref = format!("job `{}`", job.id);
    let status_prefix = format!("job_event:{}:", job.id);

    messages.iter().any(|message| {
        message.status.starts_with(&status_prefix)
            || message
                .payload_json
                .get("job_result")
                .and_then(|value| value.get("job_id"))
                .and_then(Value::as_str)
                == Some(job.id.as_str())
            || (message.status.starts_with("workflow.")
                && (message.content.contains(&short_id_ref) || message.content.contains(&id_ref)))
    })
}

fn synthetic_job_created_message(
    job: &JobRecord,
    selected_job_detail: Option<&JobDetail>,
) -> Option<MessageRecord> {
    let created_event = selected_job_detail
        .filter(|detail| detail.job.id == job.id)
        .and_then(|detail| {
            detail
                .events
                .iter()
                .find(|event| event.event_type == "job.created")
        });
    let created_at = created_event
        .map(|event| event.created_at.clone())
        .unwrap_or_else(|| job.created_at.clone());
    let event_payload = created_event
        .map(|event| event.payload_json.clone())
        .unwrap_or_else(|| json!({}));
    let device_id = event_payload
        .get("device_id")
        .and_then(Value::as_str)
        .or(job.device_id.as_deref())
        .unwrap_or("unassigned");
    let request_text = event_payload
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let status_note = match job.status.as_str() {
        "probing" => "Elowen is checking for an available edge device now.",
        "pending" => "The job is queued and waiting to be dispatched.",
        "dispatched" => "The job has been dispatched to an edge device.",
        "accepted" => "An edge device accepted the job and is preparing to run it.",
        "running" => "The job is now running.",
        "awaiting_approval" => "The job is waiting for approval before the push step.",
        "completed" => "The job has completed.",
        "failed" => "The job failed.",
        _ => "The job was created from this thread.",
    };
    let content = format!(
        "Created job `{}` for {} on device `{}`. {}",
        job.short_id,
        job_target_label(job),
        device_id,
        status_note
    );

    let mut detail_lines = vec![
        format!("Job: {}", job.short_id),
        format!("Status: {}", job.status),
        format!("Target: {}", job_target_label(job)),
        format!("Correlation: {}", job.correlation_id),
    ];
    if let Some(branch_name) = job.branch_name.as_deref().filter(|value| !value.is_empty()) {
        detail_lines.push(format!("Branch: {branch_name}"));
    }
    if let Some(base_branch) = job.base_branch.as_deref().filter(|value| !value.is_empty()) {
        detail_lines.push(format!("Base branch: {base_branch}"));
    }
    if let Some(request_text) = request_text {
        detail_lines.push(format!("Prompt: {request_text}"));
    }

    Some(MessageRecord {
        id: format!("synthetic-job-created-{}", job.id),
        role: "assistant".to_string(),
        content,
        status: format!("job_event:{}:created", job.id),
        payload_json: json!({
            "job_result": {
                "job_id": job.id,
                "job_short_id": job.short_id,
                "details": detail_lines.join("\n"),
            },
            "job_event": event_payload,
        }),
        created_at,
    })
}

fn synthetic_job_event_message(
    job: &JobRecord,
    job_detail: &JobDetail,
    event: &JobEventRecord,
) -> Option<MessageRecord> {
    let status_suffix = event.event_type.strip_prefix("job.")?.replace('.', "_");
    if status_suffix == "created" {
        return None;
    }

    let content = match event.event_type.as_str() {
        "job.started" => format!(
            "Started job `{}` on device `{}`.",
            job.short_id,
            job.device_id.as_deref().unwrap_or("unassigned")
        ),
        "job.awaiting_approval" => format!(
            "Job `{}` is waiting for push approval for branch `{}`.",
            job.short_id,
            job.branch_name.as_deref().unwrap_or("unknown")
        ),
        "job.push_started" => format!(
            "Pushing branch `{}` for job `{}`.",
            job.branch_name.as_deref().unwrap_or("unknown"),
            job.short_id
        ),
        "job.push_completed" => format!(
            "Finished pushing branch `{}` for job `{}`.",
            job.branch_name.as_deref().unwrap_or("unknown"),
            job.short_id
        ),
        "job.completed" if job_detail.job.result.as_deref() == Some("success") => job_detail
            .summary
            .as_ref()
            .map(|summary| summary.content.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("Job `{}` completed successfully.", job.short_id)),
        "job.completed" => format!(
            "Job `{}` completed with result `{}`.",
            job.short_id,
            job_detail.job.result.as_deref().unwrap_or("unknown")
        ),
        "job.failed" => format!("Job `{}` failed.", job.short_id),
        _ => format!("Job `{}` reported `{}`.", job.short_id, event.event_type),
    };

    let details = format!(
        "Job: {}\nStatus: {}\nCorrelation: {}\nEvent: {}\nPayload:\n{}",
        job.short_id,
        job_detail.job.status,
        event.correlation_id,
        event.event_type,
        format_json_value(&event.payload_json),
    );

    Some(MessageRecord {
        id: format!("synthetic-job-event-{}", event.id),
        role: "assistant".to_string(),
        content,
        status: format!("job_event:{}:{status_suffix}", job.id),
        payload_json: json!({
            "job_result": {
                "job_id": job.id,
                "job_short_id": job.short_id,
                "details": details,
            },
            "job_event": event.payload_json,
        }),
        created_at: event.created_at.clone(),
    })
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
#[allow(clippy::redundant_locals)]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (jobs, set_jobs) = signal(Vec::<JobRecord>::new());
    let (devices, set_devices) = signal(Vec::<DeviceRecord>::new());
    let (_repositories, set_repositories) = signal(Vec::<RepositoryOption>::new());
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
    let (auth_username, set_auth_username) = signal(String::new());
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
    let (new_job_device, set_new_job_device) = signal(String::new());
    let (new_job_repo, set_new_job_repo) = signal(String::new());
    let (new_job_base_branch, set_new_job_base_branch) = signal(String::from("main"));
    let (new_job_request_text, set_new_job_request_text) = signal(String::new());
    let (status_text, set_status_text) = signal(String::from("Loading threads and jobs..."));
    let (message_pane_pinned, set_message_pane_pinned) = signal(true);
    let (pending_chat_submission, set_pending_chat_submission) =
        signal(None::<PendingChatSubmission>);
    let (draft_edits, set_draft_edits) = signal(HashMap::<String, DraftEditState>::new());
    let (context_tab, set_context_tab) = signal(ContextTab::Thread);
    let (realtime_status, set_realtime_status) = signal(RealtimeStatus::Connecting);
    let (event_source, set_event_source) = signal(None::<EventSource>);
    let realtime_runtime = StoredValue::new_local(RealtimeRuntime::default());
    let message_pane_ref = NodeRef::<html::Div>::new();
    let composer_textarea_ref = NodeRef::<html::Textarea>::new();

    Effect::new(move |_| {
        let _ = selected_thread_id.get();
        set_context_tab.set(ContextTab::Thread);
    });

    Effect::new(move |_| {
        let device_options = devices.get();
        let current_device = new_job_device.get();
        let next_device =
            preferred_device_value(&device_options, &current_device, &new_job_repo.get());
        if next_device != current_device {
            set_new_job_device.set(next_device.clone());
        }

        let repositories = repositories_for_device(&device_options, &next_device);
        let current_repo = new_job_repo.get();
        let next_repo = preferred_repository_value(&repositories, &current_repo, "elowen-api");
        if next_repo != current_repo {
            set_new_job_repo.set(next_repo.clone());
        }

        let branches = branches_for_device_repository(&device_options, &next_device, &next_repo);
        let current_branch = new_job_base_branch.get();
        let next_branch = preferred_branch_value(&branches, &current_branch, "main");
        if next_branch != current_branch {
            set_new_job_base_branch.set(next_branch);
        }
    });

    Effect::new(move |_| {
        let _ = new_message_content.get();

        if let Some(textarea) = composer_textarea_ref.get() {
            let _ = textarea.set_attribute("style", "height: auto; overflow-y: hidden;");

            let viewport_height = web_sys::window()
                .and_then(|window| window.inner_height().ok())
                .and_then(|height| height.as_f64())
                .unwrap_or(720.0);
            let max_height = (viewport_height * 0.5).max(140.0);
            let scroll_height = textarea.scroll_height() as f64;
            let next_height = scroll_height.min(max_height).max(50.0);
            let overflow_y = if scroll_height > max_height {
                "auto"
            } else {
                "hidden"
            };

            let _ = textarea.set_attribute(
                "style",
                &format!("height: {next_height}px; overflow-y: {overflow_y};"),
            );
        }
    });

    spawn_local(async move {
        match fetch_auth_session().await {
            Ok(session) => {
                let can_access = session_can_access(&session);
                let can_operate = session_can_operate(&session);
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

                    if can_operate && let Err(error) = sync_device_list(set_devices).await {
                        set_status_text.set(format!("Failed to load devices: {error}"));
                    }

                    if can_operate && let Err(error) = sync_repository_list(set_repositories).await
                    {
                        set_status_text.set(format!("Failed to load repositories: {error}"));
                    }

                    if let Some(thread_id) = selected_thread_id.get_untracked()
                        && let Err(error) =
                            sync_selected_thread(thread_id, set_selected_thread, set_status_text)
                                .await
                    {
                        set_status_text.set(format!("Failed to load thread: {error}"));
                    }
                } else {
                    set_devices.set(Vec::new());
                    set_repositories.set(Vec::new());
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
                .map(|session| session_can_access(&session))
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
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
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
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll jobs: {error}"));
            }

            let can_operate = auth_session
                .get_untracked()
                .map(|session| session_can_operate(&session))
                .unwrap_or(false);
            if can_operate && let Err(error) = sync_device_list(set_devices).await {
                if is_auth_error(&error) {
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll devices: {error}"));
            }

            if can_operate && let Err(error) = sync_repository_list(set_repositories).await {
                if is_auth_error(&error) {
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll repositories: {error}"));
            }

            if let Some(thread_id) = selected_thread_id.get_untracked()
                && let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
            {
                if is_auth_error(&error) {
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
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
                    stop_realtime_updates(
                        UiEventSyncHandles {
                            selected_thread_id,
                            selected_job_id,
                            event_source,
                            set_threads,
                            set_selected_thread_id,
                            set_selected_thread,
                            set_jobs,
                            set_devices,
                            set_repositories,
                            set_selected_job_id,
                            set_selected_job_detail,
                            set_auth_session,
                            set_status_text,
                            set_realtime_status,
                            set_event_source,
                            runtime: realtime_runtime.get_value(),
                        },
                        RealtimeStatus::Disconnected,
                    );
                    let auth_mode = auth_session
                        .get_untracked()
                        .map(|session| session.auth_mode)
                        .unwrap_or(AuthMode::LocalAccounts);
                    set_auth_session.set(Some(unauthenticated_session(auth_mode)));
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
            .map(|session| session_can_access(&session))
            .unwrap_or(false);

        if can_access {
            realtime_runtime
                .get_value()
                .reconnect_controller
                .borrow_mut()
                .allow_connect();
        }

        let needs_stream = event_source.with_untracked(Option::is_none);
        let can_connect_now = realtime_runtime
            .get_value()
            .reconnect_controller
            .borrow()
            .can_connect_now();

        if can_access && needs_stream && can_connect_now {
            set_realtime_status.set(RealtimeStatus::Connecting);
            connect_ui_event_stream(UiEventSyncHandles {
                selected_thread_id,
                selected_job_id,
                event_source,
                set_threads,
                set_selected_thread_id,
                set_selected_thread,
                set_jobs,
                set_devices,
                set_repositories,
                set_selected_job_id,
                set_selected_job_detail,
                set_auth_session,
                set_status_text,
                set_realtime_status,
                set_event_source,
                runtime: realtime_runtime.get_value(),
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
            .map(|thread| {
                build_thread_timeline_messages(&thread, selected_job_detail.get().as_ref()).len()
            })
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
            {move || {
                match auth_session.get() {
                    None => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <p class="status">{auth_loading_message()}</p>
                            </section>
                        </div>
                    }.into_any(),
                    Some(session) if session.enabled && !session.authenticated => {
                        let auth_mode = session.auth_mode.clone();
                        let requires_username = matches!(auth_mode, AuthMode::LocalAccounts);
                        let auth_status = match auth_mode {
                            AuthMode::LocalAccounts => {
                                "Local account authentication is enabled for this deployment."
                            }
                            AuthMode::LegacySharedPassword => {
                                "Legacy shared-password access is enabled for this deployment."
                            }
                            AuthMode::Disabled => "Authentication is disabled for this deployment.",
                        };
                        view! {
                            <div class="auth-shell">
                                <section class="auth-card">
                                    <p class="eyebrow">{protected_workspace_label()}</p>
                                    <h1>"Sign In To Elowen"</h1>
                                    <p class="status">{auth_prompt(&auth_mode)}</p>
                                    <form data-testid="auth-form" on:submit=move |ev: ev::SubmitEvent| {
                                        ev.prevent_default();
                                        let username = auth_username.get_untracked().trim().to_string();
                                        let password = auth_password.get_untracked();
                                        if requires_username && username.is_empty() {
                                            set_auth_error.set("Username is required.".to_string());
                                            return;
                                        }
                                        if password.trim().is_empty() {
                                            set_auth_error.set("Password is required.".to_string());
                                            return;
                                        }

                                        spawn_local({
                                            let set_auth_error = set_auth_error;
                                            let set_auth_session = set_auth_session;
                                            let set_auth_username = set_auth_username;
                                            let set_auth_password = set_auth_password;
                                            let set_status_text = set_status_text;
                                            let set_threads = set_threads;
                                            let set_jobs = set_jobs;
                                            let set_devices = set_devices;
                                            let set_repositories = set_repositories;
                                            let selected_thread_id = selected_thread_id;
                                            let set_selected_thread_id = set_selected_thread_id;
                                            let set_selected_thread = set_selected_thread;
                                            let login_username = requires_username.then_some(username);
                                            async move {
                                                match login_session(login_username.as_deref(), &password).await {
                                                    Ok(session) => {
                                                        let can_operate = session_can_operate(&session);
                                                        set_auth_error.set(String::new());
                                                        set_auth_username.set(String::new());
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

                                                        if can_operate {
                                                            if let Err(error) = sync_device_list(set_devices).await {
                                                                set_status_text.set(format!("Failed to load devices: {error}"));
                                                            }
                                                            if let Err(error) = sync_repository_list(set_repositories).await {
                                                                set_status_text.set(format!("Failed to load repositories: {error}"));
                                                            }
                                                        } else {
                                                            set_devices.set(Vec::new());
                                                            set_repositories.set(Vec::new());
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
                                        {if requires_username {
                                            view! {
                                                <input
                                                    data-testid="auth-username"
                                                    type="text"
                                                    placeholder=username_placeholder(&auth_mode)
                                                    prop:value=move || auth_username.get()
                                                    on:input=move |ev| set_auth_username.set(event_target_value(&ev))
                                                />
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                        <input
                                            data-testid="auth-password"
                                            type="password"
                                            placeholder=password_placeholder(&auth_mode)
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
                                            <p class="status">{auth_status}</p>
                                            <button type="submit" data-testid="auth-submit">"Sign In"</button>
                                        </div>
                                    </form>
                                </section>
                            </div>
                        }.into_any()
                    },
                    Some(_) => view! {
                        <div class="workspace-shell">
                            <header class="workspace-header">
                                <div class="header-leading">
                                    <button
                                        type="button"
                                        class="header-button"
                                        data-testid="mobile-threads"
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
                                                .unwrap_or_else(|| default_workspace_title().to_string())
                                        }}</h2>
                                        <p class="header-subtitle">
                                            {move || {
                                                if selected_thread.get().is_some() {
                                                    status_text.get()
                                                } else {
                                                    default_workspace_subtitle().to_string()
                                                }
                                            }}
                                        </p>
                                    </div>
                                    <button
                                        type="button"
                                        class="header-button mobile-details-button"
                                        data-testid="mobile-details"
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
                                        <span class="sr-only">"Open details"</span>
                                    </button>
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
                                        <span>{move || details_toggle_label(context_open.get())}</span>
                                    </button>
                                    {move || {
                                        match auth_session.get().and_then(|session| session.actor) {
                                            Some(actor) => view! {
                                                <>
                                                    <span class="topbar-chip operator" data-testid="operator-chip">{actor_chip_label(&actor)}</span>
                                                    <span class=move || format!("topbar-chip realtime {}", realtime_status.get().class())>
                                                        {move || realtime_status.get().label()}
                                                    </span>
                                                </>
                                            }.into_any(),
                                            None => view! { <span class="topbar-chip">{protected_workspace_label()}</span> }.into_any(),
                                        }
                                    }}
                                    <button
                                        type="button"
                                        class="logout-button"
                                        data-testid="signout-button"
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
                                                let set_devices = set_devices;
                                                let set_repositories = set_repositories;
                                                async move {
                                                    match logout_session().await {
                                                        Ok(session) => {
                                                            stop_realtime_updates(
                                                                UiEventSyncHandles {
                                                                    selected_thread_id,
                                                                    selected_job_id,
                                                                    event_source,
                                                                    set_threads,
                                                                    set_selected_thread_id,
                                                                    set_selected_thread,
                                                                    set_jobs,
                                                                    set_devices,
                                                                    set_repositories,
                                                                    set_selected_job_id,
                                                                    set_selected_job_detail,
                                                                    set_auth_session,
                                                                    set_status_text,
                                                                    set_realtime_status,
                                                                    set_event_source,
                                                                    runtime: realtime_runtime.get_value(),
                                                                },
                                                                RealtimeStatus::Disconnected,
                                                            );
                                                            set_auth_session.set(Some(session));
                                                            set_status_text.set("Signed out.".to_string());
                                                            set_selected_thread.set(None);
                                                            set_selected_thread_id.set(None);
                                                            set_selected_job_detail.set(None);
                                                            set_selected_job_id.set(None);
                                                            set_threads.set(Vec::new());
                                                            set_jobs.set(Vec::new());
                                                            set_devices.set(Vec::new());
                                                            set_repositories.set(Vec::new());
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
                                    data-testid="sidebar-backdrop"
                                    class:open=move || sidebar_open.get()
                                    on:click=move |_| set_sidebar_open.set(false)
                                ></button>
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
                                                match auth_session.get().and_then(|session| session.actor) {
                                                    Some(actor) => view! {
                                                        <div class="drawer-chip-row">
                                                            <span class="topbar-chip operator" data-testid="drawer-operator-chip">{actor_chip_label(&actor)}</span>
                                                            <span class=move || format!("topbar-chip realtime {}", realtime_status.get().class())>
                                                                {move || realtime_status.get().label()}
                                                            </span>
                                                        </div>
                                                    }.into_any(),
                                                    None => view! { <span class="topbar-chip">{protected_workspace_label()}</span> }.into_any(),
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
                                                    let set_devices = set_devices;
                                                    let set_repositories = set_repositories;
                                                    async move {
                                                        match logout_session().await {
                                                            Ok(session) => {
                                                                stop_realtime_updates(
                                                                    UiEventSyncHandles {
                                                                        selected_thread_id,
                                                                        selected_job_id,
                                                                        event_source,
                                                                        set_threads,
                                                                        set_selected_thread_id,
                                                                        set_selected_thread,
                                                                        set_jobs,
                                                                        set_devices,
                                                                        set_repositories,
                                                                        set_selected_job_id,
                                                                        set_selected_job_detail,
                                                                        set_auth_session,
                                                                        set_status_text,
                                                                        set_realtime_status,
                                                                        set_event_source,
                                                                        runtime: realtime_runtime.get_value(),
                                                                    },
                                                                    RealtimeStatus::Disconnected,
                                                                );
                                                                set_auth_session.set(Some(session));
                                                                set_status_text.set("Signed out.".to_string());
                                                                set_selected_thread.set(None);
                                                                set_selected_thread_id.set(None);
                                                                set_selected_job_detail.set(None);
                                                                set_selected_job_id.set(None);
                                                                set_threads.set(Vec::new());
                                                                set_jobs.set(Vec::new());
                                                                set_devices.set(Vec::new());
                                                                set_repositories.set(Vec::new());
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
                                                        let can_operate = auth_session
                                                            .get_untracked()
                                                            .map(|session| session_can_operate(&session))
                                                            .unwrap_or(false);
                                                        if !can_operate {
                                                            set_status_text.set("Your account is read-only.".to_string());
                                                            return;
                                                        }
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
                                                            prop:disabled=move || auth_session
                                                                .get()
                                                                .map(|session| !session_can_operate(&session))
                                                                .unwrap_or(true)
                                                            on:input=move |ev| set_new_thread_title.set(event_target_value(&ev))
                                                        />
                                                        <button
                                                            type="submit"
                                                            prop:disabled=move || auth_session
                                                                .get()
                                                                .map(|session| !session_can_operate(&session))
                                                                .unwrap_or(true)
                                                        >
                                                            "Create Thread"
                                                        </button>
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
                                                                        <span>{thread_message_count_label(thread.message_count)}</span>
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
                                                    <span class="thread-pill">{move || job_count_label(jobs.get().len())}</span>
                                                </div>
                                                <div class="job-list">
                                                    <For
                                                        each=move || jobs.get()
                                                        key=|job| job.id.clone()
                                                        children=move |job| {
                                                            let active_job_id = job.id.clone();
                                                            let click_job_id = job.id.clone();
                                                            let click_thread_id = job.thread_id.clone();
                                                            let thread_label = short_thread_label(&job.thread_id);
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
                                                                        <strong>{job_target_label(&job)}</strong>
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
                                            let pending_message_thread_id = thread_id.clone();
                                            let pending_indicator_thread_id = thread_id.clone();
                                            let thread_record = thread.thread.clone();
                                            let jobs = thread.jobs.clone();
                                            let messages =
                                                build_thread_timeline_messages(&thread, selected_job_detail.get().as_ref());
                                            let thread_notes = thread.related_notes.clone();

                                            view! {
                                                <div class="thread-focus">
                                                    <section class="thread-hero">
                                                        <div class="thread-hero-copy">
                                                            <p class="eyebrow">"Conversation"</p>
                                                            <h2>{thread_record.title.clone()}</h2>
                                                            <div class="thread-summary-row">
                                                                <span class="thread-pill">{thread_message_count_label(messages.len())}</span>
                                                                <span class="thread-pill">{job_count_label(jobs.len())}</span>
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
                                                                        let is_result_surface = message_is_result_surface(&message);
                                                                        let message_mode_class = message_mode_class(&message);
                                                                        let message_mode_badge = message_mode_badge(&message);
                                                                        let timestamp_label = message_timestamp_label(&message.created_at);
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
                                                                            (result_details.is_some() && !is_result_surface) || can_dispatch;
                                                                        let row_class = match message.role.as_str() {
                                                                            "user" => "message-row outgoing",
                                                                            "system" => "message-row incoming system",
                                                                            "assistant" => "message-row incoming automated",
                                                                            _ => "message-row incoming",
                                                                        };
                                                                        let role_label = match message.role.as_str() {
                                                                            "assistant" => "Elowen",
                                                                            "system" => "System",
                                                                            _ => "You",
                                                                        };
                                                                        let surface_class = match message.role.as_str() {
                                                                            "assistant" => "automated",
                                                                            "system" => "system-surface",
                                                                            _ => "",
                                                                        };

                                                                        view! {
                                                                            <div class=row_class>
                                                                                <article class=format!("message {} {} {}", message.role, surface_class, message_mode_class)>
                                                                                    <header class="message-header">
                                                                                        <div class="message-header-main">
                                                                                            <span class="message-role">{role_label}</span>
                                                                                            {message_mode_badge.map(|(badge_class, label)| {
                                                                                                view! { <span class=format!("mode-badge {}", badge_class)>{label}</span> }
                                                                                            })}
                                                                                        </div>
                                                                                        <span class="message-time">{timestamp_label}</span>
                                                                                    </header>
                                                                                    {if is_result_surface {
                                                                                        let details = result_details.clone();
                                                                                        let result_label = if message.status.ends_with(":awaiting_approval") {
                                                                                            "Ready For Review"
                                                                                        } else if message.status.ends_with(":failed") {
                                                                                            "Failure Summary"
                                                                                        } else {
                                                                                            "Result Summary"
                                                                                        };
                                                                                        view! {
                                                                                            <section class="result-message">
                                                                                                <p class="eyebrow">{result_label}</p>
                                                                                                <p class="message-body result-summary">{message.content.clone()}</p>
                                                                                                {details.map(|details| {
                                                                                                    view! {
                                                                                                        <details class="result-details">
                                                                                                            <summary>"Operational Details"</summary>
                                                                                                            <pre>{details}</pre>
                                                                                                        </details>
                                                                                                    }
                                                                                                })}
                                                                                            </section>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        view! {
                                                                                            <p class="message-body">{message.content.clone()}</p>
                                                                                        }.into_any()
                                                                                    }}
                                                                                    {execution_draft.clone().map(|draft| {
                                                                                        let thread_id = message_actions_thread_id.clone();
                                                                                        let source_message_id = message_id.clone();
                                                                                        let source_role = draft.source_role.clone();
                                                                                        let title = draft.title.clone();
                                                                                        let target_kind = draft.target_kind.clone();
                                                                                        let target_name = draft.target_name.clone();
                                                                                        let base_branch = draft
                                                                                            .base_branch
                                                                                            .clone()
                                                                                            .unwrap_or_else(|| "main".to_string());
                                                                                        let prompt = draft.prompt.clone();
                                                                                        let execution_intent = draft.execution_intent.clone();
                                                                                        let initial_device_id = if matches!(target_kind, JobTargetKind::Repository) {
                                                                                            preferred_device_value(
                                                                                                &devices.get_untracked(),
                                                                                                "",
                                                                                                &target_name,
                                                                                            )
                                                                                        } else {
                                                                                            preferred_capability_device_value(
                                                                                                &devices.get_untracked(),
                                                                                                "",
                                                                                                &target_name,
                                                                                            )
                                                                                        };
                                                                                        let initial_target_name = preferred_repository_value(
                                                                                            &repositories_for_device(&devices.get_untracked(), &initial_device_id),
                                                                                            &target_name,
                                                                                            "",
                                                                                        );
                                                                                        let draft_key = message_id.clone();
                                                                                        let initial_draft_edit = draft_edits
                                                                                            .get_untracked()
                                                                                            .get(&draft_key)
                                                                                            .cloned()
                                                                                            .unwrap_or_else(|| DraftEditState {
                                                                                                device_id: initial_device_id.clone(),
                                                                                                target_name: initial_target_name.clone(),
                                                                                                base_branch: base_branch.clone(),
                                                                                                prompt: prompt.clone(),
                                                                                            });
                                                                                        let (selected_device_id, set_selected_device_id) =
                                                                                            signal(initial_draft_edit.device_id);
                                                                                        let (selected_target_name, set_selected_target_name) =
                                                                                            signal(initial_draft_edit.target_name);
                                                                                        let (selected_base_branch, set_selected_base_branch) =
                                                                                            signal(initial_draft_edit.base_branch);
                                                                                        let (selected_prompt, set_selected_prompt) =
                                                                                            signal(initial_draft_edit.prompt);
                                                                                        Effect::new({
                                                                                            let devices = devices;
                                                                                            let target_name = target_name.clone();
                                                                                            let target_kind = target_kind.clone();
                                                                                            let set_selected_device_id = set_selected_device_id;
                                                                                            move |_| {
                                                                                                let current_value = selected_device_id.get();
                                                                                                let next_value = if matches!(target_kind, JobTargetKind::Repository) {
                                                                                                    preferred_device_value(
                                                                                                        &devices.get(),
                                                                                                        &current_value,
                                                                                                        &target_name,
                                                                                                    )
                                                                                                } else {
                                                                                                    preferred_capability_device_value(
                                                                                                        &devices.get(),
                                                                                                        &current_value,
                                                                                                        &target_name,
                                                                                                    )
                                                                                                };
                                                                                                if next_value != current_value {
                                                                                                    set_selected_device_id.set(next_value);
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                        Effect::new({
                                                                                            let devices = devices;
                                                                                            let target_name = target_name.clone();
                                                                                            let target_kind = target_kind.clone();
                                                                                            let set_selected_target_name = set_selected_target_name;
                                                                                            move |_| {
                                                                                                if !matches!(target_kind, JobTargetKind::Repository) {
                                                                                                    return;
                                                                                                }
                                                                                                let current_device = selected_device_id.get();
                                                                                                let current_value = selected_target_name.get();
                                                                                                let next_value = preferred_repository_value(
                                                                                                    &repositories_for_device(&devices.get(), &current_device),
                                                                                                    &current_value,
                                                                                                    &target_name,
                                                                                                );
                                                                                                if next_value != current_value {
                                                                                                    set_selected_target_name.set(next_value);
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                        Effect::new({
                                                                                            let draft_key = draft_key.clone();
                                                                                            let set_draft_edits = set_draft_edits;
                                                                                            move |_| {
                                                                                                let device_id = selected_device_id.get();
                                                                                                let target_name = selected_target_name.get();
                                                                                                let base_branch = selected_base_branch.get();
                                                                                                let prompt = selected_prompt.get();
                                                                                                set_draft_edits.update(|draft_edits| {
                                                                                                    draft_edits.insert(
                                                                                                        draft_key.clone(),
                                                                                                        DraftEditState {
                                                                                                            device_id,
                                                                                                            target_name,
                                                                                                            base_branch,
                                                                                                            prompt,
                                                                                                        },
                                                                                                    );
                                                                                                });
                                                                                            }
                                                                                        });
                                                                                        Effect::new({
                                                                                            let devices = devices;
                                                                                            let fallback_branch = base_branch.clone();
                                                                                            let target_kind = target_kind.clone();
                                                                                            let set_selected_base_branch = set_selected_base_branch;
                                                                                            move |_| {
                                                                                                if !matches!(target_kind, JobTargetKind::Repository) {
                                                                                                    return;
                                                                                                }
                                                                                                let current_device = selected_device_id.get();
                                                                                                let current_repo = selected_target_name.get();
                                                                                                let current_branch = selected_base_branch.get();
                                                                                                let next_branch = preferred_branch_value(
                                                                                                    &branches_for_device_repository(
                                                                                                        &devices.get(),
                                                                                                        &current_device,
                                                                                                        &current_repo,
                                                                                                    ),
                                                                                                    &current_branch,
                                                                                                    &fallback_branch,
                                                                                                );
                                                                                                if next_branch != current_branch {
                                                                                                    set_selected_base_branch.set(next_branch);
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                        view! {
                                                                                            <section class="execution-draft">
                                                                                                <header>
                                                                                                    <div>
                                                                                                        <p class="eyebrow">{if matches!(target_kind, JobTargetKind::Repository) { "Execution Draft" } else { "Capability Draft" }}</p>
                                                                                                        <h4>{title.clone()}</h4>
                                                                                                        <p class="draft-rationale">{draft.rationale}</p>
                                                                                                    </div>
                                                                                                    <span class="draft-intent">{execution_intent_label(&execution_intent)}</span>
                                                                                                </header>
                                                                                                <div class="draft-grid">
                                                                                                    <div class="draft-field">
                                                                                                        <strong>"Available Device"</strong>
                                                                                                        <select
                                                                                                            class="draft-field-control"
                                                                                                            prop:value=move || selected_device_id.get()
                                                                                                            prop:disabled=move || auth_session
                                                                                                                .get()
                                                                                                                .map(|session| !session_can_operate(&session))
                                                                                                                .unwrap_or(true)
                                                                                                            on:change=move |ev| set_selected_device_id.set(event_target_value(&ev))
                                                                                                        >
                                                                                                            <option value="">"Select a device"</option>
                                                                                                            <For
                                                                                                                each=move || devices.get()
                                                                                                                key=|device| device.id.clone()
                                                                                                                children=move |device| {
                                                                                                                    let label = device_option_label(&device);
                                                                                                                    view! {
                                                                                                                        <option value=device.id.clone()>{label}</option>
                                                                                                                    }
                                                                                                                }
                                                                                                            />
                                                                                                        </select>
                                                                                                        {move || {
                                                                                                            let device =
                                                                                                                selected_device(&devices.get(), &selected_device_id.get());
                                                                                                            device_trust_card(device, true, Some("draft-device-trust")).into_any()
                                                                                                        }}
                                                                                                    </div>
                                                                                                    <div class="draft-field">
                                                                                                        <strong>{if matches!(target_kind, JobTargetKind::Repository) { "Repository" } else { "Capability" }}</strong>
                                                                                                        {if matches!(target_kind, JobTargetKind::Repository) {
                                                                                                            view! {
                                                                                                                <select
                                                                                                                    class="draft-field-control"
                                                                                                                    prop:value=move || selected_target_name.get()
                                                                                                                    prop:disabled=move || auth_session
                                                                                                                        .get()
                                                                                                                        .map(|session| !session_can_operate(&session))
                                                                                                                        .unwrap_or(true)
                                                                                                                    on:change=move |ev| set_selected_target_name.set(event_target_value(&ev))
                                                                                                                >
                                                                                                                    <option value="">"Select a repository"</option>
                                                                                                                    <For
                                                                                                                        each=move || repositories_for_device(&devices.get(), &selected_device_id.get())
                                                                                                                        key=|repository| repository.name.clone()
                                                                                                                        children=move |repository| {
                                                                                                                            view! { <option value=repository.name.clone()>{repository.name.clone()}</option> }
                                                                                                                        }
                                                                                                                    />
                                                                                                                </select>
                                                                                                            }.into_any()
                                                                                                        } else {
                                                                                                            view! {
                                                                                                                <input
                                                                                                                    class="draft-field-control"
                                                                                                                    type="text"
                                                                                                                    prop:value=move || selected_target_name.get()
                                                                                                                    prop:disabled=move || auth_session
                                                                                                                        .get()
                                                                                                                        .map(|session| !session_can_operate(&session))
                                                                                                                        .unwrap_or(true)
                                                                                                                    on:input=move |ev| set_selected_target_name.set(event_target_value(&ev))
                                                                                                                />
                                                                                                            }.into_any()
                                                                                                        }}
                                                                                                    </div>
                                                                                                    {if matches!(target_kind, JobTargetKind::Repository) {
                                                                                                        view! {
                                                                                                            <div class="draft-field">
                                                                                                                <strong>"Base Branch"</strong>
                                                                                                                <select
                                                                                                                    class="draft-field-control"
                                                                                                                    prop:value=move || selected_base_branch.get()
                                                                                                                    prop:disabled=move || auth_session
                                                                                                                        .get()
                                                                                                                        .map(|session| !session_can_operate(&session))
                                                                                                                        .unwrap_or(true)
                                                                                                                    on:change=move |ev| set_selected_base_branch.set(event_target_value(&ev))
                                                                                                                >
                                                                                                                    <For
                                                                                                                        each=move || {
                                                                                                                            let branches = branches_for_device_repository(
                                                                                                                                &devices.get(),
                                                                                                                                &selected_device_id.get(),
                                                                                                                                &selected_target_name.get(),
                                                                                                                            );
                                                                                                                            if branches.is_empty() { vec![selected_base_branch.get()] } else { branches }
                                                                                                                        }
                                                                                                                        key=|branch| branch.clone()
                                                                                                                        children=move |branch| {
                                                                                                                            view! { <option value=branch.clone()>{branch.clone()}</option> }
                                                                                                                        }
                                                                                                                    />
                                                                                                                </select>
                                                                                                            </div>
                                                                                                        }.into_any()
                                                                                                    } else {
                                                                                                        ().into_any()
                                                                                                    }}
                                                                                                </div>
                                                                                                <section class="draft-request">
                                                                                                    <p class="eyebrow">"Dispatch Prompt"</p>
                                                                                                    <textarea
                                                                                                        class="draft-field-control draft-request-control"
                                                                                                        rows="5"
                                                                                                        prop:value=move || selected_prompt.get()
                                                                                                        prop:disabled=move || auth_session
                                                                                                            .get()
                                                                                                            .map(|session| !session_can_operate(&session))
                                                                                                            .unwrap_or(true)
                                                                                                        on:input=move |ev| set_selected_prompt.set(event_target_value(&ev))
                                                                                                    ></textarea>
                                                                                                </section>
                                                                                                <div class="draft-actions">
                                                                                                    <button
                                                                                                        type="button"
                                                                                                        prop:disabled=move || auth_session
                                                                                                            .get()
                                                                                                            .map(|session| !session_can_operate(&session))
                                                                                                            .unwrap_or(true)
                                                                                                        on:click=move |_| {
                                                                                                            let can_operate = auth_session
                                                                                                                .get_untracked()
                                                                                                                .map(|session| session_can_operate(&session))
                                                                                                                .unwrap_or(false);
                                                                                                            if !can_operate {
                                                                                                                set_status_text.set("Your account is read-only.".to_string());
                                                                                                                return;
                                                                                                            }
                                                                                                            let device_id = selected_device_id.get_untracked().trim().to_string();
                                                                                                            let target_name = selected_target_name.get_untracked().trim().to_string();
                                                                                                            let prompt = selected_prompt.get_untracked().trim().to_string();
                                                                                                            if device_id.is_empty() || target_name.is_empty() || prompt.is_empty() {
                                                                                                                set_status_text.set("Draft device, target, and prompt are required before dispatching.".to_string());
                                                                                                                return;
                                                                                                            }
                                                                                                            let base_branch =
                                                                                                                selected_base_branch.get_untracked().trim().to_string();
                                                                                                            let base_branch = if matches!(target_kind, JobTargetKind::Repository) {
                                                                                                                Some(if base_branch.is_empty() {
                                                                                                                    "main".to_string()
                                                                                                                } else {
                                                                                                                    base_branch
                                                                                                                })
                                                                                                            } else {
                                                                                                                None
                                                                                                            };
                                                                                                            let draft_key = draft_key.clone();
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
                                                                                                                let target_kind = target_kind.clone();
                                                                                                                let execution_intent = execution_intent.clone();
                                                                                                                async move {
                                                                                                                    match dispatch_thread_message(
                                                                                                                        &thread_id,
                                                                                                                        &source_message_id,
                                                                                                                        &title,
                                                                                                                        Some(device_id),
                                                                                                                        target_kind.clone(),
                                                                                                                        Some(target_name),
                                                                                                                        base_branch,
                                                                                                                        Some(prompt),
                                                                                                                        Some(execution_intent),
                                                                                                                    ).await {
                                                                                                                        Ok(job) => {
                                                                                                                            set_draft_edits.update(|draft_edits| {
                                                                                                                                draft_edits.remove(&draft_key);
                                                                                                                            });
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
                                                                                    {if has_inspector {
                                                                                        view! {
                                                                                            <details class="message-inspector">
                                                                                                <summary>"Inspect"</summary>
                                                                                                <div class="message-inspector-body">
                                                                                                    {result_details.clone().filter(|_| !is_result_surface).map(|details| {
                                                                                                        view! {
                                                                                                            <section class="summary-block">
                                                                                                                <p class="eyebrow">"Result Details"</p>
                                                                                                                <pre>{details}</pre>
                                                                                                            </section>
                                                                                                        }
                                                                                                    })}
                                                                                                    {if can_dispatch {
                                                                                                        view! {
                                                                                                            <div class="thread-meta">
                                                                                                                <span>"Explicit handoff"</span>
                                                                                                                <button
                                                                                                                    type="button"
                                                                                                                    prop:disabled=move || auth_session
                                                                                                                        .get()
                                                                                                                        .map(|session| !session_can_operate(&session))
                                                                                                                        .unwrap_or(true)
                                                                                                                    on:click={
                                                                                                                        let thread_id = message_actions_thread_id.clone();
                                                                                                                        let source_message_id = message_id.clone();
                                                                                                                        let source_role = message_role.clone();
                                                                                                                        move |_| {
                                                                                                                            let can_operate = auth_session
                                                                                                                                .get_untracked()
                                                                                                                                .map(|session| session_can_operate(&session))
                                                                                                                                .unwrap_or(false);
                                                                                                                            if !can_operate {
                                                                                                                                set_status_text.set("Your account is read-only.".to_string());
                                                                                                                                return;
                                                                                                                           }
                                                                                                                            let device_id = new_job_device.get_untracked().trim().to_string();
                                                                                                                            let repo_name = new_job_repo.get_untracked().trim().to_string();
                                                                                                                            let title = new_job_title.get_untracked().trim().to_string();
                                                                                                                            let base_branch = new_job_base_branch.get_untracked().trim().to_string();
                                                                                                                            if device_id.is_empty() || repo_name.is_empty() {
                                                                                                                                set_status_text.set("Device and repository are required for laptop dispatch.".to_string());
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
                                                                                                                                    match dispatch_thread_message(
                                                                                                                                        &thread_id,
                                                                                                                                        &source_message_id,
                                                                                                                                        &title,
                                                                                                                                        Some(device_id),
                                                                                                                                        JobTargetKind::Repository,
                                                                                                                                        Some(repo_name),
                                                                                                                                        Some(base_branch),
                                                                                                                                        None,
                                                                                                                                        None,
                                                                                                                                    ).await {
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
                                                                {move || {
                                                                    pending_chat_submission
                                                                        .get()
                                                                        .filter(|pending| pending.thread_id == pending_message_thread_id)
                                                                        .map(|pending| {
                                                                            view! {
                                                                                <div class="message-row outgoing">
                                                                                    <article class="message user pending">
                                                                                        <header class="message-header">
                                                                                            <div class="message-header-main">
                                                                                                <span class="message-role">"You"</span>
                                                                                            </div>
                                                                                            <span class="message-time">"Sending..."</span>
                                                                                        </header>
                                                                                        <p class="message-body">{pending.content}</p>
                                                                                    </article>
                                                                                </div>
                                                                            }
                                                                        })
                                                                }}
                                                                {move || {
                                                                    pending_chat_submission
                                                                        .get()
                                                                        .filter(|pending| pending.thread_id == pending_indicator_thread_id)
                                                                        .map(|_| {
                                                                            view! {
                                                                                <div class="message-row incoming">
                                                                                    <div class="chat-response-indicator" role="status" aria-live="polite">
                                                                                        <span>"Elowen is responding"</span>
                                                                                        <span class="chat-response-indicator-dots" aria-hidden="true">
                                                                                            <span class="chat-response-indicator-dot"></span>
                                                                                            <span class="chat-response-indicator-dot"></span>
                                                                                            <span class="chat-response-indicator-dot"></span>
                                                                                        </span>
                                                                                    </div>
                                                                                </div>
                                                                            }
                                                                        })
                                                                }}
                                                            </div>
                                                        </div>

                                                        <div class="composer-dock">
                                                            <form class="thread-composer" data-testid="thread-composer" on:submit=move |ev: ev::SubmitEvent| {
                                                                ev.prevent_default();
                                                                let can_operate = auth_session
                                                                    .get_untracked()
                                                                    .map(|session| session_can_operate(&session))
                                                                    .unwrap_or(false);
                                                                if !can_operate {
                                                                    set_status_text.set("Your account is read-only.".to_string());
                                                                    return;
                                                                }
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
                                                                                set_status_text.set(format!("Failed to send chat message: {error}"));
                                                                            }
                                                                        }
                                                                    }
                                                                });
                                                            }>
                                                                <div class="composer-input-wrap">
                                                                    <textarea
                                                                        node_ref=composer_textarea_ref
                                                                        rows="1"
                                                                        placeholder="Message Elowen"
                                                                        prop:value=move || new_message_content.get()
                                                                        prop:disabled=move || pending_chat_submission.get().is_some()
                                                                            || auth_session
                                                                                .get()
                                                                                .map(|session| !session_can_operate(&session))
                                                                                .unwrap_or(true)
                                                                        on:input=move |ev| set_new_message_content.set(event_target_value(&ev))
                                                                        on:keydown=move |ev: ev::KeyboardEvent| {
                                                                            if (ev.ctrl_key() || ev.meta_key()) && ev.key() == "Enter" {
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
                                                                            || auth_session
                                                                                .get()
                                                                                .map(|session| !session_can_operate(&session))
                                                                                .unwrap_or(true)
                                                                    >
                                                                        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                                                            <path d="M5 12h12m0 0-5-5m5 5-5 5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                                                                        </svg>
                                                                    </button>
                                                                </div>
                                                                <p class="composer-hint">
                                                                    {move || {
                                                                        if auth_session
                                                                            .get()
                                                                            .map(|session| session_can_operate(&session))
                                                                            .unwrap_or(false)
                                                                        {
                                                                            "Ctrl+Enter or Cmd+Enter to send".to_string()
                                                                        } else {
                                                                            "Read-only access: ask an operator or admin to send messages.".to_string()
                                                                        }
                                                                    }}
                                                                </p>
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

                                    <div class="context-tabs" role="tablist" aria-label="Conversation detail sections">
                                        <button
                                            type="button"
                                            class="context-tab"
                                            class:active=move || context_tab.get() == ContextTab::Thread
                                            aria-selected=move || context_tab.get() == ContextTab::Thread
                                            on:click=move |_| set_context_tab.set(ContextTab::Thread)
                                        >
                                            "Thread"
                                        </button>
                                        <button
                                            type="button"
                                            class="context-tab"
                                            class:active=move || context_tab.get() == ContextTab::Devices
                                            aria-selected=move || context_tab.get() == ContextTab::Devices
                                            on:click=move |_| set_context_tab.set(ContextTab::Devices)
                                        >
                                            "Devices"
                                        </button>
                                        <button
                                            type="button"
                                            class="context-tab"
                                            class:active=move || context_tab.get() == ContextTab::Job
                                            aria-selected=move || context_tab.get() == ContextTab::Job
                                            on:click=move |_| set_context_tab.set(ContextTab::Job)
                                        >
                                            "Selected Job"
                                        </button>
                                        <button
                                            type="button"
                                            class="context-tab"
                                            class:active=move || context_tab.get() == ContextTab::Manual
                                            aria-selected=move || context_tab.get() == ContextTab::Manual
                                            on:click=move |_| set_context_tab.set(ContextTab::Manual)
                                        >
                                            "Manual Job"
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
                                                let active_context_tab = context_tab.get();

                                                view! {
                                                    {if active_context_tab == ContextTab::Thread {
                                                        view! {
                                                        <section class="context-panel context-tab-panel" data-testid="context-tab-thread">
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
                                                                                    <strong>{job_target_label(&job)}</strong>
                                                                                </header>
                                                                                <div class="job-meta">
                                                                                    {if matches!(job.target_kind, JobTargetKind::Repository) {
                                                                                        view! {
                                                                                            <>
                                                                                                <span>{format!("Branch: {}", job.branch_name.clone().unwrap_or_else(|| "pending".to_string()))}</span>
                                                                                                <span>{format!("Base: {}", job.base_branch.clone().unwrap_or_else(|| "main".to_string()))}</span>
                                                                                            </>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        ().into_any()
                                                                                    }}
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
                                                        </section>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                    {if active_context_tab == ContextTab::Devices {
                                                        view! {
                                                    <section class="context-panel context-tab-panel" data-testid="context-tab-devices">
                                                        <div class="context-panel-body">
                                                            <div class="job-browser-header">
                                                                <div>
                                                                    <p class="eyebrow">"Trust Visibility"</p>
                                                                    <h2>"Edge trust and enrollment state"</h2>
                                                                    <p class="status">
                                                                        "Trust state is shown separately from generic device freshness so operators can spot revoked or risky edges quickly."
                                                                    </p>
                                                                </div>
                                                                <span class="thread-pill">{move || format!("{} devices", devices.get().len())}</span>
                                                            </div>
                                                            <div class="device-trust-list" data-testid="device-trust-list">
                                                                <For
                                                                    each=move || devices.get()
                                                                    key=|device| device.id.clone()
                                                                    children=move |device| {
                                                                        device_trust_card(Some(device), false, None)
                                                                    }
                                                                />
                                                                {move || {
                                                                    if devices.get().is_empty() {
                                                                        view! {
                                                                            <div class="empty">
                                                                                <p class="eyebrow">"No Devices"</p>
                                                                                <p>"Device trust state will appear here when the API reports registered edges."</p>
                                                                            </div>
                                                                        }
                                                                        .into_any()
                                                                    } else {
                                                                        ().into_any()
                                                                    }
                                                                }}
                                                            </div>
                                                        </div>
                                                    </section>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                    {move || {
                                                        if active_context_tab != ContextTab::Job {
                                                            return ().into_any();
                                                        }

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
                                                                <section class="context-panel context-tab-panel" data-testid="context-tab-job">
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
                                                                                    <p class="eyebrow">{if matches!(job_detail.job.target_kind, JobTargetKind::Repository) { "Repository" } else { "Capability" }}</p>
                                                                                    <strong>{job_target_label(&job_detail.job)}</strong>
                                                                                    {if matches!(job_detail.job.target_kind, JobTargetKind::Repository) {
                                                                                        view! { <span>{format!("Base {}", job_detail.job.base_branch.clone().unwrap_or_else(|| "main".to_string()))}</span> }.into_any()
                                                                                    } else {
                                                                                        view! { <span>{format!("Device {}", job_detail.job.device_id.clone().unwrap_or_else(|| "unassigned".to_string()))}</span> }.into_any()
                                                                                    }}
                                                                                </article>
                                                                                <article>
                                                                                    <p class="eyebrow">{if matches!(job_detail.job.target_kind, JobTargetKind::Repository) { "Branch" } else { "Target Kind" }}</p>
                                                                                    <strong>{if matches!(job_detail.job.target_kind, JobTargetKind::Repository) {
                                                                                        job_detail.job.branch_name.clone().unwrap_or_else(|| "pending".to_string())
                                                                                    } else {
                                                                                        "capability".to_string()
                                                                                    }}</strong>
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
                                                                                                    prop:disabled=move || auth_session
                                                                                                        .get()
                                                                                                        .map(|session| !session_can_operate(&session))
                                                                                                        .unwrap_or(true)
                                                                                                    on:click=move |_| {
                                                                                                        let can_operate = auth_session
                                                                                                            .get_untracked()
                                                                                                            .map(|session| session_can_operate(&session))
                                                                                                            .unwrap_or(false);
                                                                                                        if !can_operate {
                                                                                                            set_status_text.set("Your account is read-only.".to_string());
                                                                                                            return;
                                                                                                        }
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
                                                                                                        {if is_pending && auth_session
                                                                                                            .get()
                                                                                                            .map(|session| session_can_admin(&session))
                                                                                                            .unwrap_or(false) {
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
                                                                                                                                    match resolve_approval(&approval_id, "approved", "Push approved from UI").await {
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
                                                                                                                                    match resolve_approval(&approval_id, "rejected", "Push rejected from UI").await {
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
                                                                </section>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <section class="context-panel context-tab-panel" data-testid="context-tab-job">
                                                                    <div class="context-panel-body">
                                                                        <div class="empty">
                                                                            <p class="eyebrow">"No Job Selected"</p>
                                                                            <p>"Choose a job to inspect the live execution detail and event history."</p>
                                                                        </div>
                                                                    </div>
                                                                </section>
                                                            }.into_any()
                                                        }
                                                    }}
                                                    {if active_context_tab == ContextTab::Manual {
                                                        view! {
                                                    <section class="context-panel context-tab-panel" data-testid="context-tab-manual">
                                                        <div class="context-panel-body">
                                                            <form on:submit=move |ev: ev::SubmitEvent| {
                                                                ev.prevent_default();
                                                                let can_operate = auth_session
                                                                    .get_untracked()
                                                                    .map(|session| session_can_operate(&session))
                                                                    .unwrap_or(false);
                                                                if !can_operate {
                                                                    set_status_text.set("Your account is read-only.".to_string());
                                                                    return;
                                                                }
                                                                let title = new_job_title.get_untracked().trim().to_string();
                                                                let device_id = new_job_device.get_untracked().trim().to_string();
                                                                let repo_name = new_job_repo.get_untracked().trim().to_string();
                                                                let base_branch = new_job_base_branch.get_untracked().trim().to_string();
                                                                let request_text = new_job_request_text.get_untracked().trim().to_string();

                                                                if title.is_empty() || device_id.is_empty() || repo_name.is_empty() || request_text.is_empty() {
                                                                    set_status_text.set("Job title, device, repo, and request are required.".to_string());
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
                                                                            Some(device_id),
                                                                            JobTargetKind::Repository,
                                                                            Some(repo_name),
                                                                            Some(base_branch),
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
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                on:input=move |ev| set_new_job_title.set(event_target_value(&ev))
                                                                />
                                                                <select
                                                                    prop:value=move || new_job_device.get()
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                    on:change=move |ev| set_new_job_device.set(event_target_value(&ev))
                                                                >
                                                                    <option value="">"Select a device"</option>
                                                                    <For
                                                                        each=move || devices.get()
                                                                        key=|device| device.id.clone()
                                                                        children=move |device| {
                                                                            let label = device_option_label(&device);
                                                                            view! {
                                                                                <option value=device.id.clone()>{label}</option>
                                                                            }
                                                                        }
                                                                    />
                                                                </select>
                                                                {move || {
                                                                    let device =
                                                                        selected_device(&devices.get(), &new_job_device.get());
                                                                    device_trust_card(device, true, Some("manual-job-device-trust")).into_any()
                                                                }}
                                                                <select
                                                                    prop:value=move || new_job_repo.get()
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                    on:change=move |ev| set_new_job_repo.set(event_target_value(&ev))
                                                                >
                                                                    <option value="">"Select a repository"</option>
                                                                    <For
                                                                        each=move || repositories_for_device(&devices.get(), &new_job_device.get())
                                                                        key=|repository| repository.name.clone()
                                                                        children=move |repository| {
                                                                            view! {
                                                                                <option value=repository.name.clone()>{repository.name.clone()}</option>
                                                                            }
                                                                        }
                                                                    />
                                                                </select>
                                                                <select
                                                                    prop:value=move || new_job_base_branch.get()
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                    on:change=move |ev| set_new_job_base_branch.set(event_target_value(&ev))
                                                                >
                                                                    <For
                                                                        each=move || {
                                                                            let branches = branches_for_device_repository(
                                                                                &devices.get(),
                                                                                &new_job_device.get(),
                                                                                &new_job_repo.get(),
                                                                            );
                                                                            if branches.is_empty() {
                                                                                vec![new_job_base_branch.get()]
                                                                            } else {
                                                                                branches
                                                                            }
                                                                        }
                                                                        key=|branch| branch.clone()
                                                                        children=move |branch| {
                                                                            view! {
                                                                                <option value=branch.clone()>{branch.clone()}</option>
                                                                            }
                                                                        }
                                                                    />
                                                                </select>
                                                                <textarea
                                                                    placeholder="Describe the prompt to dispatch"
                                                                    prop:value=move || new_job_request_text.get()
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                    on:input=move |ev| set_new_job_request_text.set(event_target_value(&ev))
                                                                />
                                                                <button
                                                                    type="submit"
                                                                    prop:disabled=move || auth_session
                                                                        .get()
                                                                        .map(|session| !session_can_operate(&session))
                                                                        .unwrap_or(true)
                                                                >
                                                                    "Create Job"
                                                                </button>
                                                            </form>
                                                        </div>
                                                    </section>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
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

#[cfg(test)]
mod tests {
    use super::{
        device_enrollment_label, device_option_label, device_requires_trust_attention,
        device_trust_status_label,
    };
    use crate::models::{DeviceRecord, DeviceRepository, DeviceTrustRecord};

    fn sample_device() -> DeviceRecord {
        DeviceRecord {
            id: "travel-edge-02".into(),
            name: "Travel Edge".into(),
            primary_flag: false,
            allowed_repos: vec!["elowen-ui".into()],
            allowed_repo_roots: vec!["C:/Users/ericw/Projects/elowen/elowen-ui".into()],
            hidden_repos: Vec::new(),
            excluded_repo_paths: Vec::new(),
            discovered_repos: vec!["elowen-ui".into()],
            repositories: vec![DeviceRepository {
                name: "elowen-ui".into(),
                branches: vec!["main".into()],
            }],
            capabilities: vec!["workspace_change".into()],
            trust: DeviceTrustRecord {
                status: "attention_needed".into(),
                label: None,
                summary: Some("Review the re-enrollment before dispatch.".into()),
                detail: None,
                reason: None,
                enrollment_kind: Some("re_enrollment".into()),
                last_trusted_registration_at: Some("2026-04-14T16:20:00Z".into()),
                rotated_at: Some("2026-04-15T09:10:00Z".into()),
                revoked_at: None,
                updated_at: Some("2026-04-15T14:32:00Z".into()),
                can_dispatch: Some(false),
                requires_attention: true,
            },
            registered_at: "2026-04-13T18:00:00Z".into(),
            last_seen_at: "2026-04-15T14:21:00Z".into(),
            created_at: "2026-04-13T17:40:00Z".into(),
            updated_at: "2026-04-15T14:40:00Z".into(),
        }
    }

    #[test]
    fn formats_enrollment_and_trust_labels_for_multi_edge_devices() {
        let device = sample_device();

        assert_eq!(device_enrollment_label(&device), "Re-enrollment");
        assert_eq!(device_trust_status_label(&device.trust), "Needs Attention");
        assert_eq!(
            device_option_label(&device),
            "Travel Edge (travel-edge-02) · Re-enrollment · Needs Attention"
        );
    }

    #[test]
    fn marks_revoked_or_attention_devices_for_operator_review() {
        let device = sample_device();

        assert!(device_requires_trust_attention(&device));
    }
}
