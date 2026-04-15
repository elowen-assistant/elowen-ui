pub(super) const APP_STYLE: &str = r#"
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
                html, body {
                    height: 100%;
                    overflow: hidden;
                }
                body {
                    margin: 0;
                    background:
                        radial-gradient(circle at top left, rgba(92, 95, 190, 0.16), transparent 24%),
                        radial-gradient(circle at top right, rgba(93, 95, 114, 0.08), transparent 28%),
                        linear-gradient(180deg, var(--bg) 0%, var(--bg-alt) 100%);
                    color: var(--ink);
                    font-family: "Segoe UI Variable Text", "Segoe UI", "Roboto", "Noto Sans", system-ui, sans-serif;
                }
                .app-shell {
                    height: 100dvh;
                    padding: 24px;
                    overflow: hidden;
                }
                .workspace-shell {
                    max-width: 1320px;
                    margin: 0 auto;
                    display: grid;
                    grid-template-rows: auto minmax(0, 1fr);
                    gap: 12px;
                    height: calc(100dvh - 48px);
                    min-height: 0;
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
                .topbar-chip.realtime {
                    color: var(--muted);
                    background: var(--surface-container-high);
                }
                .topbar-chip.realtime.connected {
                    color: #24523a;
                    background: color-mix(in srgb, var(--tertiary-container) 78%, var(--surface));
                }
                .topbar-chip.realtime.degraded,
                .topbar-chip.realtime.disconnected {
                    color: #7a3b12;
                    background: #ffdcc2;
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
                    align-items: stretch;
                    position: relative;
                    height: 100%;
                    min-height: 0;
                    max-height: 100%;
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
                    min-width: 0;
                    height: 100%;
                    min-height: 0;
                    max-height: 100%;
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
                .job-card.intent-card {
                    position: relative;
                    padding-right: 56px;
                }
                .job-card.intent-card::after {
                    content: "arrow_forward";
                    font-family: "Material Symbols Rounded";
                    position: absolute;
                    right: 18px;
                    top: 50%;
                    transform: translateY(-50%);
                    width: 32px;
                    height: 32px;
                    border-radius: 999px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    background: color-mix(in srgb, var(--primary-container) 74%, transparent);
                    color: var(--on-primary-container);
                    font-size: 20px;
                    font-variation-settings: "FILL" 0, "wght" 500, "GRAD" 0, "opsz" 24;
                }
                .job-browser {
                    height: 100%;
                    min-height: 0;
                    display: grid;
                    grid-template-rows: auto minmax(0, 1fr);
                    gap: 14px;
                }
                .job-browser-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 14px;
                    flex-wrap: wrap;
                    padding: 12px 14px;
                    border: 1px solid var(--line);
                    border-radius: 22px;
                    background: color-mix(in srgb, var(--surface-container-low) 88%, transparent);
                    box-shadow: var(--elevation-1);
                }
                .job-browser-header h2 {
                    margin: 0;
                    font-size: 1.35rem;
                    line-height: 1.15;
                }
                .job-browser-header .status {
                    margin: 4px 0 0 0;
                    font-size: 0.82rem;
                }
                .job-browser-grid {
                    min-height: 0;
                    overflow-y: auto;
                    display: grid;
                    align-content: start;
                    gap: 12px;
                    padding-right: 4px;
                }
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
                .message.pending {
                    border-style: dashed;
                    border-color: color-mix(in srgb, var(--primary) 26%, var(--line));
                    opacity: 0.92;
                }
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
                    padding: 2px 4px 24px 0;
                    scroll-behavior: smooth;
                }
                .message-list {
                    padding-bottom: 18px;
                }
                .chat-response-indicator {
                    position: sticky;
                    bottom: 0;
                    display: inline-flex;
                    align-items: center;
                    gap: 8px;
                    margin-left: 4px;
                    padding: 6px 10px;
                    border-radius: 999px;
                    background: color-mix(in srgb, var(--surface-container-lowest) 88%, transparent);
                    color: var(--muted);
                    font-size: 0.76rem;
                    font-weight: 600;
                    border: 1px solid color-mix(in srgb, var(--line) 68%, transparent);
                    box-shadow: var(--elevation-1);
                    width: fit-content;
                    max-width: 100%;
                    backdrop-filter: blur(12px);
                }
                .chat-response-indicator-dots {
                    display: inline-flex;
                    align-items: center;
                    gap: 4px;
                }
                .chat-response-indicator-dot {
                    width: 6px;
                    height: 6px;
                    border-radius: 999px;
                    background: color-mix(in srgb, var(--primary) 58%, white);
                    opacity: 0.3;
                    animation: chat-response-pulse 1.2s infinite ease-in-out;
                }
                .chat-response-indicator-dot:nth-child(2) { animation-delay: 0.15s; }
                .chat-response-indicator-dot:nth-child(3) { animation-delay: 0.3s; }
                @keyframes chat-response-pulse {
                    0%, 80%, 100% {
                        transform: scale(0.72);
                        opacity: 0.24;
                    }
                    40% {
                        transform: scale(1);
                        opacity: 0.68;
                    }
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
                        align-items: stretch;
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
                        height: 100%;
                        min-height: 0;
                        max-height: 100%;
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
                        padding-bottom: 24px;
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
                        padding-bottom: 22px;
                    }
                    .context-shell {
                        left: 10px;
                        right: 10px;
                        bottom: 10px;
                        padding: 12px;
                        border-radius: 20px;
                    }
                }
                
"#;
