use gloo_timers::future::TimeoutFuture;
use leptos::{ev, html, prelude::*, task::spawn_local};

use crate::{
    api::{
        create_job, create_thread, dispatch_thread_message, fetch_auth_session, fetch_job,
        fetch_jobs, fetch_thread, fetch_threads, login as login_session, logout as logout_session,
        promote_job_note, resolve_approval, send_thread_chat_message,
    },
    format::{
        approval_status_note, execution_intent_label, format_json_value, format_string_list,
        message_execution_draft, message_mode_badge, message_mode_class, message_result_details,
        report_array_strings, report_diff_stat, report_last_message, report_status_label,
        status_badge_class,
    },
    models::*,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavMode {
    Chats,
    Jobs,
    Details,
}

#[component]
pub fn App() -> impl IntoView {
    let (threads, set_threads) = signal(Vec::<ThreadSummary>::new());
    let (jobs, set_jobs) = signal(Vec::<JobRecord>::new());
    let (sidebar_open, set_sidebar_open) = signal(!is_compact_layout());
    let (context_open, set_context_open) = signal(false);
    let (nav_mode, set_nav_mode) = signal(NavMode::Chats);
    let (auth_session, set_auth_session) = signal(None::<AuthSessionStatus>);
    let (auth_password, set_auth_password) = signal(String::new());
    let (auth_error, set_auth_error) = signal(String::new());
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
    let message_pane_ref = NodeRef::<html::Div>::new();

    spawn_local(async move {
        match fetch_auth_session().await {
            Ok(session) => {
                let can_access = !session.enabled || session.authenticated;
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

                    if let Some(thread_id) = selected_thread_id.get_untracked()
                        && let Err(error) =
                            sync_selected_thread(thread_id, set_selected_thread, set_status_text)
                                .await
                    {
                        set_status_text.set(format!("Failed to load thread: {error}"));
                    }
                } else {
                    set_status_text.set("Sign in required.".to_string());
                }
            }
            Err(error) => {
                set_status_text.set(format!("Failed to check auth session: {error}"));
            }
        }

        loop {
            TimeoutFuture::new(5_000).await;

            let can_access = auth_session
                .get_untracked()
                .map(|session| !session.enabled || session.authenticated)
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
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
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
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
                    set_selected_thread.set(None);
                    set_selected_job_detail.set(None);
                    set_selected_job_id.set(None);
                    set_status_text.set("Session expired. Sign in again.".to_string());
                    continue;
                }
                set_status_text.set(format!("Failed to poll jobs: {error}"));
            }

            if let Some(thread_id) = selected_thread_id.get_untracked()
                && let Err(error) =
                    sync_selected_thread(thread_id, set_selected_thread, set_status_text).await
            {
                if is_auth_error(&error) {
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
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
                    set_auth_session.set(Some(AuthSessionStatus {
                        enabled: true,
                        authenticated: false,
                        operator_label: None,
                    }));
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
        if let Some(thread_id) = selected_thread_id.get() {
            set_selected_job_id.set(None);
            set_selected_job_detail.set(None);

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
            .map(|thread| thread.messages.len())
            .unwrap_or_default();

        if let Some(message_pane) = message_pane_ref.get() {
            message_pane.set_scroll_top(message_pane.scroll_height());
        }
    });

    view! {
        <main class="app-shell">
            <style>
                {r#"
                @import url('https://fonts.googleapis.com/css2?family=Material+Symbols+Rounded:opsz,wght,FILL,GRAD@20..48,500,0,0');
                :root {
                    --bg: #f9f4fb;
                    --bg-alt: #f2ecf4;
                    --surface: #fffbff;
                    --surface-container-lowest: #ffffff;
                    --surface-container-low: #f8f2fa;
                    --surface-container: #f1ebf3;
                    --surface-container-high: #ebe5ed;
                    --surface-container-highest: #e5dfe7;
                    --primary: #5c5fbe;
                    --primary-container: #e2e0ff;
                    --on-primary-container: #1b1d5a;
                    --secondary: #5d5f72;
                    --secondary-container: #e2e2f9;
                    --tertiary: #4e6354;
                    --tertiary-container: #d0e8d4;
                    --error-container: #f9dedc;
                    --ink: #1c1b1f;
                    --muted: #625b71;
                    --line: #cbc4d0;
                    --outline-strong: #938f99;
                    --scrim: rgba(31, 27, 36, 0.34);
                    --shadow-color: rgba(35, 28, 44, 0.16);
                    --elevation-1: 0 1px 2px rgba(35, 28, 44, 0.12), 0 1px 3px rgba(35, 28, 44, 0.08);
                    --elevation-2: 0 2px 6px rgba(35, 28, 44, 0.1), 0 8px 18px rgba(35, 28, 44, 0.08);
                    --elevation-3: 0 3px 10px rgba(35, 28, 44, 0.12), 0 18px 36px rgba(35, 28, 44, 0.1);
                    --shape-xs: 12px;
                    --shape-sm: 16px;
                    --shape-md: 24px;
                    --shape-lg: 30px;
                    --shape-xl: 36px;
                    --accent: var(--primary);
                    --accent-soft: var(--primary-container);
                }
                * { box-sizing: border-box; }
                .material-symbols-rounded {
                    font-family: "Material Symbols Rounded";
                    font-weight: normal;
                    font-style: normal;
                    font-size: 24px;
                    line-height: 1;
                    letter-spacing: normal;
                    text-transform: none;
                    display: inline-block;
                    white-space: nowrap;
                    word-wrap: normal;
                    direction: ltr;
                    -webkit-font-smoothing: antialiased;
                    font-variation-settings: "FILL" 0, "wght" 500, "GRAD" 0, "opsz" 24;
                }
                body {
                    margin: 0;
                    background:
                        radial-gradient(circle at top left, rgba(92, 95, 190, 0.16), transparent 24%),
                        radial-gradient(circle at top right, rgba(93, 95, 114, 0.08), transparent 28%),
                        linear-gradient(180deg, var(--bg) 0%, var(--bg-alt) 100%);
                    color: var(--ink);
                    font-family: "Segoe UI Variable Text", "Segoe UI", "Roboto", "Noto Sans", system-ui, sans-serif;
                    overflow-x: hidden;
                }
                .app-shell { min-height: 100vh; padding: 24px; overflow-x: hidden; }
                .workspace-shell {
                    max-width: 1320px;
                    margin: 0 auto;
                    display: grid;
                    gap: 12px;
                    min-height: calc(100vh - 48px);
                }
                .app-topbar {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 12px;
                    padding: 10px 14px;
                    border-radius: 22px;
                    min-width: 0;
                }
                .app-topbar-leading {
                    display: flex;
                    align-items: center;
                    gap: 14px;
                    min-width: 0;
                }
                .app-topbar-copy {
                    min-width: 0;
                    display: grid;
                    gap: 2px;
                }
                .app-topbar-copy h2 {
                    margin: 0;
                    font-size: 1.12rem;
                    line-height: 1.2;
                    letter-spacing: -0.02em;
                }
                .topbar-subtitle {
                    margin: 0;
                    color: var(--muted);
                    font-size: 0.82rem;
                }
                .app-topbar-actions {
                    display: flex;
                    align-items: center;
                    justify-content: flex-end;
                    gap: 10px;
                    flex-wrap: wrap;
                }
                .topbar-chip {
                    display: inline-flex;
                    align-items: center;
                    border-radius: 999px;
                    padding: 6px 10px;
                    background: var(--surface-container);
                    color: var(--muted);
                    border: 1px solid rgba(147, 143, 153, 0.24);
                    font-size: 0.74rem;
                    font-weight: 600;
                }
                .topbar-chip.operator {
                    max-width: 220px;
                    overflow: hidden;
                    text-overflow: ellipsis;
                    white-space: nowrap;
                }
                .nav-fab {
                    display: none;
                    min-width: 48px;
                    min-height: 48px;
                    padding: 0 18px;
                    border-radius: 16px;
                    background: var(--primary-container);
                    color: var(--on-primary-container);
                    box-shadow: var(--elevation-2);
                }
                .frame {
                    display: grid;
                    grid-template-columns: 84px 308px 1fr;
                    gap: 14px;
                    align-items: start;
                    position: relative;
                    height: calc(100vh - 108px);
                    min-height: calc(100vh - 108px);
                }
                .panel {
                    background: color-mix(in srgb, var(--surface) 92%, transparent);
                    border: 1px solid color-mix(in srgb, var(--line) 78%, transparent);
                    border-radius: var(--shape-xl);
                    box-shadow: var(--elevation-2);
                    backdrop-filter: blur(18px);
                    min-width: 0;
                }
                .sidebar-shell { min-width: 0; }
                .sidebar-backdrop,
                .sidebar-toggle,
                .sidebar-close {
                    display: none;
                }
                .sidebar { padding: 18px; display: flex; flex-direction: column; gap: 16px; }
                .sidebar-view {
                    display: grid;
                    gap: 16px;
                }
                .sidebar-view.hidden {
                    display: none;
                }
                .content {
                    padding: 20px;
                    min-height: 70vh;
                    min-width: 0;
                    height: 100%;
                    overflow: hidden;
                    background: color-mix(in srgb, var(--surface-container-lowest) 76%, transparent);
                    position: relative;
                }
                .content-toolbar { display: none; }
                .nav-rail {
                    height: 100%;
                    min-width: 0;
                    padding: 14px 8px;
                    border-radius: 32px;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 12px;
                    background: color-mix(in srgb, var(--surface-container-low) 86%, transparent);
                }
                .nav-rail-brand {
                    width: 48px;
                    height: 48px;
                    border-radius: 18px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    background: var(--primary-container);
                    color: var(--on-primary-container);
                    box-shadow: var(--elevation-1);
                }
                .nav-rail-items {
                    display: grid;
                    gap: 10px;
                    width: 100%;
                }
                .nav-rail-spacer { flex: 1; }
                .nav-rail-item {
                    width: 100%;
                    min-height: 64px;
                    padding: 6px 4px;
                    border-radius: 20px;
                    background: transparent;
                    color: var(--muted);
                    box-shadow: none;
                    display: grid;
                    justify-items: center;
                    align-content: center;
                    gap: 4px;
                    font-size: 0.68rem;
                    font-weight: 700;
                    letter-spacing: 0;
                }
                .nav-rail-item .material-symbols-rounded {
                    width: 44px;
                    height: 32px;
                    border-radius: 999px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 22px;
                }
                .nav-rail-item.active {
                    color: var(--on-primary-container);
                }
                .nav-rail-item.active .material-symbols-rounded {
                    background: var(--primary-container);
                    color: var(--on-primary-container);
                    font-variation-settings: "FILL" 1, "wght" 600, "GRAD" 0, "opsz" 24;
                }
                .nav-rail-item:hover {
                    transform: none;
                    box-shadow: none;
                    background: color-mix(in srgb, var(--surface-container-high) 74%, transparent);
                }
                .thread-mobile-meta {
                    display: none;
                }
                .context-backdrop,
                .context-toggle,
                .context-close {
                    display: none;
                }
                .auth-shell {
                    min-height: calc(100vh - 48px);
                    display: grid;
                    place-items: center;
                }
                .auth-card {
                    width: min(460px, 100%);
                    padding: 28px;
                    border: 1px solid var(--line);
                    border-radius: var(--shape-xl);
                    background: color-mix(in srgb, var(--surface) 96%, transparent);
                    box-shadow: var(--elevation-3);
                    display: grid;
                    gap: 16px;
                }
                .auth-actions {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    gap: 12px;
                    flex-wrap: wrap;
                }
                .auth-error {
                    margin: 0;
                    color: #8a2e2e;
                    font-size: 0.9rem;
                }
                .logout-button {
                    background: color-mix(in srgb, var(--secondary-container) 88%, white);
                    color: #2f2a3a;
                    box-shadow: var(--elevation-1);
                }
                .sidebar-header {
                    display: grid;
                    gap: 8px;
                }
                .sidebar-status {
                    display: grid;
                    gap: 4px;
                    padding: 10px 12px;
                    border: 1px solid rgba(92, 95, 190, 0.12);
                    border-radius: 18px;
                    background: color-mix(in srgb, var(--primary-container) 74%, var(--surface));
                }
                .sidebar-status.compact {
                    padding: 8px 10px;
                    border-radius: 14px;
                    background: color-mix(in srgb, var(--surface-container-high) 86%, transparent);
                }
                .sidebar-status .status {
                    margin: 0;
                    font-size: 0.86rem;
                }
                .eyebrow {
                    text-transform: uppercase;
                    letter-spacing: 0.12em;
                    font-size: 0.75rem;
                    color: var(--muted);
                    margin: 0 0 8px 0;
                }
                h1, h2, h3, p { margin-top: 0; }
                h1 { font-size: 1.65rem; margin-bottom: 4px; font-weight: 700; letter-spacing: -0.02em; }
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
                    padding: 6px 12px;
                    font-size: 0.74rem;
                    font-weight: 700;
                    letter-spacing: 0.04em;
                    text-transform: uppercase;
                    border: 1px solid rgba(147, 143, 153, 0.18);
                    background: color-mix(in srgb, var(--surface-container) 88%, transparent);
                    color: var(--ink);
                }
                .status-badge.pending { background: #ece7df; color: #54483a; }
                .status-badge.dispatched, .status-badge.accepted, .status-badge.running, .status-badge.pushing {
                    background: var(--primary-container);
                    color: var(--on-primary-container);
                }
                .status-badge.awaiting_approval { background: #f3e1b7; color: #6a4d12; }
                .status-badge.completed, .status-badge.approved, .status-badge.success {
                    background: var(--tertiary-container);
                    color: #21543d;
                }
                .status-badge.failed, .status-badge.rejected, .status-badge.failure { background: var(--error-container); color: #7a2f25; }
                form { display: grid; gap: 10px; }
                input, textarea, button { font: inherit; }
                input, textarea {
                    width: 100%;
                    border: 1px solid color-mix(in srgb, var(--outline-strong) 72%, transparent);
                    border-radius: 18px;
                    padding: 14px 16px;
                    background: color-mix(in srgb, var(--surface-container-low) 90%, transparent);
                    color: var(--ink);
                    transition: border-color 0.18s ease, box-shadow 0.18s ease, background 0.18s ease;
                }
                input:focus, textarea:focus {
                    outline: none;
                    border-color: var(--primary);
                    box-shadow: 0 0 0 3px rgba(92, 95, 190, 0.16);
                    background: var(--surface);
                }
                textarea { min-height: 110px; resize: vertical; }
                button {
                    border: none;
                    border-radius: 20px;
                    padding: 11px 18px;
                    background: var(--accent);
                    color: white;
                    cursor: pointer;
                    font-weight: 700;
                    letter-spacing: 0.01em;
                    box-shadow: var(--elevation-1);
                    transition: transform 0.14s ease, box-shadow 0.14s ease, background 0.14s ease, filter 0.14s ease;
                }
                button:hover {
                    transform: translateY(-1px);
                    box-shadow: var(--elevation-2);
                    filter: saturate(1.02);
                }
                button:active { transform: translateY(0); }
                .sidebar-section { display: grid; gap: 12px; }
                .sidebar-section + .sidebar-section { margin-top: 8px; }
                .thread-list { display: grid; gap: 8px; }
                .thread-list, .job-list, .message-list, .job-event-list, .summary-block, .approval-list, .report-grid {
                    min-width: 0;
                }
                .thread-card {
                    border: 1px solid var(--line);
                    border-radius: 18px;
                    padding: 10px 12px;
                    background: color-mix(in srgb, var(--surface-container-low) 90%, transparent);
                    cursor: pointer;
                    display: grid;
                    gap: 6px;
                }
                .thread-card.active {
                    border-color: rgba(92, 95, 190, 0.26);
                    background: color-mix(in srgb, var(--primary-container) 84%, var(--surface));
                    box-shadow: var(--elevation-1);
                }
                .thread-card-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 10px;
                }
                .thread-card h3 {
                    margin-bottom: 0;
                    font-size: 0.95rem;
                    font-weight: 650;
                    line-height: 1.3;
                }
                .thread-card p {
                    margin: 0;
                    color: var(--muted);
                    font-size: 0.79rem;
                    line-height: 1.35;
                }
                .thread-meta, .job-meta, .message header, .job-event header {
                    display: flex;
                    justify-content: space-between;
                    gap: 12px;
                    color: var(--muted);
                    font-size: 0.82rem;
                }
                .thread-meta {
                    justify-content: flex-start;
                    gap: 8px 12px;
                    flex-wrap: wrap;
                    font-size: 0.76rem;
                }
                .thread-status-dot {
                    width: 8px;
                    height: 8px;
                    border-radius: 999px;
                    background: var(--primary);
                    flex: 0 0 auto;
                    margin-top: 5px;
                }
                .thread-card-time {
                    font-size: 0.74rem;
                    color: var(--muted);
                    white-space: nowrap;
                }
                .message-list, .job-list, .job-event-list { display: grid; gap: 12px; }
                .job-card, .message, .job-event, .job-detail, .approval-card, .report-grid article, .note-card {
                    border: 1px solid var(--line);
                    border-radius: 24px;
                    padding: 16px;
                    background: color-mix(in srgb, var(--surface) 94%, transparent);
                    box-shadow: var(--elevation-1);
                }
                .job-card { cursor: pointer; }
                .job-card.active { border-color: var(--accent); background: color-mix(in srgb, var(--accent-soft) 82%, var(--surface)); box-shadow: var(--elevation-2); }
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
                .job-detail { background: color-mix(in srgb, var(--surface-container-lowest) 78%, transparent); margin: 0 0 24px 0; }
                .job-overview {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
                    gap: 12px;
                    margin: 16px 0;
                }
                .job-overview article {
                    border: 1px solid var(--line);
                    border-radius: 18px;
                    padding: 14px;
                    background: color-mix(in srgb, var(--surface-container-lowest) 84%, transparent);
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
                    background: color-mix(in srgb, var(--secondary-container) 86%, white);
                    color: #2f2a3a;
                    box-shadow: var(--elevation-1);
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
                .message.assistant { background: color-mix(in srgb, var(--primary-container) 42%, var(--surface)); }
                .message.system { background: color-mix(in srgb, var(--secondary-container) 40%, var(--surface)); }
                .message.mode-conversation { border-color: #7e8cc1; }
                .message.mode-draft-ready { box-shadow: inset 0 0 0 1px rgba(79, 93, 146, 0.12); }
                .message.mode-handoff { border-color: #9c7c44; background: #fbf5e7; }
                .message.mode-dispatch, .message.mode-job-update { border-color: #7e8cc1; }
                .message.mode-job-update {
                    background: color-mix(in srgb, var(--secondary-container) 34%, var(--surface));
                }
                .message.mode-job-complete {
                    border-color: color-mix(in srgb, var(--tertiary) 48%, white);
                    background: color-mix(in srgb, var(--tertiary-container) 40%, var(--surface));
                    box-shadow: inset 0 0 0 1px rgba(78, 99, 84, 0.08);
                }
                .thread-focus {
                    display: grid;
                    gap: 12px;
                    min-height: 0;
                    height: 100%;
                    grid-template-rows: auto minmax(0, 1fr);
                }
                .thread-hero {
                    display: grid;
                    gap: 8px;
                    padding: 12px 14px;
                    border: 1px solid var(--line);
                    border-radius: 20px;
                    background:
                        linear-gradient(145deg, color-mix(in srgb, var(--primary-container) 92%, white), color-mix(in srgb, var(--surface) 94%, transparent)),
                        color-mix(in srgb, var(--surface-container-lowest) 92%, transparent);
                    box-shadow: var(--elevation-1);
                }
                .thread-hero-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 12px;
                    flex-wrap: wrap;
                }
                .thread-hero h2 {
                    margin: 0;
                    font-size: 1.4rem;
                    line-height: 1.15;
                }
                .thread-hero .status {
                    margin: 0;
                    font-size: 0.82rem;
                }
                .thread-details-button {
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    gap: 6px;
                    border-radius: 18px;
                    border: 1px solid var(--line);
                    background: color-mix(in srgb, var(--surface) 96%, transparent);
                    color: var(--on-primary-container);
                    padding: 8px 12px;
                    font-size: 0.82rem;
                    font-weight: 700;
                    box-shadow: var(--elevation-1);
                }
                .thread-summary-row {
                    display: flex;
                    flex-wrap: wrap;
                    gap: 6px;
                }
                .thread-pill {
                    display: inline-flex;
                    align-items: center;
                    gap: 8px;
                    border-radius: 999px;
                    padding: 6px 10px;
                    background: color-mix(in srgb, var(--surface-container-lowest) 88%, transparent);
                    color: var(--muted);
                    font-size: 0.76rem;
                    border: 1px solid rgba(147, 143, 153, 0.2);
                }
                .thread-primary {
                    display: grid;
                    gap: 10px;
                    min-height: 0;
                    height: 100%;
                    grid-template-rows: minmax(0, 1fr) auto;
                    align-self: stretch;
                }
                .message-pane {
                    min-height: 0;
                    height: 100%;
                    overflow-y: auto;
                    padding: 2px 4px 16px 0;
                    scroll-behavior: smooth;
                }
                .context-shell {
                    display: grid;
                    gap: 8px;
                    align-content: start;
                }
                .context-shell-header {
                    display: none;
                }
                .context-panel {
                    border: 1px solid var(--line);
                    border-radius: 20px;
                    background: color-mix(in srgb, var(--surface-container-low) 84%, transparent);
                    overflow: hidden;
                    box-shadow: var(--elevation-1);
                }
                .context-panel summary {
                    cursor: pointer;
                    list-style: none;
                    padding: 10px 12px;
                    font-weight: 700;
                    font-size: 0.92rem;
                    color: var(--ink);
                    background: color-mix(in srgb, var(--surface-container-high) 82%, transparent);
                }
                .context-panel summary::-webkit-details-marker {
                    display: none;
                }
                .context-panel[open] summary {
                    border-bottom: 1px solid var(--line);
                }
                .context-panel-body {
                    padding: 10px 12px 12px 12px;
                    display: grid;
                    gap: 8px;
                }
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
                    border: 1px solid rgba(147, 143, 153, 0.18);
                    background: color-mix(in srgb, var(--surface-container) 88%, transparent);
                    color: var(--ink);
                }
                .mode-badge.conversation { background: var(--primary-container); color: #33457a; }
                .mode-badge.handoff { background: #f1e2c8; color: #6e4a1d; }
                .mode-badge.dispatch { background: #d6e3ff; color: #2f4b7f; }
                .mode-badge.job-update { background: var(--secondary-container); color: #4b4166; }
                .mode-badge.job-complete { background: var(--tertiary-container); color: #31503c; }
                .mode-badge.job-complete.failed { background: var(--error-container); color: #8a2e2e; }
                .mode-badge.system { background: color-mix(in srgb, var(--secondary-container) 88%, white); color: #5d3e84; }
                .message-body { white-space: pre-wrap; }
                .message-details {
                    margin-top: 12px;
                    border: 1px solid #d8dfeb;
                    border-radius: 18px;
                    background: color-mix(in srgb, var(--surface-container-low) 78%, transparent);
                    overflow: hidden;
                }
                .message-details summary {
                    cursor: pointer;
                    list-style: none;
                    padding: 10px 14px;
                    font-size: 0.78rem;
                    font-weight: 700;
                    letter-spacing: 0.04em;
                    text-transform: uppercase;
                    color: var(--muted);
                    background: color-mix(in srgb, var(--primary-container) 52%, transparent);
                }
                .message-details summary::-webkit-details-marker {
                    display: none;
                }
                .message-details[open] summary {
                    border-bottom: 1px solid var(--line);
                }
                .message-details pre {
                    padding: 14px;
                    background: transparent;
                    font-size: 0.84rem;
                    color: #3f4c65;
                }
                .thread-composer {
                    margin-top: 0;
                    padding: 10px 12px;
                    border: 1px solid var(--line);
                    border-radius: 20px;
                    background: color-mix(in srgb, var(--surface) 96%, transparent);
                    display: grid;
                    gap: 8px;
                    position: sticky;
                    bottom: 0;
                    box-shadow: var(--elevation-3);
                    z-index: 2;
                }
                .composer-input-wrap {
                    position: relative;
                }
                .composer-input-wrap textarea {
                    min-height: 78px;
                    padding-right: 64px;
                    padding-bottom: 18px;
                    border-radius: 22px;
                }
                .composer-send {
                    position: absolute;
                    right: 12px;
                    bottom: 12px;
                    width: 42px;
                    height: 42px;
                    min-width: 42px;
                    min-height: 42px;
                    padding: 0;
                    border-radius: 999px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 1rem;
                    line-height: 1;
                    background: var(--accent);
                    box-shadow: var(--elevation-2);
                }
                .composer-send .material-symbols-rounded {
                    font-size: 22px;
                }
                .result-message {
                    border: 1px solid #b8d3c7;
                    border-radius: 16px;
                    padding: 14px;
                    background: color-mix(in srgb, var(--tertiary-container) 44%, var(--surface));
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
                    background: color-mix(in srgb, var(--surface-container-lowest) 78%, transparent);
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
                    gap: 8px;
                    align-items: center;
                }
                .draft-rationale {
                    color: var(--muted);
                    font-size: 0.88rem;
                    margin: 0;
                }
                .empty {
                    padding: 24px 18px;
                    border: 1px dashed var(--line);
                    border-radius: 18px;
                    text-align: center;
                    color: var(--muted);
                    background: rgba(255,255,255,0.6);
                }
                @media (min-width: 921px) {
                    .thread-focus {
                        grid-template-columns: minmax(0, 1fr);
                        grid-template-areas:
                            "hero"
                            "chat";
                        grid-template-rows: auto minmax(0, 1fr);
                        align-items: start;
                    }
                    .thread-focus.details-open {
                        grid-template-columns: minmax(0, 1fr) minmax(280px, 320px);
                        grid-template-areas:
                            "hero hero"
                            "chat context";
                    }
                    .thread-hero {
                        grid-area: hero;
                    }
                    .thread-primary {
                        grid-area: chat;
                        min-height: 0;
                    }
                    .message-pane {
                        height: 100%;
                    }
                    .context-shell {
                        grid-area: context;
                        display: none;
                        position: sticky;
                        top: 82px;
                        max-height: calc(100vh - 136px);
                        overflow-y: auto;
                        padding-right: 2px;
                    }
                    .thread-focus.details-open .context-shell {
                        display: grid;
                    }
                }
                @media (max-width: 920px) {
                    .app-shell { padding: 16px; }
                    .workspace-shell { gap: 14px; }
                    .app-topbar {
                        padding: 12px 14px;
                        border-radius: 20px;
                        align-items: center;
                    }
                    .app-topbar-leading {
                        width: 100%;
                        gap: 10px;
                    }
                    .app-topbar-actions {
                        display: none;
                    }
                    .nav-rail {
                        display: none;
                    }
                    .topbar-subtitle {
                        display: none;
                    }
                    .nav-fab {
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                    }
                    .frame {
                        display: block;
                        height: calc(100vh - 112px);
                        min-height: calc(100vh - 112px);
                    }
                    .sidebar-backdrop {
                        display: block;
                        position: fixed;
                        inset: 0;
                        background: var(--scrim);
                        opacity: 0;
                        pointer-events: none;
                        transition: opacity 0.18s ease;
                        z-index: 20;
                    }
                    .sidebar-backdrop.open {
                        opacity: 1;
                        pointer-events: auto;
                    }
                    .sidebar-shell {
                        position: fixed;
                        left: 12px;
                        top: 12px;
                        bottom: 12px;
                        width: min(360px, calc(100vw - 40px));
                        z-index: 30;
                        transform: translateX(-115%);
                        opacity: 0;
                        pointer-events: none;
                        transition: transform 0.22s ease, opacity 0.22s ease;
                    }
                    .sidebar-shell.open {
                        transform: translateX(0);
                        opacity: 1;
                        pointer-events: auto;
                    }
                    .sidebar {
                        order: 2;
                        padding: 16px;
                        height: 100%;
                        overflow-y: auto;
                    }
                    .content {
                        order: 1;
                        padding: 14px;
                        height: 100%;
                    }
                    .content-toolbar {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        gap: 10px;
                        margin-bottom: 8px;
                    }
                    .sidebar-toggle,
                    .sidebar-close,
                    .context-toggle,
                    .context-close {
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        border-radius: 18px;
                        border: 1px solid var(--line);
                        background: color-mix(in srgb, var(--surface) 96%, transparent);
                        color: var(--on-primary-container);
                        padding: 9px 14px;
                        font-size: 0.84rem;
                        font-weight: 700;
                        box-shadow: var(--elevation-2);
                    }
                    .sidebar-header {
                        grid-template-columns: 1fr auto;
                        align-items: start;
                    }
                    .sidebar-status.compact {
                        display: none;
                    }
                    .thread-focus {
                        gap: 10px;
                    }
                    .thread-hero {
                        order: 1;
                        padding: 10px 12px;
                        border-radius: 18px;
                        gap: 6px;
                    }
                    .thread-hero-header {
                        align-items: center;
                        gap: 8px;
                    }
                    .thread-hero h2 {
                        font-size: 1.12rem;
                    }
                    .thread-hero .eyebrow,
                    .thread-hero .status,
                    .thread-summary-row {
                        display: none;
                    }
                    .thread-details-button {
                        display: none;
                    }
                    .thread-mobile-meta {
                        display: flex;
                        align-items: center;
                        gap: 8px;
                        flex-wrap: wrap;
                        color: var(--muted);
                        font-size: 0.76rem;
                    }
                    .thread-mobile-meta strong {
                        color: var(--ink);
                        font-size: 0.78rem;
                    }
                    .thread-primary {
                        order: 2;
                        min-height: 0;
                    }
                    .context-shell {
                        position: fixed;
                        left: 12px;
                        right: 12px;
                        bottom: 12px;
                        z-index: 80;
                        max-height: min(72vh, 620px);
                        padding: 14px;
                        border: 1px solid var(--line);
                        border-radius: 24px;
                        background: color-mix(in srgb, var(--surface) 98%, transparent);
                        box-shadow: var(--elevation-3);
                        overflow-y: auto;
                        transform: translateY(112%);
                        opacity: 0;
                        pointer-events: none;
                        transition: transform 0.22s ease, opacity 0.22s ease;
                    }
                    .context-shell.open {
                        transform: translateY(0);
                        opacity: 1;
                        pointer-events: auto;
                    }
                    .context-backdrop {
                        display: block;
                        position: fixed;
                        inset: 0;
                        background: var(--scrim);
                        opacity: 0;
                        pointer-events: none;
                        transition: opacity 0.18s ease;
                        z-index: 70;
                    }
                    .context-backdrop.open {
                        opacity: 1;
                        pointer-events: auto;
                    }
                    .context-shell-header {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        gap: 12px;
                        margin-bottom: 4px;
                    }
                    .context-shell-header h3 {
                        margin: 0;
                        font-size: 1rem;
                    }
                    .context-panel summary { padding: 10px 12px; }
                    .context-panel-body { padding: 10px 12px 12px 12px; }
                    .thread-composer {
                        margin-top: 8px;
                        padding: 10px 12px;
                        bottom: 0;
                        box-shadow: 0 10px 22px rgba(40, 34, 28, 0.08);
                    }
                    .composer-input-wrap textarea {
                        min-height: 72px;
                        padding-right: 58px;
                    }
                    .composer-send {
                        width: 40px;
                        height: 40px;
                        min-width: 40px;
                        min-height: 40px;
                        right: 10px;
                        bottom: 10px;
                    }
                    .message-pane {
                        height: 100%;
                        padding-bottom: 12px;
                    }
                }
                @media (max-width: 640px) {
                    .app-shell { padding: 12px; }
                    .app-topbar {
                        padding: 12px 14px;
                        border-radius: 20px;
                    }
                    .app-topbar-copy h2 {
                        font-size: 1rem;
                    }
                    .panel { border-radius: 16px; }
                    .content {
                        padding: 12px;
                        height: 100%;
                    }
                    .sidebar-header,
                    .thread-focus,
                    .thread-primary,
                    .context-shell { gap: 12px; }
                    .thread-hero { padding: 10px 12px; gap: 6px; }
                    .thread-pill {
                        width: 100%;
                        justify-content: flex-start;
                    }
                    .thread-card,
                    .job-card,
                    .message,
                    .job-event,
                    .job-detail,
                    .approval-card,
                    .report-grid article,
                    .note-card {
                        padding: 14px;
                        border-radius: 16px;
                    }
                    .thread-card {
                        padding: 10px 12px;
                    }
                    .message-header,
                    .thread-meta,
                    .job-meta,
                    .job-event header {
                        flex-direction: column;
                        align-items: flex-start;
                        gap: 6px;
                    }
                    .thread-composer textarea { min-height: 70px; }
                    .message-pane {
                        height: 100%;
                        padding-bottom: 10px;
                    }
                    .context-shell {
                        left: 10px;
                        right: 10px;
                        bottom: 10px;
                        padding: 12px;
                        border-radius: 20px;
                    }
                }
                "#}
            </style>
            {move || {
                match auth_session.get() {
                    None => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <div>
                                    <p class="eyebrow">"Elowen Workspace"</p>
                                    <h1>"Checking session"</h1>
                                    <p class="status">"Loading authentication state before the workspace UI appears."</p>
                                </div>
                            </section>
                        </div>
                    }.into_any(),
                    Some(session) if session.enabled && !session.authenticated => view! {
                        <div class="auth-shell">
                            <section class="auth-card">
                                <div>
                                    <p class="eyebrow">"Elowen Sign In"</p>
                                    <h1>"Protected workspace"</h1>
                                    <p class="status">"Sign in to browse threads, dispatch jobs, review notes, and approve pushes."</p>
                                </div>
                                <form on:submit=move |ev: ev::SubmitEvent| {
                                    ev.prevent_default();
                                    let password = auth_password.get_untracked().trim().to_string();
                                    if password.is_empty() {
                                        set_auth_error.set("Password is required.".to_string());
                                        return;
                                    }

                                    spawn_local({
                                        let set_auth_session = set_auth_session;
                                        let set_auth_error = set_auth_error;
                                        let set_auth_password = set_auth_password;
                                        let set_status_text = set_status_text;
                                        let set_threads = set_threads;
                                        let selected_thread_id = selected_thread_id;
                                        let set_selected_thread_id = set_selected_thread_id;
                                        let set_selected_thread = set_selected_thread;
                                        let set_jobs = set_jobs;

                                        async move {
                                            match login_session(&password).await {
                                                Ok(session) => {
                                                    set_auth_error.set(String::new());
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
                                    <input
                                        type="password"
                                        placeholder="Workspace password"
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
                                        <p class="status">"Authentication is enabled for this deployment."</p>
                                        <button type="submit">"Sign In"</button>
                                    </div>
                                </form>
                            </section>
                        </div>
                    }.into_any(),
                    Some(_) => view! {
            <div class="workspace-shell">
            <section class="panel app-topbar">
                <div class="app-topbar-leading">
                    <button
                        type="button"
                        class="nav-fab"
                        on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                    >
                        {move || if sidebar_open.get() { "Close" } else { "Threads" }}
                    </button>
                    <div class="app-topbar-copy">
                        <p class="eyebrow">"Elowen Assistant"</p>
                        <h2>"Chat-first orchestration"</h2>
                        <p class="topbar-subtitle">"Threads, chat, and explicit laptop handoff."</p>
                    </div>
                </div>
                <div class="app-topbar-actions">
                    {move || {
                        match auth_session.get().and_then(|session| session.operator_label) {
                            Some(operator_label) => view! {
                                <>
                                    <span class="topbar-chip operator">{operator_label}</span>
                                    <button
                                        type="button"
                                        class="logout-button"
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
                                                async move {
                                                    match logout_session().await {
                                                        Ok(session) => {
                                                            set_auth_session.set(Some(session));
                                                            set_status_text.set("Signed out.".to_string());
                                                            set_selected_thread.set(None);
                                                            set_selected_thread_id.set(None);
                                                            set_selected_job_detail.set(None);
                                                            set_selected_job_id.set(None);
                                                            set_threads.set(Vec::new());
                                                            set_jobs.set(Vec::new());
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
                                </>
                            }.into_any(),
                            None => view! { <span class="topbar-chip">"Protected workspace"</span> }.into_any(),
                        }
                    }}
                </div>
            </section>
            <div class="frame">
                <nav class="panel nav-rail" data-testid="nav-rail" aria-label="Primary navigation">
                    <div class="nav-rail-brand" aria-hidden="true">
                        <span class="material-symbols-rounded">"auto_awesome"</span>
                    </div>
                    <div class="nav-rail-items">
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Chats
                            data-testid="nav-chats"
                            on:click=move |_| {
                                set_nav_mode.set(NavMode::Chats);
                                set_context_open.set(false);
                                set_sidebar_open.set(true);
                            }
                        >
                            <span class="material-symbols-rounded" aria-hidden="true">"chat"</span>
                            <span>"Chats"</span>
                        </button>
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Details || context_open.get()
                            data-testid="nav-details"
                            on:click=move |_| {
                                set_nav_mode.set(NavMode::Details);
                                set_context_open.update(|open| *open = !*open);
                            }
                        >
                            <span class="material-symbols-rounded" aria-hidden="true">"info"</span>
                            <span>"Details"</span>
                        </button>
                        <button
                            type="button"
                            class="nav-rail-item"
                            class:active=move || nav_mode.get() == NavMode::Jobs
                            data-testid="nav-jobs"
                            on:click=move |_| {
                                set_nav_mode.set(NavMode::Jobs);
                                set_context_open.set(false);
                                set_sidebar_open.set(true);
                            }
                        >
                            <span class="material-symbols-rounded" aria-hidden="true">"work_history"</span>
                            <span>"Jobs"</span>
                        </button>
                    </div>
                    <div class="nav-rail-spacer"></div>
                </nav>
                <button
                    type="button"
                    class="sidebar-backdrop"
                    class:open=move || sidebar_open.get()
                    on:click=move |_| set_sidebar_open.set(false)
                ></button>
                <div class="sidebar-shell" class:open=move || sidebar_open.get()>
                <section class="panel sidebar">
                    <div class="sidebar-header">
                        <div>
                            <h1>{move || match nav_mode.get() {
                                NavMode::Jobs => "Jobs",
                                _ => "Chats",
                            }}</h1>
                        </div>
                        <div class="sidebar-status compact">
                            <p class="status">{move || status_text.get()}</p>
                        </div>
                        <button
                            type="button"
                            class="sidebar-close"
                            on:click=move |_| set_sidebar_open.set(false)
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="sidebar-view" class:hidden=move || nav_mode.get() != NavMode::Chats data-testid="thread-nav-panel">
                    <details class="context-panel">
                        <summary>"New Thread"</summary>
                        <div class="context-panel-body">
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
                                        if is_compact_layout() {
                                            set_sidebar_open.set(false);
                                        }
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
                                                if is_compact_layout() {
                                                    set_sidebar_open.set(false);
                                                }
                                            }
                                        >
                                            <div class="thread-card-header">
                                                <h3>{thread.title.clone()}</h3>
                                                <span class="thread-card-time">{updated_at.clone()}</span>
                                            </div>
                                            <div class="thread-meta">
                                                <span class="thread-status-dot"></span>
                                                <span>{thread.status.clone()}</span>
                                                <span>{format!("{} messages", thread.message_count)}</span>
                                            </div>
                                        </article>
                                    }
                                }
                            />
                        </div>
                    </div>
                    </div>
                    <div class="sidebar-view" class:hidden=move || nav_mode.get() != NavMode::Jobs data-testid="job-nav-panel">
                    <details class="context-panel" open>
                        <summary>{move || format!("Global Jobs ({})", jobs.get().len())}</summary>
                        <div class="context-panel-body">
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
                                                set_context_open.set(false);
                                                if is_compact_layout() {
                                                    set_sidebar_open.set(false);
                                                }
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
                    </details>
                    </div>
                </section>
                </div>
                <section class="panel content">
                    <button
                        type="button"
                        class="context-backdrop"
                        class:open=move || context_open.get()
                        on:click=move |_| set_context_open.set(false)
                    ></button>
                    <div class="content-toolbar">
                        <button
                            type="button"
                            class="sidebar-toggle"
                            on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                        >
                            {move || if sidebar_open.get() { "Hide Threads" } else { "Show Threads" }}
                        </button>
                        {move || {
                            if selected_thread.get().is_some() {
                                view! {
                                    <button
                                        type="button"
                                        class="context-toggle"
                                        on:click=move |_| set_context_open.update(|open| *open = !*open)
                                    >
                                        {move || if context_open.get() { "Hide Details" } else { "Details" }}
                                    </button>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                    </div>
                    {move || {
                        if let Some(thread) = selected_thread.get() {
                            let thread_id = thread.thread.id.clone();
                            let job_thread_id = thread_id.clone();
                            let message_actions_thread_id = thread_id.clone();
                            let chat_submit_thread_id = thread_id.clone();
                            let thread_record = thread.thread.clone();
                            let jobs = thread.jobs.clone();
                            let messages = thread.messages.clone();
                            let thread_notes = thread.related_notes.clone();
                            let has_jobs = !jobs.is_empty();
                            let active_job_id = selected_job_id.get();

                            view! {
                                <div class="thread-focus" class:details-open=move || context_open.get()>
                                    <section class="thread-hero">
                                        <div class="thread-hero-header">
                                            <div>
                                                <p class="eyebrow">"Conversation"</p>
                                                <h2>{thread_record.title.clone()}</h2>
                                                <div class="thread-mobile-meta">
                                                    <strong>{format!("{} messages", messages.len())}</strong>
                                                    <span>{format!("{} jobs", jobs.len())}</span>
                                                    <span>{format!("Updated {}", thread_record.updated_at.clone())}</span>
                                                </div>
                                            </div>
                                            <span class=format!(
                                                "status-badge {}",
                                                status_badge_class(&thread_record.status)
                                            )>
                                                {thread_record.status.clone()}
                                            </span>
                                            <button
                                                type="button"
                                                class="thread-details-button"
                                                on:click=move |_| set_context_open.update(|open| *open = !*open)
                                            >
                                                {move || if context_open.get() { "Hide Details" } else { "Details" }}
                                            </button>
                                        </div>
                                        <div class="thread-summary-row">
                                            <span class="thread-pill">{format!("{} messages", messages.len())}</span>
                                            <span class="thread-pill">{format!("{} jobs", jobs.len())}</span>
                                            <span class="thread-pill">{format!("{} notes", thread_notes.len())}</span>
                                            <span class="thread-pill">{format!("Updated {}", thread_record.updated_at)}</span>
                                        </div>
                                    </section>

                                    <div class="context-shell" class:open=move || context_open.get() data-testid="context-sheet">
                                    <div class="context-shell-header">
                                        <h3>"Conversation Details"</h3>
                                        <button
                                            type="button"
                                            class="context-close"
                                            on:click=move |_| set_context_open.set(false)
                                        >
                                            "Close"
                                        </button>
                                    </div>
                                    <details class="context-panel">
                                        <summary>"Thread Context"</summary>
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
                                        </div>
                                    </details>

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
                                                <details class="context-panel">
                                                    <summary>"Selected Job"</summary>
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
                                                    </div>
                                                </details>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <details class="context-panel">
                                                    <summary>"Selected Job"</summary>
                                                    <div class="context-panel-body">
                                                        <div class="empty">
                                                            <p class="eyebrow">"No Job Selected"</p>
                                                            <p>"Choose a job to inspect the live execution detail and event history."</p>
                                                        </div>
                                                    </div>
                                                </details>
                                            }.into_any()
                                        }
                                    }}

                                    <details class="context-panel">
                                        <summary>"Advanced Manual Job"</summary>
                                        <div class="context-panel-body">
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
                                                    None,
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
                                        </div>
                                    </details>

                                    </div>

                                    <div class="thread-primary">
                                    <div class="message-pane" data-testid="message-pane" node_ref=message_pane_ref>
                                    <div class="message-list">
                                        <For
                                            each=move || messages.clone()
                                            key=|message| message.id.clone()
                                            children=move |message| {
                                                let message_id = message.id.clone();
                                                let message_role = message.role.clone();
                                                let execution_draft = message_execution_draft(&message);
                                                let result_details = message_result_details(&message);
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
                                                let draft_execution_intent = execution_draft
                                                    .as_ref()
                                                    .map(|draft| draft.execution_intent.clone())
                                                    .unwrap_or(ExecutionIntent::WorkspaceChange);
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
                                                        {result_details.map(|details| {
                                                            view! {
                                                                <details class="message-details">
                                                                    <summary>"More Details"</summary>
                                                                    <pre>{details}</pre>
                                                                </details>
                                                            }
                                                        })}
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
                                                                        <div class="thread-meta">
                                                                            <span>"Execution Mode"</span>
                                                                            <span>{execution_intent_label(&draft_execution_intent)}</span>
                                                                        </div>
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
                                                                                        let execution_intent = draft_execution_intent.clone();

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
                                                                                                    Some(execution_intent),
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
                                                                                        match dispatch_thread_message(&thread_id, &source_message_id, &title, &repo_name, &base_branch, None, None).await {
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
                                    </div>
                                    <form class="thread-composer" data-testid="thread-composer" on:submit=move |ev: ev::SubmitEvent| {
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
                                        <div class="composer-input-wrap">
                                            <textarea
                                                placeholder="Message Elowen"
                                                prop:value=move || new_message_content.get()
                                                on:input=move |ev| set_new_message_content.set(event_target_value(&ev))
                                            />
                                            <button type="submit" class="composer-send" aria-label="Send message">
                                                <span class="material-symbols-rounded" aria-hidden="true">"arrow_circle_up"</span>
                                            </button>
                                        </div>
                                    </form>
                                    </div>
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
            </div>
                    }.into_any(),
                }
            }}
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

fn is_compact_layout() -> bool {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .map(|width| width <= 920.0)
        .unwrap_or(false)
}

fn is_auth_error(error: &str) -> bool {
    error.contains("sign in required") || error.contains("status 401")
}
