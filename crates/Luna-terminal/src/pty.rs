use portable_pty::{CommandBuilder, PtySize};
use std::io::Write;
use tokio::sync::mpsc;

use crate::shell::ShellConfig;

pub struct PtyHandle {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub writer: Option<Box<dyn std::io::Write + Send>>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub reader: Option<Box<dyn std::io::Read + Send>>,
}

pub struct PtySession {
    pub pty: PtyHandle,
    pub rx: mpsc::UnboundedReceiver<Option<Vec<u8>>>,
}

impl PtyHandle {
    pub fn spawn(cols: u16, rows: u16, shell: &ShellConfig) -> Result<PtyHandle, String> {
        let pty_system = portable_pty::native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(size).map_err(|e| {
            format!(
                "PTY open error: {}. Luna requires a POSIX-compatible PTY.",
                e
            )
        })?;

        let mut cmd = CommandBuilder::new(&shell.program);
        if !shell.args.is_empty() {
            cmd.args(&shell.args);
        }
        for (key, value) in &shell.env {
            cmd.env(key, value);
        }
        if let Some(ref cwd) = shell.cwd {
            cmd.cwd(std::path::Path::new(cwd));
        }

        let child = pair.slave.spawn_command(cmd).map_err(|e| {
            format!(
                "PTY spawn error: {} (shell: {}). Verify the shell exists and is executable.",
                e, shell.program
            )
        })?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("PTY reader clone error: {}", e))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("PTY writer error: {}", e))?;

        Ok(PtyHandle {
            master: pair.master,
            writer: Some(writer),
            child,
            reader: Some(reader),
        })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        if let Some(ref mut w) = self.writer {
            w.write_all(data)
                .map_err(|e| format!("PTY write error: {}", e))
        } else {
            Err("PTY writer not available".into())
        }
    }

    pub fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .map_err(|e| format!("PTY kill error: {}", e))
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        self.master
            .resize(size)
            .map_err(|e| format!("PTY resize error: {}", e))
    }

    pub fn start_reader(pty: PtyHandle) -> PtySession {
        let PtyHandle {
            master,
            writer,
            child,
            reader,
        } = pty;

        if let Some(mut reader) = reader {
            let (tx, rx) = mpsc::unbounded_channel::<Option<Vec<u8>>>();

            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => {
                            let _ = tx.send(None);
                            break;
                        }
                        Ok(n) => {
                            let data = buf[..n].to_vec();
                            if tx.send(Some(data)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                                continue;
                            }
                            let _ = tx.send(None);
                            break;
                        }
                    }
                }
            });

            PtySession {
                pty: PtyHandle {
                    master,
                    writer,
                    child,
                    reader: None,
                },
                rx,
            }
        } else {
            let (_tx, rx) = mpsc::unbounded_channel::<Option<Vec<u8>>>();
            PtySession {
                pty: PtyHandle {
                    master,
                    writer,
                    child,
                    reader: None,
                },
                rx,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_spawn() {
        let shell = crate::shell::detect_shell();
        let handle = PtyHandle::spawn(80, 24, &shell);
        assert!(handle.is_ok());
    }

    #[test]
    fn test_pty_write_and_read() {
        let shell = crate::shell::detect_shell();
        let mut handle = PtyHandle::spawn(80, 24, &shell).unwrap();

        handle.write(b"echo HELLO_LUNA\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        if let Some(ref mut reader) = handle.reader {
            let mut buf = [0u8; 4096];
            if let Ok(n) = std::io::Read::read(reader, &mut buf) {
                let output = String::from_utf8_lossy(&buf[..n]);
                assert!(output.contains("HELLO_LUNA"));
            }
        }
    }
}
