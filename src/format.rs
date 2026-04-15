//! Pure UI formatting helpers.

use js_sys::Date;
use serde_json::Value;
use wasm_bindgen::JsValue;

use crate::models::{ExecutionDraft, ExecutionIntent, MessageRecord};

pub(crate) fn format_json_value(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub(crate) fn report_status_label(report: &Value, key: &str) -> String {
    report
        .get(key)
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn report_diff_stat(report: &Value) -> Option<String> {
    report
        .get("diff_stat")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(crate) fn report_last_message(report: &Value) -> Option<String> {
    report
        .get("last_message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn report_array_strings(report: &Value, key: &str) -> Vec<String> {
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

pub(crate) fn message_execution_draft(message: &MessageRecord) -> Option<ExecutionDraft> {
    message
        .payload_json
        .get("execution_draft")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub(crate) fn message_result_details(message: &MessageRecord) -> Option<String> {
    message
        .payload_json
        .get("job_result")
        .and_then(|value| value.get("details"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_job_completion_status(status: &str) -> bool {
    status.ends_with(":completed")
        || status.ends_with(":failed")
        || status.ends_with(":push_completed")
}

pub(crate) fn message_is_result_surface(message: &MessageRecord) -> bool {
    is_job_completion_status(&message.status) || message.status.ends_with(":awaiting_approval")
}

pub(crate) fn message_timestamp_label(value: &str) -> String {
    let date = Date::new(&JsValue::from_str(value));
    if date.get_time().is_nan() {
        return value.to_string();
    }

    let now = Date::new_0();
    let time_label = format_local_time(&date);
    if is_same_local_day(&date, &now) {
        time_label
    } else {
        format!("{} {}", format_local_month_day(&date), time_label)
    }
}

pub(crate) fn execution_intent_label(intent: &ExecutionIntent) -> &'static str {
    match intent {
        ExecutionIntent::WorkspaceChange => "Workspace Change",
        ExecutionIntent::ReadOnly => "Read-Only Investigation",
    }
}

pub(crate) fn message_mode_class(message: &MessageRecord) -> &'static str {
    if message.status == "conversation.reply" && message_execution_draft(message).is_some() {
        "mode-conversation mode-draft-ready"
    } else if message.status == "conversation.reply" {
        "mode-conversation"
    } else if message.status == "workflow.handoff.created" {
        "mode-handoff"
    } else if message.status == "workflow.dispatch.created" {
        "mode-dispatch"
    } else if is_job_completion_status(&message.status) {
        "mode-job-complete"
    } else if message.status.starts_with("job_event:") {
        "mode-job-update"
    } else {
        ""
    }
}

pub(crate) fn message_mode_badge(message: &MessageRecord) -> Option<(&'static str, &'static str)> {
    if message.status == "conversation.reply" && message_execution_draft(message).is_some() {
        Some(("conversation", "Draft Ready"))
    } else if message.status == "conversation.reply" {
        Some(("conversation", "Conversation"))
    } else if message.status == "workflow.handoff.created" {
        Some(("handoff", "Workflow Handoff"))
    } else if message.status == "workflow.dispatch.created" {
        Some(("dispatch", "Direct Dispatch"))
    } else if message.status.ends_with(":failed") {
        Some(("job-complete failed", "Job Failed"))
    } else if message.status.ends_with(":push_completed") {
        Some(("job-complete", "Push Complete"))
    } else if message.status.ends_with(":awaiting_approval") {
        Some(("job-complete awaiting-approval", "Ready To Push"))
    } else if message.status.ends_with(":completed") {
        Some(("job-complete", "Job Complete"))
    } else if message.status.ends_with(":push_started") {
        Some(("job-update pushing", "Job Update"))
    } else if message.status.ends_with(":started") || message.status.ends_with(":running") {
        Some(("job-update running", "Job Update"))
    } else if message.status.starts_with("job_event:") {
        Some(("job-update", "Job Update"))
    } else if message.role == "system" {
        Some(("system", "System"))
    } else {
        None
    }
}

pub(crate) fn status_badge_class(status: &str) -> &'static str {
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

pub(crate) fn approval_status_note(approval_status: &str, job_status: &str) -> String {
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

pub(crate) fn format_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join("\n")
    }
}

fn is_same_local_day(left: &Date, right: &Date) -> bool {
    left.get_full_year() == right.get_full_year()
        && left.get_month() == right.get_month()
        && left.get_date() == right.get_date()
}

fn format_local_time(date: &Date) -> String {
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let period = if hours >= 12 { "PM" } else { "AM" };
    let display_hour = match hours % 12 {
        0 => 12,
        hour => hour,
    };

    format!("{display_hour}:{minutes:02} {period}")
}

fn format_local_month_day(date: &Date) -> String {
    let month = match date.get_month() {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        11 => "Dec",
        _ => "Date",
    };

    format!("{month} {}", date.get_date())
}

#[cfg(test)]
mod tests {
    use super::{
        execution_intent_label, message_execution_draft, message_is_result_surface,
        message_mode_badge, message_mode_class, message_result_details, report_last_message,
    };
    use crate::models::ExecutionIntent;
    use crate::models::MessageRecord;
    use serde_json::json;

    #[test]
    fn extracts_last_message_from_report() {
        assert_eq!(
            report_last_message(&json!({ "last_message": " done " })),
            Some("done".to_string())
        );
    }

    #[test]
    fn extracts_execution_draft_from_payload() {
        let message = MessageRecord {
            id: "m1".into(),
            role: "assistant".into(),
            content: String::new(),
            status: "conversation.reply".into(),
            payload_json: json!({
                "execution_draft": {
                    "title": "Update README",
                    "repo_name": "elowen-api",
                    "base_branch": "main",
                    "request_text": "Update the README",
                    "execution_intent": "workspace_change",
                    "source_message_id": "m1",
                    "source_role": "assistant",
                    "rationale": "test"
                }
            }),
            created_at: String::new(),
        };

        assert_eq!(
            message_execution_draft(&message).map(|draft| draft.title),
            Some("Update README".to_string())
        );
    }

    #[test]
    fn labels_read_only_execution_intent() {
        assert_eq!(
            execution_intent_label(&ExecutionIntent::ReadOnly),
            "Read-Only Investigation"
        );
    }

    #[test]
    fn extracts_message_result_details() {
        let message = MessageRecord {
            id: "m2".into(),
            role: "assistant".into(),
            content: "result".into(),
            status: "job_event:job:completed".into(),
            payload_json: json!({
                "job_result": {
                    "details": "Detailed job metadata"
                }
            }),
            created_at: String::new(),
        };

        assert_eq!(
            message_result_details(&message),
            Some("Detailed job metadata".to_string())
        );
    }

    #[test]
    fn marks_completed_job_messages_as_job_complete() {
        let message = MessageRecord {
            id: "m3".into(),
            role: "assistant".into(),
            content: "done".into(),
            status: "job_event:job:completed".into(),
            payload_json: json!({}),
            created_at: String::new(),
        };

        assert_eq!(message_mode_class(&message), "mode-job-complete");
        assert_eq!(
            message_mode_badge(&message),
            Some(("job-complete", "Job Complete"))
        );
    }

    #[test]
    fn keeps_started_job_messages_as_job_updates() {
        let message = MessageRecord {
            id: "m4".into(),
            role: "assistant".into(),
            content: "started".into(),
            status: "job_event:job:started".into(),
            payload_json: json!({}),
            created_at: String::new(),
        };

        assert_eq!(message_mode_class(&message), "mode-job-update");
        assert_eq!(
            message_mode_badge(&message),
            Some(("job-update running", "Job Update"))
        );
    }

    #[test]
    fn treats_awaiting_approval_as_result_surface() {
        let message = MessageRecord {
            id: "m5".into(),
            role: "assistant".into(),
            content: "awaiting".into(),
            status: "job_event:job:awaiting_approval".into(),
            payload_json: json!({}),
            created_at: String::new(),
        };

        assert!(message_is_result_surface(&message));
        assert_eq!(
            message_mode_badge(&message),
            Some(("job-complete awaiting-approval", "Ready To Push"))
        );
    }
}
