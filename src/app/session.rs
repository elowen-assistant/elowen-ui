use leptos::prelude::*;

use crate::models::{AuthSessionStatus, JobDetail, ThreadDetail};

use super::state::SignedOutHandles;

fn expired_auth_session() -> AuthSessionStatus {
    AuthSessionStatus {
        enabled: true,
        authenticated: false,
        operator_label: None,
    }
}

pub(super) fn apply_session_expired_state(
    set_auth_session: WriteSignal<Option<AuthSessionStatus>>,
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    set_status_text: WriteSignal<String>,
) {
    set_auth_session.set(Some(expired_auth_session()));
    set_selected_thread.set(None);
    set_selected_job_detail.set(None);
    set_status_text.set("Session expired. Sign in again.".to_string());
}

pub(super) fn apply_polled_session_expired_state(
    set_auth_session: WriteSignal<Option<AuthSessionStatus>>,
    set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    set_selected_job_id: WriteSignal<Option<String>>,
    set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    set_status_text: WriteSignal<String>,
) {
    apply_session_expired_state(
        set_auth_session,
        set_selected_thread,
        set_selected_job_detail,
        set_status_text,
    );
    set_selected_job_id.set(None);
}

pub(super) fn apply_signed_out_state(session: AuthSessionStatus, handles: SignedOutHandles) {
    handles.set_auth_session.set(Some(session));
    handles.set_status_text.set("Signed out.".to_string());
    handles.set_selected_thread.set(None);
    handles.set_selected_thread_id.set(None);
    handles.set_selected_job_detail.set(None);
    handles.set_selected_job_id.set(None);
    handles.set_threads.set(Vec::new());
    handles.set_jobs.set(Vec::new());
}
