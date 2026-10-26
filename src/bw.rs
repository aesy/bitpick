use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Deserialize;
use thiserror::Error;
use which::which;

#[derive(Debug, Error)]
pub enum BwError {
    #[error("Could not find `bw` in PATH")]
    MissingProgram,

    #[error(
        "Bitwarden CLI is not logged in. Run `bw login` once, then launch `bitwarden-tui` again."
    )]
    NotLoggedIn,

    #[error("Unsupported Bitwarden status: {0}")]
    UnknownStatus(String),

    #[error("Bitwarden did not return a session key after unlock")]
    EmptyUnlockSession,

    #[error("Could not determine a directory for the Bitwarden session cache")]
    MissingCacheDir,

    #[error("Failed to create directory `{path}`")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to read `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write `{path}`")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to update permissions for `{path}`")]
    SetPermissions {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to run `{command}`")]
    SpawnCommand {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{command} failed: {stderr}")]
    CommandFailed { command: String, stderr: String },

    #[error("{command} failed with status {status}")]
    CommandFailedStatus { command: String, status: String },

    #[error("{command} returned non-UTF8 output")]
    CommandUtf8 {
        command: String,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("Invalid JSON from `{command}`")]
    JsonCommand {
        command: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Bitwarden session is not initialized")]
    SessionNotInitialized,
}

pub type Result<T> = std::result::Result<T, BwError>;

pub struct BitwardenCli {
    program: PathBuf,
    session_cache: SessionCache,
    session: RefCell<Option<String>>,
}

impl BitwardenCli {
    pub fn new() -> Result<Self> {
        let program = which("bw").map_err(|_| BwError::MissingProgram)?;
        Ok(Self {
            program,
            session_cache: SessionCache::new()?,
            session: RefCell::new(None),
        })
    }

    pub fn ensure_session(&self) -> Result<()> {
        if self.session.borrow().is_some() {
            return Ok(());
        }

        if let Ok(session) = env::var("BW_SESSION") {
            let trimmed = session.trim();
            if !trimmed.is_empty() {
                self.session_cache.store(trimmed)?;
                *self.session.borrow_mut() = Some(trimmed.to_owned());
                return Ok(());
            }
        }

        if let Some(session) = self.session_cache.load()? {
            *self.session.borrow_mut() = Some(session);
            return Ok(());
        }

        self.unlock_and_store()?;
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        self.with_retry(|session| self.run_ok(["sync"], Some(session)))
    }

    pub fn list_login_items(&self) -> Result<Vec<VaultItem>> {
        let output =
            self.with_retry(|session| self.run_capture(["list", "items"], Some(session)))?;

        let mut items: Vec<VaultItem> =
            serde_json::from_str(&output).map_err(|source| BwError::JsonCommand {
                command: "bw list items".to_owned(),
                source,
            })?;

        items.retain(|item| item.login.is_some());
        items.sort_by_cached_key(|item| (!item.favorite, item.name.to_lowercase()));
        Ok(items)
    }

    pub fn get_totp(&self, item_id: &str) -> Result<Option<String>> {
        let session = self.active_session()?;
        let output = self.run_capture(["get", "totp", item_id], Some(&session))?;
        let totp = output.trim();
        if totp.is_empty() {
            Ok(None)
        } else {
            Ok(Some(totp.to_owned()))
        }
    }

    fn active_session(&self) -> Result<String> {
        self.session
            .borrow()
            .clone()
            .ok_or(BwError::SessionNotInitialized)
    }

    fn refresh_session(&self) -> Result<String> {
        self.session_cache.clear()?;
        *self.session.borrow_mut() = None;
        self.unlock_and_store()
    }

    fn unlock_and_store(&self) -> Result<String> {
        let session = match self.status()? {
            VaultStatus::Unauthenticated => return Err(BwError::NotLoggedIn),
            VaultStatus::Locked | VaultStatus::Unlocked => self.unlock()?,
            VaultStatus::Unknown(status) => return Err(BwError::UnknownStatus(status)),
        };
        self.session_cache.store(&session)?;
        *self.session.borrow_mut() = Some(session.clone());
        Ok(session)
    }

    fn with_retry<T>(&self, mut op: impl FnMut(&str) -> Result<T>) -> Result<T> {
        let session = self.active_session()?;
        match op(&session) {
            Ok(value) => Ok(value),
            Err(err) if is_session_error(&err) => {
                let new_session = self.refresh_session()?;
                op(&new_session)
            }
            Err(err) => Err(err),
        }
    }

    fn status(&self) -> Result<VaultStatus> {
        let output = self.run_capture(["status", "--raw"], None)?;
        let status: StatusResponse =
            serde_json::from_str(&output).map_err(|source| BwError::JsonCommand {
                command: "bw status --raw".to_owned(),
                source,
            })?;
        Ok(VaultStatus::from(status.status))
    }

    fn unlock(&self) -> Result<String> {
        let command = "bw unlock --raw".to_owned();
        let output = Command::new(&self.program)
            .arg("unlock")
            .arg("--raw")
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|source| BwError::SpawnCommand {
                command: command.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(BwError::CommandFailedStatus {
                command,
                status: output.status.to_string(),
            });
        }

        let session = String::from_utf8(output.stdout)
            .map_err(|source| BwError::CommandUtf8 {
                command: command.clone(),
                source,
            })?
            .trim()
            .to_owned();

        if session.is_empty() {
            return Err(BwError::EmptyUnlockSession);
        }

        Ok(session)
    }

    fn run_ok<const N: usize>(&self, args: [&str; N], session: Option<&str>) -> Result<()> {
        self.run_capture(args, session).map(|_| ())
    }

    fn run_capture<const N: usize>(
        &self,
        args: [&str; N],
        session: Option<&str>,
    ) -> Result<String> {
        let mut command = Command::new(&self.program);
        command.args(args);

        if let Some(session) = session {
            command.env("BW_SESSION", session);
        }

        let rendered = render_command(&args);
        let output = command.output().map_err(|source| BwError::SpawnCommand {
            command: rendered.clone(),
            source,
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stderr.is_empty() {
                return Err(BwError::CommandFailedStatus {
                    command: rendered,
                    status: output.status.to_string(),
                });
            }
            return Err(BwError::CommandFailed {
                command: rendered,
                stderr,
            });
        }

        String::from_utf8(output.stdout).map_err(|source| BwError::CommandUtf8 {
            command: rendered,
            source,
        })
    }
}

fn is_session_error(err: &BwError) -> bool {
    match err {
        BwError::CommandFailed { stderr, .. } => {
            let lower = stderr.to_lowercase();
            lower.contains("vault is locked")
                || lower.contains("not logged in")
                || lower.contains("session key is invalid")
                || lower.contains("invalid session")
        }
        _ => false,
    }
}

fn render_command<const N: usize>(args: &[&str; N]) -> String {
    format!("bw {}", args.join(" "))
}

struct SessionCache {
    path: PathBuf,
}

impl SessionCache {
    fn new() -> Result<Self> {
        Ok(Self {
            path: session_cache_path()?,
        })
    }

    fn load(&self) -> Result<Option<String>> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                let session = contents.trim().to_owned();
                if session.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(session))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(BwError::ReadFile {
                path: self.path.clone(),
                source,
            }),
        }
    }

    fn store(&self, session: &str) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| BwError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&self.path, session).map_err(|source| BwError::WriteFile {
            path: self.path.clone(),
            source,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).map_err(
                |source| BwError::SetPermissions {
                    path: self.path.clone(),
                    source,
                },
            )?;
        }

        Ok(())
    }

    fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(BwError::WriteFile {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

fn session_cache_path() -> Result<PathBuf> {
    let base = if let Some(runtime) = env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime)
    } else if let Some(cache_home) = env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(cache_home)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".cache")
    } else {
        return Err(BwError::MissingCacheDir);
    };

    Ok(base.join("bitwarden-tui").join("bw-session"))
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub login: Option<Login>,
}

