use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
}

pub fn detect_shell() -> ShellConfig {
    let shell = std::env::var("SHELL")
        .map(|s| {
            if std::path::Path::new(&s).exists() {
                s
            } else {
                fallback_shell()
            }
        })
        .unwrap_or_else(|_| fallback_shell());

    ShellConfig {
        program: shell,
        args: vec![],
        env: HashMap::new(),
        cwd: None,
    }
}

#[cfg(target_os = "macos")]
fn fallback_shell() -> String {
    String::from("/bin/zsh")
}

#[cfg(not(target_os = "macos"))]
fn fallback_shell() -> String {
    String::from("/bin/bash")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_returns_program() {
        let config = detect_shell();
        assert!(!config.program.is_empty());
    }
}
