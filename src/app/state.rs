use leptos::prelude::*;
use web_sys::EventSource;

use crate::models::{AuthSessionStatus, JobDetail, JobRecord, ThreadDetail, ThreadSummary};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NavMode {
    Chats,
    Jobs,
    Details,
}

impl NavMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Chats => "chats",
            Self::Jobs => "jobs",
            Self::Details => "details",
        }
    }

    pub(super) fn from_storage(value: &str) -> Self {
        match value {
            "jobs" => Self::Jobs,
            "details" => Self::Details,
            _ => Self::Chats,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RealtimeStatus {
    Connecting,
    Connected,
    Degraded,
    Disconnected,
}

impl RealtimeStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Realtime connecting",
            Self::Connected => "Realtime connected",
            Self::Degraded => "Realtime degraded",
            Self::Disconnected => "Realtime offline",
        }
    }

    pub(super) fn class(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Degraded => "degraded",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct UiEventSyncHandles {
    pub(super) selected_thread_id: ReadSignal<Option<String>>,
    pub(super) selected_job_id: ReadSignal<Option<String>>,
    pub(super) set_threads: WriteSignal<Vec<ThreadSummary>>,
    pub(super) set_selected_thread_id: WriteSignal<Option<String>>,
    pub(super) set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    pub(super) set_jobs: WriteSignal<Vec<JobRecord>>,
    pub(super) set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    pub(super) set_auth_session: WriteSignal<Option<AuthSessionStatus>>,
    pub(super) set_status_text: WriteSignal<String>,
    pub(super) set_realtime_status: WriteSignal<RealtimeStatus>,
    pub(super) set_event_source: WriteSignal<Option<EventSource>>,
}

#[derive(Clone, Copy)]
pub(super) struct SignedOutHandles {
    pub(super) set_auth_session: WriteSignal<Option<AuthSessionStatus>>,
    pub(super) set_status_text: WriteSignal<String>,
    pub(super) set_selected_thread: WriteSignal<Option<ThreadDetail>>,
    pub(super) set_selected_thread_id: WriteSignal<Option<String>>,
    pub(super) set_selected_job_detail: WriteSignal<Option<JobDetail>>,
    pub(super) set_selected_job_id: WriteSignal<Option<String>>,
    pub(super) set_threads: WriteSignal<Vec<ThreadSummary>>,
    pub(super) set_jobs: WriteSignal<Vec<JobRecord>>,
}

pub(super) const STORAGE_SELECTED_THREAD_ID: &str = "elowen.selected_thread_id";
pub(super) const STORAGE_SELECTED_JOB_ID: &str = "elowen.selected_job_id";
pub(super) const STORAGE_CONTEXT_OPEN: &str = "elowen.context_open";
pub(super) const STORAGE_NAV_MODE: &str = "elowen.nav_mode";
pub(super) const STORAGE_COMPOSER_TEXT: &str = "elowen.composer_text";
pub(super) const POLL_FALLBACK_MS: u32 = 30_000;