impl VaultItem {
    pub fn username(&self) -> Option<&str> {
        self.login
            .as_ref()?
            .username
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    pub fn password(&self) -> Option<&str> {
        self.login
            .as_ref()?
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    pub fn first_uri(&self) -> Option<&str> {
        self.login
            .as_ref()?
            .uris
            .iter()
            .filter_map(|uri| uri.uri.as_deref())
            .map(str::trim)
            .find(|uri| !uri.is_empty())
    }

    pub fn has_totp(&self) -> bool {
        self.login
            .as_ref()
            .and_then(|login| login.totp.as_deref())
            .is_some_and(|value| !value.is_empty())
    }

    pub fn has_notes(&self) -> bool {
        self.notes
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Login {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub totp: Option<String>,
    #[serde(default)]
    pub uris: Vec<LoginUri>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginUri {
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VaultStatus {
    Unauthenticated,
    Locked,
    Unlocked,
    Unknown(String),
}

impl From<String> for VaultStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "unauthenticated" => Self::Unauthenticated,
            "locked" => Self::Locked,
            "unlocked" => Self::Unlocked,
            _ => Self::Unknown(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BwError, Login, LoginUri, VaultItem, is_session_error};

    fn sample_item() -> VaultItem {
        VaultItem {
            id: "item-id".to_owned(),
            name: "GitHub".to_owned(),
            notes: Some("  personal account  ".to_owned()),
            favorite: true,
            login: Some(Login {
                username: Some("octocat".to_owned()),
                password: Some("secret".to_owned()),
                totp: Some("AAAA".to_owned()),
                uris: vec![
                    LoginUri {
                        uri: Some("".to_owned()),
                    },
                    LoginUri {
                        uri: Some("https://github.com/login".to_owned()),
                    },
                ],
            }),
        }
    }

    #[test]
    fn picks_first_non_empty_uri() {
        assert_eq!(sample_item().first_uri(), Some("https://github.com/login"));
    }

    #[test]
    fn reports_totp_presence() {
        assert!(sample_item().has_totp());
    }

    #[test]
    fn reports_notes_presence() {
        assert!(sample_item().has_notes());
    }

    #[test]
    fn missing_notes_are_reported() {
        let item = VaultItem {
            id: "1".to_owned(),
            name: "Example".to_owned(),
            notes: Some("   ".to_owned()),
            favorite: false,
            login: None,
        };
        assert!(!item.has_notes());
    }

    #[test]
    fn session_error_matches_bw_locked_message() {
        let err = BwError::CommandFailed {
            command: "bw list items".to_owned(),
            stderr: "Vault is locked.".to_owned(),
        };
        assert!(is_session_error(&err));
    }

    #[test]
    fn session_error_matches_bw_not_logged_in() {
        let err = BwError::CommandFailed {
            command: "bw list items".to_owned(),
            stderr: "You are not logged in.".to_owned(),
        };
        assert!(is_session_error(&err));
    }

    #[test]
    fn session_error_does_not_match_unrelated_stderr() {
        let err = BwError::CommandFailed {
            command: "bw sync".to_owned(),
            stderr: "Network error".to_owned(),
        };
        assert!(!is_session_error(&err));
    }
}
