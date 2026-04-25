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

pub(crate) async fn fetch_repositories() -> Result<Vec<RepositoryOption>, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/repositories", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_devices() -> Result<Vec<DeviceRecord>, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/devices", api_base())))
            .send()
            .await
            .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_device_trust_events(
    device_id: &str,
) -> Result<Vec<DeviceTrustEventRecord>, String> {
    decode_json(
        with_credentials(Request::get(&format!(
            "{}/devices/{device_id}/trust/events",
            api_base()
        )))
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn fetch_orchestrator_signers()
-> Result<Vec<OrchestratorSignerStateRecord>, String> {
    decode_json(
        with_credentials(Request::get(&format!("{}/trust/signers", api_base())))
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_thread_message(
    thread_id: &str,
    source_message_id: &str,
    title: &str,
    device_id: Option<String>,
    target_kind: JobTargetKind,
    target_name: Option<String>,
    base_branch: Option<String>,
    prompt: Option<String>,
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
            target_kind,
            target_name,
            base_branch,
            device_id,
            prompt,
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_job(
    thread_id: &str,
    title: &str,
    device_id: Option<String>,
    target_kind: JobTargetKind,
    target_name: Option<String>,
    base_branch: Option<String>,
    prompt: &str,
    execution_intent: Option<ExecutionIntent>,
) -> Result<JobRecord, String> {
    let detail: JobDetail = decode_json(
        with_credentials(Request::post(&format!(
            "{}/threads/{thread_id}/jobs",
            api_base()
        )))
        .json(&CreateJobRequest {
            title: title.to_string(),
            target_kind,
            target_name,
            base_branch,
            prompt: prompt.to_string(),
            device_id,
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

pub(crate) async fn revoke_device_trust(
    device_id: &str,
    reason: Option<String>,
) -> Result<DeviceRecord, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/devices/{device_id}/trust/revoke",
            api_base()
        )))
        .json(&TrustLifecycleActionRequest { reason })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}

pub(crate) async fn confirm_device_trust_rotation(
    device_id: &str,
    reason: Option<String>,
) -> Result<DeviceRecord, String> {
    device_trust_action(device_id, "confirm-rotation", reason).await
}

pub(crate) async fn resolve_device_trust_attention(
    device_id: &str,
    reason: Option<String>,
) -> Result<DeviceRecord, String> {
    device_trust_action(device_id, "resolve-attention", reason).await
}

pub(crate) async fn clear_device_trust_revocation(
    device_id: &str,
    reason: Option<String>,
) -> Result<DeviceRecord, String> {
    device_trust_action(device_id, "clear-revocation", reason).await
}

pub(crate) async fn stage_orchestrator_signer(
    key_id: &str,
    reason: Option<String>,
) -> Result<OrchestratorSignerStateRecord, String> {
    signer_action(key_id, "stage", reason).await
}

pub(crate) async fn activate_orchestrator_signer(
    key_id: &str,
    reason: Option<String>,
) -> Result<OrchestratorSignerStateRecord, String> {
    signer_action(key_id, "activate", reason).await
}

pub(crate) async fn retire_orchestrator_signer(
    key_id: &str,
    reason: Option<String>,
) -> Result<OrchestratorSignerStateRecord, String> {
    signer_action(key_id, "retire", reason).await
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

async fn device_trust_action(
    device_id: &str,
    action: &str,
    reason: Option<String>,
) -> Result<DeviceRecord, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/devices/{device_id}/trust/{action}",
            api_base()
        )))
        .json(&TrustLifecycleActionRequest { reason })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}

async fn signer_action(
    key_id: &str,
    action: &str,
    reason: Option<String>,
) -> Result<OrchestratorSignerStateRecord, String> {
    decode_json(
        with_credentials(Request::post(&format!(
            "{}/trust/signers/{key_id}/{action}",
            api_base()
        )))
        .json(&TrustLifecycleActionRequest { reason })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?,
    )
    .await
}
