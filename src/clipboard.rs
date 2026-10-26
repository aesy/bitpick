use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use thiserror::Error;
use which::which;

#[cfg(target_os = "linux")]
use std::env;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error(
        "No clipboard backend is available. Use `--print` or install a desktop clipboard utility."
    )]
    Unavailable,

    #[error("Failed to copy the selected value to the system clipboard")]
    Arboard(#[source] arboard::Error),

    #[error("Clipboard I/O error")]
    Io(#[from] io::Error),

    #[error("Failed to run `{command}`")]
    SpawnCommand {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("{command} failed with status {status}")]
    CommandFailedStatus { command: String, status: String },
}

pub type Result<T> = std::result::Result<T, ClipboardError>;

pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
    external: Option<ExternalClipboard>,
}

impl Clipboard {
    pub fn new() -> Result<Self> {
        let inner = arboard::Clipboard::new().ok();
        let external = ExternalClipboard::detect();

        if inner.is_none() && external.is_none() {
            return Err(ClipboardError::Unavailable);
        }

        Ok(Self { inner, external })
    }

    pub fn copy(&mut self, value: &str) -> Result<()> {
        let mut first_error = None;

        #[cfg(target_os = "linux")]
        if let Some(external) = self.external.as_ref() {
            match external.copy(value) {
                Ok(()) => return Ok(()),
                Err(error) => first_error = Some(error),
            }
        }

        if let Some(inner) = self.inner.as_mut() {
            match inner.set_text(value.to_owned()) {
                Ok(()) => return Ok(()),
                Err(error) => first_error = Some(ClipboardError::Arboard(error)),
            }
        }

        #[cfg(not(target_os = "linux"))]
        if let Some(external) = self.external.as_ref() {
            return external
                .copy(value)
                .map_err(|error| first_error.unwrap_or(error));
        }

        Err(first_error.unwrap_or(ClipboardError::Unavailable))
    }

    pub fn read(&mut self) -> Result<String> {
        let mut first_error = None;

        #[cfg(target_os = "linux")]
        if let Some(external) = self.external.as_ref() {
            match external.read() {
                Ok(value) => return Ok(value),
                Err(error) => first_error = Some(error),
            }
        }

        if let Some(inner) = self.inner.as_mut() {
            match inner.get_text() {
                Ok(value) => return Ok(value),
                Err(error) => first_error = Some(ClipboardError::Arboard(error)),
            }
        }

        #[cfg(not(target_os = "linux"))]
        if let Some(external) = self.external.as_ref() {
            return external
                .read()
                .map_err(|error| first_error.unwrap_or(error));
        }

        Err(first_error.unwrap_or(ClipboardError::Unavailable))
    }

    pub fn clear(&mut self) -> Result<()> {
        self.copy("")
    }
}

pub fn spawn_clear_timer(value: &str, delay: Duration) -> Result<()> {
    let program = std::env::current_exe().map_err(ClipboardError::Io)?;
    let secs = delay.as_secs().to_string();

    let mut child = Command::new(&program)
        .arg("--__clear-clipboard-after")
        .arg(&secs)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| ClipboardError::SpawnCommand {
            command: format!("{} --__clear-clipboard-after {}", program.display(), secs),
            source,
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(value.as_bytes())?;
    }
    drop(child);
    Ok(())
}

pub fn run_clear_timer(delay: Duration) -> Result<()> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    std::thread::sleep(delay);

    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(_) => return Ok(()),
    };
    match clipboard.read() {
        Ok(current) if current == buf => {
            let _ = clipboard.clear();
        }
        _ => {}
    }
    Ok(())
}

struct ExternalClipboard {
    program: PathBuf,
    copy_args: Vec<&'static str>,
    read_program: Option<PathBuf>,
    read_args: Vec<&'static str>,
}

impl ExternalClipboard {
    fn detect() -> Option<Self> {
        #[cfg(target_os = "macos")]
        if let Ok(program) = which("pbcopy") {
            let read_program = which("pbpaste").ok();
            return Some(Self {
                program,
                copy_args: Vec::new(),
                read_program,
                read_args: Vec::new(),
            });
        }

        #[cfg(target_os = "windows")]
        if let Ok(program) = which("clip.exe") {
            let read_program = which("powershell.exe").ok();
            return Some(Self {
                program,
                copy_args: Vec::new(),
                read_program,
                read_args: vec!["-NoProfile", "-Command", "Get-Clipboard"],
            });
        }

        #[cfg(target_os = "linux")]
        {
            if env::var_os("WAYLAND_DISPLAY").is_some()
                && let Ok(program) = which("wl-copy")
            {
                let read_program = which("wl-paste").ok();
                return Some(Self {
                    program,
                    copy_args: vec!["--trim-newline"],
                    read_program,
                    read_args: vec!["--no-newline"],
                });
            }

            if let Ok(program) = which("xclip") {
                let read_program = Some(program.clone());
                return Some(Self {
                    program,
                    copy_args: vec!["-selection", "clipboard"],
                    read_program,
                    read_args: vec!["-selection", "clipboard", "-o"],
                });
            }

            if let Ok(program) = which("xsel") {
                let read_program = Some(program.clone());
                return Some(Self {
                    program,
                    copy_args: vec!["--clipboard", "--input"],
                    read_program,
                    read_args: vec!["--clipboard", "--output"],
                });
            }
        }

        None
    }

    fn copy(&self, value: &str) -> Result<()> {
        let command = render_command(&self.program, &self.copy_args);
        let mut child = Command::new(&self.program)
            .args(&self.copy_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| ClipboardError::SpawnCommand {
                command: command.clone(),
                source,
            })?;

        {
            let mut stdin = child.stdin.take().expect("stdin was piped");
            stdin.write_all(value.as_bytes())?;
        }

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(ClipboardError::CommandFailedStatus {
                command,
                status: status.to_string(),
            })
        }
    }

    fn read(&self) -> Result<String> {
        let program = self
            .read_program
            .as_ref()
            .ok_or(ClipboardError::Unavailable)?;
        let command = render_command(program, &self.read_args);
        let output = Command::new(program)
            .args(&self.read_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|source| ClipboardError::SpawnCommand {
                command: command.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(ClipboardError::CommandFailedStatus {
                command,
                status: output.status.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn render_command(program: &std::path::Path, args: &[&str]) -> String {
    if args.is_empty() {
        program.display().to_string()
    } else {
        format!("{} {}", program.display(), args.join(" "))
    }
}
