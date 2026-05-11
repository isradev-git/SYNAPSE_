use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
}

pub fn detect_shell() -> ShellConfig {
    #[cfg(target_os = "windows")]
    {
        let comspec = std::env::var("COMSPEC");
        let mut candidates: Vec<String> = Vec::new();

        if let Ok(ref cs) = comspec {
            if !cs.is_empty() {
                candidates.push(cs.clone());
            }
        }

        // Try to find PowerShell next
        if let Some(ps_path) = find_program_in_path("powershell.exe") {
            candidates.push(ps_path);
        }
        if let Some(pwsh_path) = find_program_in_path("pwsh.exe") {
            candidates.push(pwsh_path);
        }

        // Hardcoded fallbacks for cmd.exe
        for p in &[
            "C:\\Windows\\System32\\cmd.exe",
            "C:\\Windows\\Sysnative\\cmd.exe",
        ] {
            let path = std::path::Path::new(p);
            if path.exists() {
                candidates.push(p.to_string());
            }
        }

        if candidates.is_empty() {
            candidates.push(String::from("cmd.exe"));
        }

        let program = candidates.into_iter().next().unwrap();
        ShellConfig {
            program,
            args: vec![],
            env: HashMap::new(),
            cwd: None,
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
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
}

#[cfg(target_os = "windows")]
fn find_program_in_path(name: &str) -> Option<String> {
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(name);
            if full.exists() {
                return Some(full.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn fallback_shell() -> String {
    String::from("/bin/zsh")
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
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
