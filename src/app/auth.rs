use crate::models::AuthMode;

pub(super) fn auth_loading_message() -> &'static str {
    "Checking auth session..."
}

pub(super) fn protected_workspace_label() -> &'static str {
    "Protected Workspace"
}

pub(super) fn auth_prompt(mode: &AuthMode) -> &'static str {
    match mode {
        AuthMode::LocalAccounts => {
            "Sign in with your Elowen account to access threads, jobs, and notes."
        }
        AuthMode::LegacySharedPassword | AuthMode::Disabled => {
            "Enter the shared workspace password to access threads, jobs, and notes."
        }
    }
}

pub(super) fn username_placeholder(mode: &AuthMode) -> &'static str {
    match mode {
        AuthMode::LocalAccounts => "Username",
        AuthMode::LegacySharedPassword | AuthMode::Disabled => "Shared access",
    }
}

pub(super) fn password_placeholder(mode: &AuthMode) -> &'static str {
    match mode {
        AuthMode::LocalAccounts => "Account password",
        AuthMode::LegacySharedPassword | AuthMode::Disabled => "Workspace password",
    }
}
