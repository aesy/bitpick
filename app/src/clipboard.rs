use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;
use which::which;

// On X11/Wayland the clipboard content is served by the process that set it and
// vanishes when it exits. Shell tools fork a daemon for that; arboard needs a
// process of ours to stay alive.
const OWNER_SERVED_CLIPBOARD: bool = cfg!(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
));

const WORKER_FLAG: &str = "--__clipboard-worker";

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error(
        "No clipboard backend is available. Use `--print` or install a desktop clipboard utility."
    )]
    Unavailable,

    #[error("Failed to write to the system clipboard")]
    ArboardWrite(#[source] arboard::Error),

    #[error("Failed to read the system clipboard")]
    ArboardRead(#[source] arboard::Error),

    #[error("Clipboard I/O error")]
    Io(#[from] io::Error),

    #[error("Failed to run `{command}`")]
    SpawnCommand {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("{command} failed ({status})")]
    CommandFailedStatus { command: String, status: String },
}

pub type Result<T> = std::result::Result<T, ClipboardError>;

pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
    external: Option<ExternalClipboard>,
}

enum Backend<'a> {
    External(&'a ExternalClipboard),
    Arboard(&'a mut arboard::Clipboard),
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

    /// Whether a copy must go through [`spawn_worker`] so the content outlives
    /// this process.
    pub const fn needs_worker(&self) -> bool {
        OWNER_SERVED_CLIPBOARD && self.external.is_none()
    }

    pub fn copy(&mut self, value: &str) -> Result<()> {
        self.try_backends(|backend| match backend {
            Backend::External(external) => external.copy(value),
            Backend::Arboard(inner) => inner.set_text(value).map_err(ClipboardError::ArboardWrite),
        })
    }

    pub fn read(&mut self) -> Result<String> {
        self.try_backends(|backend| match backend {
            Backend::External(external) => external.read(),
            Backend::Arboard(inner) => inner.get_text().map_err(ClipboardError::ArboardRead),
        })
    }

    pub fn clear(&mut self) -> Result<()> {
        self.copy("")
    }

    fn try_backends<T>(&mut self, mut op: impl FnMut(Backend<'_>) -> Result<T>) -> Result<T> {
        let mut backends = [
            self.external.as_ref().map(Backend::External),
            self.inner.as_mut().map(Backend::Arboard),
        ];
        if !OWNER_SERVED_CLIPBOARD {
            backends.reverse();
        }

        let mut first_error = None;
        for backend in backends.into_iter().flatten() {
            match op(backend) {
                Ok(value) => return Ok(value),
                Err(error) => {
                    first_error.get_or_insert(error);
                },
            }
        }

        Err(first_error.unwrap_or(ClipboardError::Unavailable))
    }

    /// Sets the clipboard and keeps serving it until `deadline`, or until
    /// another program replaces it.
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    fn hold(&mut self, value: &str, deadline: Option<Instant>) -> Result<()> {
        use arboard::SetExtLinux;

        let inner = self.inner.as_mut().ok_or(ClipboardError::Unavailable)?;
        let set = inner.set();
        let set = match deadline {
            Some(deadline) => set.wait_until(deadline),
            None => set.wait(),
        };
        set.text(value).map_err(ClipboardError::ArboardWrite)
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    fn hold(&mut self, value: &str, _deadline: Option<Instant>) -> Result<()> {
        self.copy(value)
    }
}

/// Starts a detached copy of this binary that clears the clipboard after
/// `clear_after` and, when [`Clipboard::needs_worker`], also sets and serves
/// the clipboard itself.
pub fn spawn_worker(value: &str, clear_after: Option<Duration>) -> Result<()> {
    let program = std::env::current_exe()?;
    let secs = clear_after.map_or(0, |delay| delay.as_secs()).to_string();

    let mut command = Command::new(&program);
    command
        .arg(WORKER_FLAG)
        .arg(&secs)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);

    let mut child = command
        .spawn()
        .map_err(|source| ClipboardError::SpawnCommand {
            command: format!("{} {WORKER_FLAG} {secs}", program.display()),
            source,
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(value.as_bytes())?;
    }

    Ok(())
}

pub fn run_worker(clear_after: Option<Duration>) -> Result<()> {
    let mut value = String::new();
    io::stdin().read_to_string(&mut value)?;
    let copied = fingerprint(&value);
    let deadline = clear_after.map(|delay| Instant::now() + delay);

    let mut clipboard = Clipboard::new()?;
    if clipboard.needs_worker() {
        clipboard.hold(&value, deadline)?;
    }
    drop(value);

    let Some(deadline) = deadline else {
        return Ok(());
    };
    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));

    if clipboard
        .read()
        .is_ok_and(|current| fingerprint(&current) == copied)
    {
        clipboard.clear()?;
    }

    Ok(())
}

fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Outside the shell's job process groups, so closing the terminal does
        // not SIGHUP it.
        command.process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        // DETACHED_PROCESS: no console of its own, so closing ours does not end
        // it.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(DETACHED_PROCESS);
    }
}

fn fingerprint(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
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

        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        {
            if std::env::var_os("WAYLAND_DISPLAY").is_some()
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

fn render_command(program: &Path, args: &[&str]) -> String {
    if args.is_empty() {
        program.display().to_string()
    } else {
        format!("{} {}", program.display(), args.join(" "))
    }
}
