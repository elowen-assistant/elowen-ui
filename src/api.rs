//! API client helpers for the browser UI.

use gloo_net::http::{Request, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use web_sys::RequestCredentials;

use crate::models::*;

fn api_base() -> String {
    if let Some(origin) = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .filter(|value| !value.is_empty() && value != "null")
    {
        return format!("{origin}/api/v1");
    }

    "http://localhost:8080/api/v1".to_string()
}

fn with_credentials(request: RequestBuilder) -> RequestBuilder {
    request.credentials(RequestCredentials::Include)
}

fn auth_url(path: &str) -> String {
    format!("{}/auth/{path}", api_base())
}

pub(crate) fn events_url() -> String {
    format!("{}/events", api_base())
}

pub(crate) async fn fetch_threads() -> Result<Vec<ThreadSummary>, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/threads", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_jobs() -> Result<Vec<JobRecord>, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/jobs", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_thread(thread_id: &str) -> Result<ThreadDetail, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/threads/{thread_id}", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_job(job_id: &str) -> Result<JobDetail, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/jobs/{job_id}", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn create_thread(title: &str) -> Result<ThreadDetail, String> {
    decode_json(
        with_credentials(Request::post(&format!("{}/threads", api_base())))
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

pub(crate) async fn send_thread_chat_message(
    thread_id: &str,
    content: &str,
) -> Result<ChatReplyResponse, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/threads/{thread_id}/chat",
            api_base()
        )))
        .json(&CreateThreadChatRequest {
            content: content.to_string(),
        })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn dispatch_thread_message(
    thread_id: &str,
    source_message_id: &str,
    title: &str,
    repo_name: &str,
    base_branch: &str,
    request_text: Option<String>,
    execution_intent: Option<ExecutionIntent>,
) -> Result<JobRecord, String> {
    let response: MessageDispatchResponse = decode_json(
        with_credentials(Request::post(&format!(
            "{}/threads/{thread_id}/message-dispatch",
            api_base()
        )))
        .json(&DispatchThreadMessageRequest {
            source_message_id: source_message_id.to_string(),
            title: title.to_string(),
            repo_name: repo_name.to_string(),
            base_branch: base_branch.to_string(),
            request_text,
            execution_intent,
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

pub(crate) async fn create_job(
    thread_id: &str,
    title: &str,
    repo_name: &str,
    base_branch: &str,
    request_text: &str,
    execution_intent: Option<ExecutionIntent>,
) -> Result<JobRecord, String> {
    let detail: JobDetail = decode_json(
        with_credentials(Request::post(&format!(
            "{}/threads/{thread_id}/jobs",
            api_base()
        )))
        .json(&CreateJobRequest {
            title: title.to_string(),
            repo_name: repo_name.to_string(),
            base_branch: base_branch.to_string(),
            request_text: request_text.to_string(),
            execution_intent,
        })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await?;

    Ok(detail.job)
}

pub(crate) async fn promote_job_note(job_id: &str) -> Result<NoteRecord, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/jobs/{job_id}/notes/promote",
            api_base()
        )))
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

pub(crate) async fn resolve_approval(
    approval_id: &str,
    status: &str,
    reason: &str,
) -> Result<ApprovalRecord, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/approvals/{approval_id}/resolve",
            api_base()
        )))
        .json(&ResolveApprovalRequest {
            status: status.to_string(),
            reason: reason.to_string(),
        })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_auth_session() -> Result<AuthSessionStatus, String> {
    decode_json(
        with_credentials(Request::get(&auth_url("session")))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn login(
    username: Option<&str>,
    password: &str,
) -> Result<AuthSessionStatus, String> {
    decode_json(
        with_credentials(Request::post(&auth_url("login")))
            .json(&LoginRequest {
                username: username.map(str::to_string),
                password: password.to_string(),
            })
            .map_err(|error| error.to_string())?
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn logout() -> Result<AuthSessionStatus, String> {
    decode_json(
        with_credentials(Request::post(&auth_url("logout")))
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
