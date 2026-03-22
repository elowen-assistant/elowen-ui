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
    created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JobRecord {
    id: String,
    short_id: String,
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
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JobDetail {
    #[serde(flatten)]
    job: JobRecord,
    events: Vec<JobEventRecord>,
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
struct CreateMessageRequest {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct CreateJobRequest {
    title: String,
    repo_name: String,
    base_branch: String,
    request_text: String,
}

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (selected_thread_id, set_selected_thread_id) = signal(None::<String>);
    let (selected_thread, set_selected_thread) = signal(None::<ThreadDetail>);
    let (selected_job_id, set_selected_job_id) = signal(None::<String>);
    let (selected_job_detail, set_selected_job_detail) = signal(None::<JobDetail>);
    let (new_thread_title, set_new_thread_title) = signal(String::new());
    let (new_message_content, set_new_message_content) = signal(String::new());
    let (new_job_title, set_new_job_title) = signal(String::new());
    let (new_job_repo, set_new_job_repo) = signal(String::from("elowen-api"));
    let (new_job_base_branch, set_new_job_base_branch) = signal(String::from("main"));
    let (new_job_request_text, set_new_job_request_text) = signal(String::new());
    let (status_text, set_status_text) = signal(String::from("Loading threads..."));

    spawn_local({
        let set_threads = set_threads;
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
        let selected_job_id = selected_job_id;
        let set_selected_job_id = set_selected_job_id;
        let set_selected_job_detail = set_selected_job_detail;

        move |_| {
            if let Some(thread) = selected_thread.get() {
                let current_job_id = selected_job_id.get();
                let next_job_id = if current_job_id
                    .as_ref()
                    .is_some_and(|job_id| thread.jobs.iter().any(|job| job.id == *job_id))
                {
                    current_job_id
                } else {
                    thread.jobs.first().map(|job| job.id.clone())
                };
                set_selected_job_id.set(next_job_id);
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
                }
                .app-shell { min-height: 100vh; padding: 24px; }
                .frame {
                    display: grid;
                    grid-template-columns: 340px 1fr;
                    gap: 20px;
                    max-width: 1280px;
                    margin: 0 auto;
                }
                .panel {
                    background: rgba(255, 250, 242, 0.92);
                    border: 1px solid var(--line);
                    border-radius: 20px;
                    box-shadow: 0 18px 40px rgba(40, 34, 28, 0.08);
                    backdrop-filter: blur(10px);
                }
                .sidebar { padding: 20px; display: flex; flex-direction: column; gap: 18px; }
                .content { padding: 24px; min-height: 70vh; }
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
                .thread-list { display: grid; gap: 10px; }
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
                .job-card, .message, .job-event, .job-detail {
                    border: 1px solid var(--line);
                    border-radius: 18px;
                    padding: 16px;
                    background: #fff;
                }
                .job-card { cursor: pointer; }
                .job-card.active { border-color: var(--accent); background: var(--accent-soft); }
                .job-meta { flex-wrap: wrap; justify-content: flex-start; gap: 10px 16px; }
                .job-detail { background: rgba(255, 255, 255, 0.8); margin: 0 0 24px 0; }
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
                </section>
                <section class="panel content">
                    {move || {
                        if let Some(thread) = selected_thread.get() {
                            let thread_id = thread.thread.id.clone();
                            let job_thread_id = thread_id.clone();
                            let message_thread_id = thread_id.clone();
                            let thread_record = thread.thread.clone();
                            let jobs = thread.jobs.clone();
                            let messages = thread.messages.clone();
                            let has_jobs = !jobs.is_empty();
                            let active_job_id = selected_job_id.get();

                            view! {
                                <div>
                                    <p class="eyebrow">"Thread Detail"</p>
                                    <h2>{thread_record.title.clone()}</h2>
                                    <p class="status">{format!("Status: {} | Updated: {}", thread_record.status, thread_record.updated_at)}</p>

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
                                                                <p class="status">{format!("{} - {}", job.short_id, job.status)}</p>
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
                                                    <p>"Create a coding job from this thread to dispatch it to the primary edge device."</p>
                                                </div>
                                            }.into_any()
                                        }}
                                    </div>

                                    {move || {
                                        if let Some(job_detail) = selected_job_detail.get() {
                                            view! {
                                                <section class="job-detail">
                                                    <p class="eyebrow">"Selected Job"</p>
                                                    <h3>{job_detail.job.title.clone()}</h3>
                                                    <p class="status">
                                                        {format!(
                                                            "{} | {} | {}",
                                                            job_detail.job.short_id,
                                                            job_detail.job.status,
                                                            job_detail.job.device_id.clone().unwrap_or_else(|| "unassigned".to_string())
                                                        )}
                                                    </p>
                                                    <div class="job-meta">
                                                        <span>{format!("Repo: {}", job_detail.job.repo_name.clone())}</span>
                                                        <span>{format!("Branch: {}", job_detail.job.branch_name.clone().unwrap_or_else(|| "pending".to_string()))}</span>
                                                        <span>{format!("Base: {}", job_detail.job.base_branch.clone().unwrap_or_else(|| "main".to_string()))}</span>
                                                        <span>{format!("Updated: {}", job_detail.job.updated_at.clone())}</span>
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
                                            let set_selected_job_id = set_selected_job_id;
                                            let set_status_text = set_status_text;
                                            let set_threads = set_threads;
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

                                    <div class="message-list">
                                        <For
                                            each=move || messages.clone()
                                            key=|message| message.id.clone()
                                            children=move |message| {
                                                view! {
                                                    <article class=format!("message {}", message.role)>
                                                        <header>
                                                            <strong>{message.role.clone()}</strong>
                                                            <span>{message.created_at.clone()}</span>
                                                        </header>
                                                        <p>{message.content.clone()}</p>
                                                    </article>
                                                }
                                            }
                                        />
                                    </div>
                                    <form on:submit=move |ev: ev::SubmitEvent| {
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
                                            let thread_id = message_thread_id.clone();

                                            async move {
                                                match create_message(&thread_id, &content).await {
                                                    Ok(_) => {
                                                        set_new_message_content.set(String::new());
                                                        set_status_text.set("Message posted.".to_string());
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
                                                            .set(format!("Failed to post message: {error}"));
                                                    }
                                                }
                                            }
                                        });
                                    }>
                                        <textarea
                                            placeholder="Post a message to this thread"
                                            prop:value=move || new_message_content.get()
                                            on:input=move |ev| set_new_message_content.set(event_target_value(&ev))
                                        />
                                        <button type="submit">"Post Message"</button>
                                    </form>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="empty">
                                    <p class="eyebrow">"No Thread Selected"</p>
                                    <h2>"Create or choose a thread"</h2>
                                    <p>"Select a thread to view messages, jobs, and runtime execution progress."</p>
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
    let host = web_sys::window()
        .and_then(|window| window.location().hostname().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "localhost".to_string());

    format!("http://{host}:8080/api/v1")
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

async fn create_message(thread_id: &str, content: &str) -> Result<MessageRecord, String> {
    decode_json(
        Request::post(&format!("{}/threads/{thread_id}/messages", api_base()))
            .json(&CreateMessageRequest {
                role: "user".to_string(),
                content: content.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
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
