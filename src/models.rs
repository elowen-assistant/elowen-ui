//! UI-side DTOs mirrored from the orchestration API.
//!
//! Keep these in sync with the corresponding API response and request models in
//! `elowen-api`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct ThreadSummary {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) message_count: i64,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct ThreadRecord {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct MessageRecord {
    pub(crate) id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) status: String,
    pub(crate) payload_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionIntent {
    WorkspaceChange,
    ReadOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct ExecutionDraft {
    pub(crate) title: String,
    pub(crate) repo_name: Option<String>,
    pub(crate) base_branch: String,
    pub(crate) request_text: String,
    pub(crate) execution_intent: ExecutionIntent,
    pub(crate) source_message_id: String,
    pub(crate) source_role: String,
    pub(crate) rationale: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct JobRecord {
    pub(crate) id: String,
    pub(crate) short_id: String,
    pub(crate) correlation_id: String,
    pub(crate) thread_id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) result: Option<String>,
    pub(crate) failure_class: Option<String>,
    pub(crate) repo_name: String,
    pub(crate) device_id: Option<String>,
    pub(crate) branch_name: Option<String>,
    pub(crate) base_branch: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct JobEventRecord {
    pub(crate) id: String,
    pub(crate) correlation_id: String,
    pub(crate) event_type: String,
    pub(crate) payload_json: Value,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct ThreadDetail {
    #[serde(flatten)]
    pub(crate) thread: ThreadRecord,
    pub(crate) messages: Vec<MessageRecord>,
    pub(crate) jobs: Vec<JobRecord>,
    pub(crate) related_notes: Vec<NoteRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct JobDetail {
    #[serde(flatten)]
    pub(crate) job: JobRecord,
    pub(crate) execution_report_json: Value,
    pub(crate) summary: Option<SummaryRecord>,
    pub(crate) approvals: Vec<ApprovalRecord>,
    pub(crate) related_notes: Vec<NoteRecord>,
    pub(crate) events: Vec<JobEventRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct NoteRecord {
    pub(crate) note_id: String,
    pub(crate) title: String,
    pub(crate) slug: String,
    pub(crate) summary: String,
    pub(crate) tags: Vec<String>,
    pub(crate) aliases: Vec<String>,
    pub(crate) note_type: String,
    pub(crate) source_kind: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) current_revision_id: String,
    pub(crate) updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct SummaryRecord {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) source_id: String,
    pub(crate) version: i32,
    pub(crate) content: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct ApprovalRecord {
    pub(crate) id: String,
    pub(crate) thread_id: String,
    pub(crate) job_id: String,
    pub(crate) action_type: String,
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) resolved_by: Option<String>,
    pub(crate) resolution_reason: Option<String>,
    pub(crate) created_at: String,
    pub(crate) resolved_at: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiError {
    pub(crate) error: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateThreadRequest {
    pub(crate) title: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateChatDispatchRequest {
    pub(crate) content: String,
    pub(crate) title: String,
    pub(crate) repo_name: String,
    pub(crate) base_branch: String,
    pub(crate) execution_intent: Option<ExecutionIntent>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateThreadChatRequest {
    pub(crate) content: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DispatchThreadMessageRequest {
    pub(crate) source_message_id: String,
    pub(crate) title: String,
    pub(crate) repo_name: String,
    pub(crate) base_branch: String,
    pub(crate) request_text: Option<String>,
    pub(crate) execution_intent: Option<ExecutionIntent>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateJobRequest {
    pub(crate) title: String,
    pub(crate) repo_name: String,
    pub(crate) base_branch: String,
    pub(crate) request_text: String,
    pub(crate) execution_intent: Option<ExecutionIntent>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResolveApprovalRequest {
    pub(crate) status: String,
    pub(crate) resolved_by: String,
    pub(crate) reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatDispatchResponse {
    pub(crate) message: MessageRecord,
    pub(crate) acknowledgement: MessageRecord,
    pub(crate) job: JobRecord,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatReplyResponse {
    pub(crate) user_message: MessageRecord,
    pub(crate) assistant_message: MessageRecord,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MessageDispatchResponse {
    pub(crate) source_message: MessageRecord,
    pub(crate) acknowledgement: MessageRecord,
    pub(crate) job: JobRecord,
}

#[derive(Debug, Serialize)]
pub(crate) struct PromoteJobNoteRequest {
    pub(crate) title: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) body_markdown: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) aliases: Vec<String>,
    pub(crate) note_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct AuthSessionStatus {
    pub(crate) enabled: bool,
    pub(crate) authenticated: bool,
    pub(crate) operator_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginRequest {
    pub(crate) password: String,
}
