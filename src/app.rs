use gloo_net::http::{Request, Response};
use gloo_timers::future::TimeoutFuture;
use leptos::{ev, prelude::*, task::spawn_local};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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
struct ThreadDetail {
    #[serde(flatten)]
    thread: ThreadRecord,
    messages: Vec<MessageRecord>,
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

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (selected_thread_id, set_selected_thread_id) = signal(None::<String>);
    let (selected_thread, set_selected_thread) = signal(None::<ThreadDetail>);
    let (new_thread_title, set_new_thread_title) = signal(String::new());
    let (new_message_content, set_new_message_content) = signal(String::new());
    let (status_text, set_status_text) = signal(String::from("Loading threads..."));

    spawn_local({
        let set_threads = set_threads;
        let selected_thread_id = selected_thread_id;
        let set_selected_thread_id = set_selected_thread_id;
        let set_status_text = set_status_text;
        let set_selected_thread = set_selected_thread;

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
            }
        }
    });

    Effect::new({
        let selected_thread_id = selected_thread_id;
        let set_selected_thread = set_selected_thread;
        let set_status_text = set_status_text;

        move |_| {
            if let Some(thread_id) = selected_thread_id.get() {
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
                .app-shell {
                    min-height: 100vh;
                    padding: 24px;
                }
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
                .sidebar {
                    padding: 20px;
                    display: flex;
                    flex-direction: column;
                    gap: 18px;
                }
                .content {
                    padding: 24px;
                    min-height: 70vh;
                }
                .eyebrow {
                    text-transform: uppercase;
                    letter-spacing: 0.12em;
                    font-size: 0.75rem;
                    color: var(--muted);
                    margin: 0 0 8px 0;
                }
                h1, h2, h3, p {
                    margin-top: 0;
                }
                h1 {
                    font-size: 2.3rem;
                    margin-bottom: 6px;
                }
                .status {
                    color: var(--muted);
                    font-size: 0.95rem;
                    margin-bottom: 18px;
                }
                form {
                    display: grid;
                    gap: 10px;
                }
                input, textarea, button {
                    font: inherit;
                }
                input, textarea {
                    width: 100%;
                    border: 1px solid var(--line);
                    border-radius: 14px;
                    padding: 12px 14px;
                    background: #fff;
                    color: var(--ink);
                }
                textarea {
                    min-height: 110px;
                    resize: vertical;
                }
                button {
                    border: none;
                    border-radius: 999px;
                    padding: 11px 16px;
                    background: var(--accent);
                    color: white;
                    cursor: pointer;
                    transition: transform 120ms ease, opacity 120ms ease;
                }
                button:hover {
                    transform: translateY(-1px);
                    opacity: 0.95;
                }
                .thread-list {
                    display: grid;
                    gap: 10px;
                }
                .thread-card {
                    border: 1px solid var(--line);
                    border-radius: 16px;
                    padding: 14px;
                    background: #fff;
                    cursor: pointer;
                }
                .thread-card.active {
                    border-color: var(--accent);
                    background: var(--accent-soft);
                }
                .thread-card h3 {
                    margin-bottom: 6px;
                    font-size: 1.05rem;
                }
                .thread-meta {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    color: var(--muted);
                    font-size: 0.85rem;
                }
                .message-list {
                    display: grid;
                    gap: 14px;
                    margin: 24px 0;
                }
                .message {
                    border-radius: 18px;
                    padding: 16px;
                    border: 1px solid var(--line);
                    background: #fff;
                }
                .message.user {
                    background: #fcf3e8;
                }
                .message.assistant {
                    background: #eef6f3;
                }
                .message.system {
                    background: #f5f0fb;
                }
                .message header {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    color: var(--muted);
                    font-size: 0.82rem;
                    margin-bottom: 8px;
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
                    .frame {
                        grid-template-columns: 1fr;
                    }
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

                                        if let Err(error) = sync_thread_list(
                                            set_threads,
                                            selected_thread_id,
                                            set_selected_thread_id,
                                            set_status_text,
                                        )
                                        .await
                                        {
                                            set_status_text
                                                .set(format!("Failed to refresh threads: {error}"));
                                        }
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
                            view! {
                                <div>
                                    <p class="eyebrow">"Thread Detail"</p>
                                    <h2>{thread.thread.title.clone()}</h2>
                                    <p class="status">{format!("Status: {} | Updated: {}", thread.thread.status, thread.thread.updated_at)}</p>
                                    <div class="message-list">
                                        <For
                                            each=move || thread.messages.clone()
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
                                            let thread_id = thread_id.clone();

                                            async move {
                                                match create_message(&thread_id, &content).await {
                                                    Ok(_) => {
                                                        set_new_message_content.set(String::new());
                                                        set_status_text.set("Message posted.".to_string());

                                                        if let Err(error) = sync_selected_thread(
                                                            thread_id.clone(),
                                                            set_selected_thread,
                                                            set_status_text,
                                                        )
                                                        .await
                                                        {
                                                            set_status_text.set(format!(
                                                                "Failed to refresh thread: {error}"
                                                            ));
                                                        }

                                                        if let Err(error) = sync_thread_list(
                                                            set_threads,
                                                            selected_thread_id,
                                                            set_selected_thread_id,
                                                            set_status_text,
                                                        )
                                                        .await
                                                        {
                                                            set_status_text.set(format!(
                                                                "Failed to refresh threads: {error}"
                                                            ));
                                                        }
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
                                    <p>"Slice 1 is focused on the core conversation surface: thread list, thread detail, and persisted messages."</p>
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
