use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── socket path ────────────────────────────────────────────────────────────────

pub fn socket_path() -> PathBuf {
    let uid = rustix::process::getuid().as_raw();
    std::env::temp_dir().join(format!("synapse_{uid}.sock"))
}

// ── protocol types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum IpcCommandKind {
    List,
    Send {
        text: String,
    },
    NewTab {
        command: Option<String>,
        cwd: Option<String>,
    },
    NewWindow {
        command: Option<String>,
        cwd: Option<String>,
    },
    Kill {
        pane_id: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: u32,
    pub title: String,
    pub cwd: String,
    pub active: bool,
    pub tab_index: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcResponse {
    Panes { ok: bool, panes: Vec<PaneInfo> },
    Text { ok: bool, output: String },
    Err { ok: bool, error: String },
    Ok { ok: bool },
}

impl IpcResponse {
    pub fn ok() -> Self {
        Self::Ok { ok: true }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self::Err {
            ok: false,
            error: msg.into(),
        }
    }
}

// ── request wrapper (command + one-shot response channel) ─────────────────────

pub struct IpcRequest {
    pub command: IpcCommandKind,
    pub response_tx: mpsc::SyncSender<IpcResponse>,
}

// ── server ────────────────────────────────────────────────────────────────────

pub fn start_ipc_server(tx: mpsc::Sender<IpcRequest>) -> std::io::Result<()> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;

    std::thread::Builder::new()
        .name("ipc-accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let tx2 = tx.clone();
                        std::thread::Builder::new()
                            .name("ipc-conn".into())
                            .spawn(move || handle_connection(stream, tx2))
                            .ok();
                    }
                    Err(e) => tracing::warn!("IPC accept error: {e}"),
                }
            }
        })?;

    tracing::info!("IPC server listening on {}", path.display());
    Ok(())
}

fn handle_connection(stream: UnixStream, tx: mpsc::Sender<IpcRequest>) {
    let write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("IPC clone stream: {e}");
            return;
        }
    };
    let mut writer = std::io::BufWriter::new(write_stream);
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => break,
        };

        let command: IpcCommandKind = match serde_json::from_str(&line) {
            Ok(cmd) => cmd,
            Err(e) => {
                send_response(&mut writer, &IpcResponse::err(format!("parse error: {e}")));
                continue;
            }
        };

        let (resp_tx, resp_rx) = mpsc::sync_channel(1);
        if tx
            .send(IpcRequest {
                command,
                response_tx: resp_tx,
            })
            .is_err()
        {
            break;
        }

        let response = resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| IpcResponse::err("timeout"));
        send_response(&mut writer, &response);
    }
}

fn send_response(writer: &mut impl Write, resp: &IpcResponse) {
    match serde_json::to_string(resp) {
        Ok(json) => {
            let _ = writeln!(writer, "{json}");
            let _ = writer.flush();
        }
        Err(e) => tracing::warn!("IPC serialize response: {e}"),
    }
}

// ── client ────────────────────────────────────────────────────────────────────

pub fn client_send(cmd: &IpcCommandKind) -> Result<IpcResponse, String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| {
        format!(
            "No SYNAPSE_ instance found at {}.\nIs a SYNAPSE_ window open?\nError: {e}",
            path.display()
        )
    })?;

    let json = serde_json::to_string(cmd).map_err(|e| e.to_string())?;
    writeln!(stream, "{json}").map_err(|e| e.to_string())?;

    let mut response = String::new();
    BufReader::new(&stream)
        .read_line(&mut response)
        .map_err(|e| e.to_string())?;

    serde_json::from_str(response.trim()).map_err(|e| format!("parse response: {e}"))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_in_tmpdir() {
        let p = socket_path();
        assert!(p.starts_with(std::env::temp_dir()));
        assert!(p
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("synapse_"));
    }

    #[test]
    fn test_ipc_command_deserialize_list() {
        let cmd: IpcCommandKind = serde_json::from_str(r#"{"cmd":"list"}"#).unwrap();
        assert!(matches!(cmd, IpcCommandKind::List));
    }

    #[test]
    fn test_ipc_command_deserialize_send() {
        let cmd: IpcCommandKind =
            serde_json::from_str(r#"{"cmd":"send","text":"hello\n"}"#).unwrap();
        assert!(matches!(cmd, IpcCommandKind::Send { text } if text == "hello\n"));
    }

    #[test]
    fn test_ipc_command_deserialize_new_tab() {
        let cmd: IpcCommandKind =
            serde_json::from_str(r#"{"cmd":"new-tab","command":"vim","cwd":"/tmp"}"#).unwrap();
        assert!(matches!(cmd, IpcCommandKind::NewTab { command, cwd }
                if command.as_deref() == Some("vim") && cwd.as_deref() == Some("/tmp")));
    }

    #[test]
    fn test_ipc_command_deserialize_kill_no_id() {
        let cmd: IpcCommandKind = serde_json::from_str(r#"{"cmd":"kill","pane_id":null}"#).unwrap();
        assert!(matches!(cmd, IpcCommandKind::Kill { pane_id: None }));
    }

    #[test]
    fn test_ipc_response_serialize_ok() {
        let resp = IpcResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
    }

    #[test]
    fn test_ipc_response_serialize_err() {
        let resp = IpcResponse::err("something failed");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("something failed"));
    }
}
