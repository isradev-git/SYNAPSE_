//! Default shell resolution.
//!
//! The user can always override the interactive shell via `shell_program` in the
//! config; these helpers provide the fallback when it is empty, and the way to
//! run a one-shot command string.

/// The default interactive shell when none is configured: `$SHELL`, else `/bin/bash`.
pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
}

/// Shell program and flag used to execute a single command string (`sh -c "<cmd>"`).
pub fn command_runner() -> (&'static str, &'static str) {
    ("/bin/sh", "-c")
}
