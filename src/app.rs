use gloo_net::http::{Request, Response};
use gloo_timers::future::TimeoutFuture;
use leptos::{ev, prelude::*, task::spawn_local};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ThreadSummary {
    id: String,
    title: String,
    status: String,
    message_count: i64,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ThreadRecord {
    id: String,
    title: String,
    status: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct MessageRecord {
    id: String,
    role: String,
    content: String,
    status: String,
    payload_json: Value,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct ExecutionDraft {
    title: String,
    repo_name: Option<String>,
    base_branch: String,
    request_text: String,
    source_message_id: String,
    source_role: String,
    rationale: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JobRecord {
    id: String,
    short_id: String,
    correlation_id: String,
    thread_id: String,
    title: String,
    status: String,
    result: Option<String>,
    failure_class: Option<String>,
    repo_name: String,
    device_id: Option<String>,
    branch_name: Option<String>,
    base_branch: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JobEventRecord {
    id: String,
    correlation_id: String,
    event_type: String,
    payload_json: Value,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ThreadDetail {
    #[serde(flatten)]
    thread: ThreadRecord,
    messages: Vec<MessageRecord>,
    jobs: Vec<JobRecord>,
    related_notes: Vec<NoteRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JobDetail {
    #[serde(flatten)]
    job: JobRecord,
    execution_report_json: Value,
    summary: Option<SummaryRecord>,
    approvals: Vec<ApprovalRecord>,
    related_notes: Vec<NoteRecord>,
    events: Vec<JobEventRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct NoteRecord {
    note_id: String,
    title: String,
    slug: String,
    summary: String,
    tags: Vec<String>,
    aliases: Vec<String>,
    note_type: String,
    source_kind: Option<String>,
    source_id: Option<String>,
    current_revision_id: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct SummaryRecord {
    id: String,
    scope: String,
    source_id: String,
    version: i32,
    content: String,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ApprovalRecord {
    id: String,
    thread_id: String,
    job_id: String,
    action_type: String,
    status: String,
    summary: String,
    resolved_by: Option<String>,
    resolution_reason: Option<String>,
    created_at: String,
    resolved_at: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}

#[derive(Debug, Serialize)]
struct CreateThreadRequest {
    title: String,
}

#[derive(Debug, Serialize)]
struct CreateChatDispatchRequest {
    content: String,
    title: String,
    repo_name: String,
    base_branch: String,
}

#[derive(Debug, Serialize)]
struct CreateThreadChatRequest {
    content: String,
}

#[derive(Debug, Serialize)]
struct DispatchThreadMessageRequest {
    source_message_id: String,
    title: String,
    repo_name: String,
    base_branch: String,
    request_text: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateJobRequest {
    title: String,
    repo_name: String,
    base_branch: String,
    request_text: String,
}

#[derive(Debug, Serialize)]
struct ResolveApprovalRequest {
    status: String,
    resolved_by: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ChatDispatchResponse {
    message: MessageRecord,
    acknowledgement: MessageRecord,
    job: JobRecord,
}

#[derive(Debug, Deserialize)]
struct ChatReplyResponse {
    user_message: MessageRecord,
    assistant_message: MessageRecord,
}

#[derive(Debug, Deserialize)]
struct MessageDispatchResponse {
    source_message: MessageRecord,
    acknowledgement: MessageRecord,
    job: JobRecord,
}

#[derive(Debug, Serialize)]
struct PromoteJobNoteRequest {
    title: Option<String>,
    summary: Option<String>,
    body_markdown: Option<String>,
    tags: Vec<String>,
    aliases: Vec<String>,
    note_type: Option<String>,
}

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (jobs, set_jobs) = signal(Vec::<JobRecord>::new());
    let (selected_thread_id, set_selected_thread_id) = signal(None::<String>);
    let (selected_thread, set_selected_thread) = signal(None::<ThreadDetail>);
    let (preferred_job_id, set_preferred_job_id) = signal(None::<String>);
    let (selected_job_id, set_selected_job_id) = signal(None::<String>);
    let (selected_job_detail, set_selected_job_detail) = signal(None::<JobDetail>);
    let (new_thread_title, set_new_thread_title) = signal(String::new());
    let (new_message_content, set_new_message_content) = signal(String::new());
    let (new_job_title, set_new_job_title) = signal(String::new());
    let (new_job_repo, set_new_job_repo) = signal(String::from("elowen-api"));
    let (new_job_base_branch, set_new_job_base_branch) = signal(String::from("main"));
    let (new_job_request_text, set_new_job_request_text) = signal(String::new());
    let (status_text, set_status_text) = signal(String::from("Loading threads and jobs..."));

    spawn_local({
        let set_threads = set_threads;
        let set_jobs = set_jobs;
        let selected_thread_id = selected_thread_id;
        let set_selected_thread_id = set_selected_thread_id;
        let set_status_text = set_status_text;
        let set_selected_thread = set_selected_thread;
        let selected_job_id = selected_job_id;
        let set_selected_job_detail = set_selected_job_detail;

        async move {
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

            if let Some(thread_id) = selected_thread_id.get_untracked() {
                if let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
                {
                    set_status_text.set(format!("Failed to load thread: {error}"));
                }
            }

            loop {
                TimeoutFuture::new(5_000).await;

                if let Err(error) = sync_thread_list(
                    set_threads,
                    selected_thread_id,
                    set_selected_thread_id,
                    set_status_text,
                )
                .await
                {
                    set_status_text.set(format!("Failed to poll threads: {error}"));
                }

                if let Err(error) = sync_job_list(set_jobs).await {
                    set_status_text.set(format!("Failed to poll jobs: {error}"));
                }

                if let Some(thread_id) = selected_thread_id.get_untracked() {
                    if let Err(error) =
                        sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
                    {
                        set_status_text.set(format!("Failed to refresh thread: {error}"));
                    }
                }

                if let Some(job_id) = selected_job_id.get_untracked() {
                    if let Err(error) =
                        sync_selected_job(job_id, set_selected_job_detail, set_status_text).await
                    {
                        set_status_text.set(format!("Failed to refresh job: {error}"));
                    }
                }
            }
        }
    });

    Effect::new({
        let selected_thread_id = selected_thread_id;
        let set_selected_thread = set_selected_thread;
        let set_selected_job_id = set_selected_job_id;
        let set_selected_job_detail = set_selected_job_detail;
        let set_status_text = set_status_text;

        move |_| {
            if let Some(thread_id) = selected_thread_id.get() {
                set_selected_job_id.set(None);
                set_selected_job_detail.set(None);

                spawn_local({
                    let set_selected_thread = set_selected_thread;
                    let set_status_text = set_status_text;
                    async move {
                        if let Err(error) =
                            sync_selected_thread(thread_id, set_selected_thread, set_status_text)
                                .await
                        {
                            set_status_text.set(format!("Failed to load thread: {error}"));
                        }
                    }
                });
            } else {
                set_selected_thread.set(None);
                set_selected_job_id.set(None);
                set_selected_job_detail.set(None);
            }
        }
    });

    Effect::new({
        let selected_thread = selected_thread;
        let preferred_job_id = preferred_job_id;
        let selected_job_id = selected_job_id;
        let set_preferred_job_id = set_preferred_job_id;
        let set_selected_job_id = set_selected_job_id;
        let set_selected_job_detail = set_selected_job_detail;

        move |_| {
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
        }
    });

    Effect::new({
        let selected_job_id = selected_job_id;
        let set_selected_job_detail = set_selected_job_detail;
        let set_status_text = set_status_text;

        move |_| {
            if let Some(job_id) = selected_job_id.get() {
                spawn_local({
                    let set_selected_job_detail = set_selected_job_detail;
                    let set_status_text = set_status_text;
                    async move {
                        if let Err(error) =
                            sync_selected_job(job_id, set_selected_job_detail, set_status_text)
                                .await
                        {
                            set_status_text.set(format!("Failed to load job: {error}"));
                        }
                    }
                });
            } else {
                set_selected_job_detail.set(None);
            }
        }
    });

    view! {
        <main class="app-shell">
            <style>
                {r#"
                :root {
                    --bg: #f4f0e8;
                    --panel: #fffaf2;
                    --ink: #1f1b16;
                    --muted: #6d665d;
                    --line: #d8ccba;
                    --accent: #1f5a4d;
                    --accent-soft: #d9ebe5;
                }
                * { box-sizing: border-box; }
                body {
                    margin: 0;
                    background:
                        radial-gradient(circle at top left, rgba(31, 90, 77, 0.16), transparent 30%),
                        linear-gradient(180deg, #efe8da 0%, var(--bg) 100%);
                    color: var(--ink);
                    font-family: Georgia, 'Times New Roman', serif;
                    overflow-x: hidden;
                }
                .app-shell { min-height: 100vh; padding: 24px; overflow-x: hidden; }
                .frame {
                    display: grid;
                    grid-template-columns: 340px 1fr;
                    gap: 20px;
                    max-width: 1280px;
                    margin: 0 auto;
                    align-items: start;
                }
                .panel {
                    background: rgba(255, 250, 242, 0.92);
                    border: 1px solid var(--line);
                    border-radius: 20px;
                    box-shadow: 0 18px 40px rgba(40, 34, 28, 0.08);
                    backdrop-filter: blur(10px);
                    min-width: 0;
                }
                .sidebar { padding: 20px; display: flex; flex-direction: column; gap: 18px; }
                .content { padding: 24px; min-height: 70vh; min-width: 0; overflow-x: hidden; }
                .eyebrow {
                    text-transform: uppercase;
                    letter-spacing: 0.12em;
                    font-size: 0.75rem;
                    color: var(--muted);
                    margin: 0 0 8px 0;
                }
                h1, h2, h3, p { margin-top: 0; }
                h1 { font-size: 2.3rem; margin-bottom: 6px; }
                .status { color: var(--muted); font-size: 0.95rem; margin-bottom: 18px; }
                .status-row {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    flex-wrap: wrap;
                    margin-bottom: 12px;
                }
                .status-badge {
                    display: inline-flex;
                    align-items: center;
                    border-radius: 999px;
                    padding: 4px 10px;
                    font-size: 0.74rem;
                    font-weight: 700;
                    letter-spacing: 0.04em;
                    text-transform: uppercase;
                    background: rgba(64, 55, 42, 0.08);
                    color: var(--ink);
                }
                .status-badge.pending { background: #ece7df; color: #54483a; }
                .status-badge.dispatched, .status-badge.accepted, .status-badge.running, .status-badge.pushing {
                    background: #e1ebf7;
                    color: #264d7a;
                }
                .status-badge.awaiting_approval { background: #f4ead0; color: #72501f; }
                .status-badge.completed, .status-badge.approved, .status-badge.success {
                    background: #dfeee5;
                    color: #24543c;
                }
                .status-badge.failed, .status-badge.rejected, .status-badge.failure {
                    background: #f4ddd8;
                    color: #7a2f25;
                }
                form { display: grid; gap: 10px; }
                input, textarea, button { font: inherit; }
                input, textarea {
                    width: 100%;
                    border: 1px solid var(--line);
                    border-radius: 14px;
                    padding: 12px 14px;
                    background: #fff;
                    color: var(--ink);
                }
                textarea { min-height: 110px; resize: vertical; }
                button {
                    border: none;
                    border-radius: 999px;
                    padding: 11px 16px;
                    background: var(--accent);
                    color: white;
                    cursor: pointer;
                }
                .sidebar-section { display: grid; gap: 12px; }
                .sidebar-section + .sidebar-section { margin-top: 8px; }
                .thread-list { display: grid; gap: 10px; }
                .thread-list, .job-list, .message-list, .job-event-list, .summary-block, .approval-list, .report-grid {
                    min-width: 0;
                }
                .thread-card {
                    border: 1px solid var(--line);
                    border-radius: 16px;
                    padding: 14px;
                    background: #fff;
                    cursor: pointer;
                }
                .thread-card.active { border-color: var(--accent); background: var(--accent-soft); }
                .thread-meta, .job-meta, .message header, .job-event header {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    color: var(--muted);
                    font-size: 0.82rem;
                }
                .message-list, .job-list, .job-event-list { display: grid; gap: 12px; }
                .job-card, .message, .job-event, .job-detail, .approval-card, .report-grid article, .note-card {
                    border: 1px solid var(--line);
                    border-radius: 18px;
                    padding: 16px;
                    background: #fff;
                }
                .job-card { cursor: pointer; }
                .job-card.active { border-color: var(--accent); background: var(--accent-soft); }
                .job-card.compact {
                    padding: 14px;
                    gap: 8px;
                }
                .job-card.compact h3 {
                    margin-bottom: 6px;
                    font-size: 1rem;
                }
                .job-card.compact .status {
                    margin-bottom: 0;
                    font-size: 0.85rem;
                }
                .job-card.compact .job-meta {
                    font-size: 0.8rem;
                }
                .job-meta { flex-wrap: wrap; justify-content: flex-start; gap: 10px 16px; }
                .job-detail { background: rgba(255, 255, 255, 0.8); margin: 0 0 24px 0; }
                .job-overview {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
                    gap: 12px;
                    margin: 16px 0;
                }
                .job-overview article {
                    border: 1px solid var(--line);
                    border-radius: 16px;
                    padding: 14px;
                    background: rgba(255, 255, 255, 0.84);
                }
                .job-overview strong {
                    display: block;
                    font-size: 1rem;
                    margin-bottom: 4px;
                }
                pre {
                    margin: 0;
                    max-width: 100%;
                    overflow-x: auto;
                    white-space: pre-wrap;
                    word-break: break-word;
                }
                .report-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
                    gap: 12px;
                    margin: 16px 0;
                }
                .summary-block, .approval-list {
                    display: grid;
                    gap: 12px;
                    margin: 16px 0;
                }
                .note-list {
                    display: grid;
                    gap: 12px;
                    margin: 16px 0;
                }
                .summary-body {
                    white-space: pre-wrap;
                    line-height: 1.5;
                }
                .approval-card.pending { border-color: var(--accent); background: #f4fbf8; }
                .approval-note {
                    color: var(--muted);
                    font-size: 0.9rem;
                    margin-bottom: 0;
                }
                .approval-actions {
                    display: flex;
                    flex-wrap: wrap;
                    gap: 10px;
                    margin-top: 12px;
                }
                .button-secondary {
                    background: #8b6a42;
                }
                .job-event pre {
                    margin: 0;
                    padding: 12px;
                    border-radius: 12px;
                    background: #f7f1e6;
                    overflow-x: auto;
                    white-space: pre-wrap;
                    word-break: break-word;
                    font-size: 0.82rem;
                }
                .message.user { background: #fcf3e8; }
                .message.assistant { background: #eef6f3; }
                .message.system { background: #f5f0fb; }
                .message.mode-conversation { border-color: #7aa88b; }
                .message.mode-draft-ready { box-shadow: inset 0 0 0 1px rgba(31, 90, 77, 0.12); }
                .message.mode-handoff { border-color: #8b6a42; background: #fbf4e8; }
                .message.mode-dispatch, .message.mode-job-update { border-color: #6c7ea6; }
                .message-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 12px;
                    flex-wrap: wrap;
                }
                .message-header-main {
                    display: flex;
                    align-items: center;
                    gap: 10px;
                    flex-wrap: wrap;
                }
                .mode-badge {
                    display: inline-flex;
                    align-items: center;
                    border-radius: 999px;
                    padding: 4px 10px;
                    font-size: 0.72rem;
                    font-weight: 700;
                    letter-spacing: 0.04em;
                    text-transform: uppercase;
                    background: rgba(64, 55, 42, 0.08);
                    color: var(--ink);
                }
                .mode-badge.conversation { background: #dfeee5; color: #24543c; }
                .mode-badge.handoff { background: #f1e2c8; color: #6e4a1d; }
                .mode-badge.dispatch { background: #e3e8f5; color: #334e82; }
                .mode-badge.job-update { background: #dde7f7; color: #2f4b7f; }
                .mode-badge.system { background: #efe7fb; color: #5d3e84; }
                .message-body { white-space: pre-wrap; }
                .thread-composer {
                    margin-top: 18px;
                    padding: 16px;
                    border: 1px solid var(--line);
                    border-radius: 18px;
                    background: rgba(255, 255, 255, 0.86);
                    display: grid;
                    gap: 12px;
                }
                .composer-header {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    flex-wrap: wrap;
                    color: var(--muted);
                    font-size: 0.86rem;
                }
                .composer-actions {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    gap: 10px;
                    flex-wrap: wrap;
                }
                .composer-actions .button-secondary {
                    background: #8b6a42;
                }
                .dispatch-fallback {
                    border: 1px solid var(--line);
                    border-radius: 16px;
                    padding: 12px 14px;
                    background: rgba(244, 240, 232, 0.65);
                }
                .dispatch-fallback summary {
                    cursor: pointer;
                    font-weight: 700;
                    color: var(--ink);
                }
                .dispatch-fallback[open] {
                    display: grid;
                    gap: 10px;
                }
                .result-message {
                    border: 1px solid #b8d3c7;
                    border-radius: 16px;
                    padding: 14px;
                    background: #f6fbf8;
                }
                .result-message pre {
                    background: transparent;
                    padding: 0;
                }
                .execution-draft {
                    margin-top: 14px;
                    padding: 14px;
                    border: 1px solid #b8d3c7;
                    border-radius: 16px;
                    background: rgba(255, 255, 255, 0.78);
                    display: grid;
                    gap: 10px;
                }
                .execution-draft header {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    flex-wrap: wrap;
                    color: var(--muted);
                    font-size: 0.82rem;
                }
                .execution-draft h4 {
                    margin: 0;
                    font-size: 1rem;
                    color: var(--ink);
                }
                .draft-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
                    gap: 10px;
                }
                .draft-field {
                    display: grid;
                    gap: 6px;
                    font-size: 0.84rem;
                    color: var(--muted);
                }
                .draft-field strong {
                    color: var(--ink);
                    font-size: 0.9rem;
                }
                .draft-field textarea {
                    min-height: 96px;
                }
                .draft-actions {
                    display: flex;
                    flex-wrap: wrap;
                    gap: 10px;
                    align-items: center;
                }
                .draft-rationale {
                    color: var(--muted);
                    font-size: 0.88rem;
                    margin: 0;
                }
                .empty {
                    padding: 36px 24px;
                    border: 1px dashed var(--line);
                    border-radius: 18px;
                    text-align: center;
                    color: var(--muted);
                    background: rgba(255,255,255,0.6);
                }
                @media (max-width: 920px) {
                    .frame { grid-template-columns: 1fr; }
                }
                "#}
            </style>
            <div class="frame">
                <section class="panel sidebar">
                    <div>
                        <p class="eyebrow">"Elowen Workspace"</p>
                        <h1>"Threads"</h1>
                        <p class="status">{move || status_text.get()}</p>
                    </div>
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
                    <div class="sidebar-section">
                        <p class="eyebrow">"Threads"</p>
                        <div class="thread-list">
                            <For
                                each=move || threads.get()
                                key=|thread| thread.id.clone()
                                children=move |thread| {
                                    let active_thread_id = thread.id.clone();
                                    let click_thread_id = thread.id.clone();
                                    view! {
                                        <article
                                            class=("thread-card", true)
                                            class:active=move || selected_thread_id.get() == Some(active_thread_id.clone())
                                            on:click=move |_| set_selected_thread_id.set(Some(click_thread_id.clone()))
                                        >
                                            <h3>{thread.title.clone()}</h3>
                                            <div class="thread-meta">
                                                <span>{format!("{} messages", thread.message_count)}</span>
                                                <span>{thread.status.clone()}</span>
                                            </div>
                                        </article>
                                    }
                                }
                            />
                        </div>
                    </div>
                    <div class="sidebar-section">
                        <p class="eyebrow">"Global Jobs"</p>
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
                </section>
                <section class="panel content">
                    {move || {
                        if let Some(thread) = selected_thread.get() {
                            let thread_id = thread.thread.id.clone();
                            let job_thread_id = thread_id.clone();
                            let message_actions_thread_id = thread_id.clone();
                            let chat_submit_thread_id = thread_id.clone();
                            let draft_dispatch_thread_id = thread_id.clone();
                            let thread_record = thread.thread.clone();
                            let jobs = thread.jobs.clone();
                            let messages = thread.messages.clone();
                            let thread_notes = thread.related_notes.clone();
                            let has_jobs = !jobs.is_empty();
                            let active_job_id = selected_job_id.get();

                            view! {
                                <div>
                                    <p class="eyebrow">"Thread Detail"</p>
                                    <h2>{thread_record.title.clone()}</h2>
                                    <p class="status">{format!("Status: {} | Updated: {}", thread_record.status, thread_record.updated_at)}</p>

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
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div class="empty">
                                                    <p class="eyebrow">"No Job Selected"</p>
                                                    <p>"Choose a job to inspect the live execution detail and event history."</p>
                                                </div>
                                            }.into_any()
                                        }
                                    }}

                                    <details>
                                        <summary>"Advanced Manual Job"</summary>
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
                                    </details>

                                    <div class="message-list">
                                        <For
                                            each=move || messages.clone()
                                            key=|message| message.id.clone()
                                            children=move |message| {
                                                let message_id = message.id.clone();
                                                let message_role = message.role.clone();
                                                let execution_draft = message_execution_draft(&message);
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
                                                                                        match dispatch_thread_message(&thread_id, &source_message_id, &title, &repo_name, &base_branch, None).await {
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
                                    </div>
                                    <form class="thread-composer" on:submit=move |ev: ev::SubmitEvent| {
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
                                                        set_status_text
                                                            .set(format!("Failed to send chat message: {error}"));
                                                    }
                                                }
                                            }
                                        });
                                    }>
                                        <div class="composer-header">
                                            <span>"Conversational Chat"</span>
                                            <span>"Workflow #2 is the default. Use the dispatch controls below only when you want laptop execution."</span>
                                        </div>
                                        <textarea
                                            placeholder="Send a message to Elowen"
                                            prop:value=move || new_message_content.get()
                                            on:input=move |ev| set_new_message_content.set(event_target_value(&ev))
                                        />
                                        <details class="dispatch-fallback">
                                            <summary>"Dispatch To Laptop (Workflow #1 Fallback)"</summary>
                                            <div class="thread-meta">
                                                <span>"Explicit dispatch only"</span>
                                                <span>{format!("Primary device fallback is automatic when repo `{}` is allowed.", new_job_repo.get())}</span>
                                            </div>
                                            <input
                                                type="text"
                                                placeholder="Repository"
                                                prop:value=move || new_job_repo.get()
                                                on:input=move |ev| set_new_job_repo.set(event_target_value(&ev))
                                            />
                                            <input
                                                type="text"
                                                placeholder="Optional job title"
                                                prop:value=move || new_job_title.get()
                                                on:input=move |ev| set_new_job_title.set(event_target_value(&ev))
                                            />
                                            <input
                                                type="text"
                                                placeholder="Base branch"
                                                prop:value=move || new_job_base_branch.get()
                                                on:input=move |ev| set_new_job_base_branch.set(event_target_value(&ev))
                                            />
                                        </details>
                                        <div class="composer-actions">
                                            <button type="submit">"Send"</button>
                                            <button
                                                class="button-secondary"
                                                type="button"
                                                on:click={
                                                    let message_thread_id = draft_dispatch_thread_id.clone();
                                                    move |_| {
                                                    let content = new_message_content.get_untracked().trim().to_string();
                                                    let repo_name = new_job_repo.get_untracked().trim().to_string();
                                                    let title = new_job_title.get_untracked().trim().to_string();
                                                    let base_branch = new_job_base_branch.get_untracked().trim().to_string();
                                                    if content.is_empty() {
                                                        set_status_text.set("Message content is required.".to_string());
                                                        return;
                                                    }
                                                    if repo_name.is_empty() {
                                                        set_status_text.set("Repository is required for laptop dispatch.".to_string());
                                                        return;
                                                    }

                                                    spawn_local({
                                                        let set_new_message_content = set_new_message_content;
                                                        let set_new_job_title = set_new_job_title;
                                                        let set_selected_thread = set_selected_thread;
                                                        let set_preferred_job_id = set_preferred_job_id;
                                                        let set_selected_job_id = set_selected_job_id;
                                                        let set_status_text = set_status_text;
                                                        let set_threads = set_threads;
                                                        let set_jobs = set_jobs;
                                                        let selected_thread_id = selected_thread_id;
                                                        let set_selected_thread_id = set_selected_thread_id;
                                                        let thread_id = message_thread_id.clone();

                                                        async move {
                                                            match dispatch_chat_message(&thread_id, &content, &title, &repo_name, &base_branch).await {
                                                                Ok(job) => {
                                                                    set_new_message_content.set(String::new());
                                                                    set_new_job_title.set(String::new());
                                                                    set_preferred_job_id.set(Some(job.id.clone()));
                                                                    set_selected_job_id.set(Some(job.id.clone()));
                                                                    set_status_text.set(format!(
                                                                        "Dispatched job {} from chat.",
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
                                                                    set_status_text
                                                                        .set(format!("Failed to dispatch from chat: {error}"));
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                                }
                                            >
                                                "Dispatch Draft To Laptop"
                                            </button>
                                        </div>
                                    </form>
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
        </main>
    }
}

async fn sync_thread_list(
    set_threads: WriteSignal<Vec<ThreadSummary>>,
    selected_thread_id: ReadSignal<Option<String>>,
    set_selected_thread_id: WriteSignal<Option<String>>,
    set_status_text: WriteSignal<String>,
) -> Result<(), String> {
    let fetched_threads = fetch_threads().await?;
    let current_selected = selected_thread_id.get_untracked();

    if fetched_threads.is_empty() {
        set_selected_thread_id.set(None);
        set_status_text.set("No threads yet. Create one to start.".to_string());
    } else {
        let selected_exists = current_selected
            .as_ref()
            .map(|id| fetched_threads.iter().any(|thread| thread.id == *id))
            .unwrap_or(false);

        if !selected_exists {
            set_selected_thread_id.set(fetched_threads.first().map(|thread| thread.id.clone()));
        }

        set_status_text.set("Thread state synced.".to_string());
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
    let thread = fetch_thread(&thread_id).await?;
    set_selected_thread.set(Some(thread));
    set_status_text.set("Thread detail loaded.".to_string());
    Ok(())
}

async fn sync_selected_job(
    job_id: String,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    set_status_text: WriteSignal<String>,
) -> Result<(), String> {
    let job = fetch_job(&job_id).await?;
    set_selected_job_detail.set(Some(job));
    set_status_text.set("Job detail loaded.".to_string());
    Ok(())
}

fn api_base() -> String {
    if let Some(origin) = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .filter(|value| !value.is_empty() && value != "null")
    {
        return format!("{origin}/api/v1");
    }

    "http://localhost:8080/api/v1".to_string()
}

async fn fetch_threads() -> Result<Vec<ThreadSummary>, String> {
    decode_json(
        Request::get(&format!("{}/threads", api_base()))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn fetch_jobs() -> Result<Vec<JobRecord>, String> {
    decode_json(
        Request::get(&format!("{}/jobs", api_base()))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn fetch_thread(thread_id: &str) -> Result<ThreadDetail, String> {
    decode_json(
        Request::get(&format!("{}/threads/{thread_id}", api_base()))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn fetch_job(job_id: &str) -> Result<JobDetail, String> {
    decode_json(
        Request::get(&format!("{}/jobs/{job_id}", api_base()))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn create_thread(title: &str) -> Result<ThreadDetail, String> {
    decode_json(
        Request::post(&format!("{}/threads", api_base()))
            .json(&CreateThreadRequest {
                title: title.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn send_thread_chat_message(thread_id: &str, content: &str) -> Result<(), String> {
    let response: ChatReplyResponse = decode_json(
        Request::post(&format!("{}/threads/{thread_id}/chat", api_base()))
            .json(&CreateThreadChatRequest {
                content: content.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await?;

    let _ = (&response.user_message, &response.assistant_message);
    Ok(())
}

async fn dispatch_chat_message(
    thread_id: &str,
    content: &str,
    title: &str,
    repo_name: &str,
    base_branch: &str,
) -> Result<JobRecord, String> {
    let response: ChatDispatchResponse = decode_json(
        Request::post(&format!("{}/threads/{thread_id}/chat-dispatch", api_base()))
            .json(&CreateChatDispatchRequest {
                content: content.to_string(),
                title: title.to_string(),
                repo_name: repo_name.to_string(),
                base_branch: base_branch.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await?;

    let _ = (&response.message, &response.acknowledgement);
    Ok(response.job)
}

async fn dispatch_thread_message(
    thread_id: &str,
    source_message_id: &str,
    title: &str,
    repo_name: &str,
    base_branch: &str,
    request_text: Option<String>,
) -> Result<JobRecord, String> {
    let response: MessageDispatchResponse = decode_json(
        Request::post(&format!(
            "{}/threads/{thread_id}/message-dispatch",
            api_base()
        ))
        .json(&DispatchThreadMessageRequest {
            source_message_id: source_message_id.to_string(),
            title: title.to_string(),
            repo_name: repo_name.to_string(),
            base_branch: base_branch.to_string(),
            request_text,
        })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await?;

    let _ = (&response.source_message, &response.acknowledgement);
    Ok(response.job)
}

async fn create_job(
    thread_id: &str,
    title: &str,
    repo_name: &str,
    base_branch: &str,
    request_text: &str,
) -> Result<JobRecord, String> {
    let detail: JobDetail = decode_json(
        Request::post(&format!("{}/threads/{thread_id}/jobs", api_base()))
            .json(&CreateJobRequest {
                title: title.to_string(),
                repo_name: repo_name.to_string(),
                base_branch: base_branch.to_string(),
                request_text: request_text.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await?;

    Ok(detail.job)
}

async fn promote_job_note(job_id: &str) -> Result<NoteRecord, String> {
    decode_json(
        Request::post(&format!("{}/jobs/{job_id}/notes/promote", api_base()))
            .json(&PromoteJobNoteRequest {
                title: None,
                summary: None,
                body_markdown: None,
                tags: vec!["job".to_string(), "promoted".to_string()],
                aliases: Vec::new(),
                note_type: Some("job-summary".to_string()),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn resolve_approval(
    approval_id: &str,
    status: &str,
    resolved_by: &str,
    reason: &str,
) -> Result<ApprovalRecord, String> {
    decode_json(
        Request::post(&format!("{}/approvals/{approval_id}/resolve", api_base()))
            .json(&ResolveApprovalRequest {
                status: status.to_string(),
                resolved_by: resolved_by.to_string(),
                reason: reason.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

async fn decode_json<T>(response: Response) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;

    if !(200..300).contains(&status) {
        if let Ok(api_error) = serde_json::from_str::<ApiError>(&body) {
            return Err(api_error.error);
        }

        return Err(format!("request failed with status {status}: {body}"));
    }

    serde_json::from_str::<T>(&body).map_err(|error| error.to_string())
}

fn format_json_value(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn report_status_label(report: &Value, key: &str) -> String {
    report
        .get(key)
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn report_diff_stat(report: &Value) -> Option<String> {
    report
        .get("diff_stat")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn report_last_message(report: &Value) -> Option<String> {
    report
        .get("last_message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn report_array_strings(report: &Value, key: &str) -> Vec<String> {
    report
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn message_execution_draft(message: &MessageRecord) -> Option<ExecutionDraft> {
    message
        .payload_json
        .get("execution_draft")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn message_mode_class(message: &MessageRecord) -> &'static str {
    if message.status == "conversation.reply" && message_execution_draft(message).is_some() {
        "mode-conversation mode-draft-ready"
    } else if message.status == "conversation.reply" {
        "mode-conversation"
    } else if message.status == "workflow.handoff.created" {
        "mode-handoff"
    } else if message.status == "workflow.dispatch.created" {
        "mode-dispatch"
    } else if message.status.starts_with("job_event:") {
        "mode-job-update"
    } else {
        ""
    }
}

fn message_mode_badge(message: &MessageRecord) -> Option<(&'static str, &'static str)> {
    if message.status == "conversation.reply" && message_execution_draft(message).is_some() {
        Some(("conversation", "Draft Ready"))
    } else if message.status == "conversation.reply" {
        Some(("conversation", "Conversation"))
    } else if message.status == "workflow.handoff.created" {
        Some(("handoff", "Workflow Handoff"))
    } else if message.status == "workflow.dispatch.created" {
        Some(("dispatch", "Direct Dispatch"))
    } else if message.status.starts_with("job_event:") {
        Some(("job-update", "Job Update"))
    } else if message.role == "system" {
        Some(("system", "System"))
    } else {
        None
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "pending" => "pending",
        "dispatched" => "dispatched",
        "accepted" => "accepted",
        "running" => "running",
        "pushing" => "pushing",
        "awaiting_approval" => "awaiting_approval",
        "completed" => "completed",
        "approved" => "approved",
        "success" => "success",
        "failed" => "failed",
        "rejected" => "rejected",
        "failure" => "failure",
        _ => "pending",
    }
}

fn approval_status_note(approval_status: &str, job_status: &str) -> String {
    match (approval_status, job_status) {
        ("pending", _) => {
            "Review the summary and approve when you want the laptop to push the branch."
                .to_string()
        }
        ("approved", "pushing") => {
            "Push approval was granted and the laptop is currently pushing the branch.".to_string()
        }
        ("approved", "completed") => {
            "Push approval was granted and the job has finished its post-approval push step."
                .to_string()
        }
        ("approved", _) => {
            "Push approval was granted. Waiting for the post-approval push lifecycle to settle."
                .to_string()
        }
        ("rejected", _) => {
            "Push was rejected. The job summary remains available, but no branch was pushed."
                .to_string()
        }
        _ => "This approval record is retained as part of the job audit trail.".to_string(),
    }
}

fn format_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join("\n")
    }
}
