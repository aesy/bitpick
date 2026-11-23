use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use which::which;

#[derive(Debug, Error)]
pub enum RbwError {
    #[error("Could not find `rbw` in PATH")]
    MissingProgram,

    #[error("Unlocking the vault failed: {reason}")]
    UnlockFailed { reason: String },

    #[error("Failed to run `{command}`")]
    SpawnCommand {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{command} failed: {stderr}")]
    CommandFailed { command: String, stderr: String },

    #[error("{command} failed ({status})")]
    CommandFailedStatus { command: String, status: String },

    #[error("Failed to send input to `{command}`")]
    WriteInput {
        command: String,
        #[source]
        source: std::io::Error,
    },

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
}

pub type Result<T> = std::result::Result<T, RbwError>;

const PROFILE_ENV: &str = "RBW_PROFILE";

// rbw forwards these to pinentry. Without them pinentry prompts on the terminal
// (curses) instead of opening a window.
const DISPLAY_ENV: [&str; 3] = ["DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE"];

pub struct Rbw {
    program: PathBuf,
    profile: Option<String>,
    profile_name: String,
}

impl Rbw {
    /// `profile` is passed to rbw as `RBW_PROFILE`; `None` keeps the inherited
    /// environment.
    pub fn new(profile: Option<String>) -> Result<Self> {
        let program = which("rbw").map_err(|_| RbwError::MissingProgram)?;
        let profile_name = profile
            .clone()
            .or_else(|| std::env::var(PROFILE_ENV).ok())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "default".to_owned());

        Ok(Self {
            program,
            profile,
            profile_name,
        })
    }

    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Unlocks if needed, optionally syncs, and lists the login items.
    pub fn load(&self, sync: bool) -> Result<Vec<VaultItem>> {
        self.ensure_unlocked()?;
        if sync {
            self.sync()?;
        }
        self.list_items()
    }

    pub fn is_unlocked(&self) -> bool {
        self.command()
            .arg("unlocked")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Unlocks the agent if needed. Must run while the terminal is free, since
    /// pinentry prompts on it.
    pub fn ensure_unlocked(&self) -> Result<()> {
        if self.is_unlocked() {
            return Ok(());
        }

        // rbw hands its stdin tty to pinentry, so stdin stays inherited.
        let output = self
            .command()
            .arg("unlock")
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|source| RbwError::SpawnCommand {
                command: "rbw unlock".to_owned(),
                source,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(RbwError::UnlockFailed {
                reason: if stderr.is_empty() {
                    output.status.to_string()
                } else {
                    stderr
                },
            });
        }

        Ok(())
    }

    fn sync(&self) -> Result<()> {
        self.run_capture(&["sync"]).map(|_| ())
    }

    fn list_items(&self) -> Result<Vec<VaultItem>> {
        let entries: Vec<ListEntry> = self.run_json(&["list", "--raw"])?;

        let mut items: Vec<VaultItem> = entries
            .into_iter()
            .filter_map(VaultItem::from_entry)
            .collect();
        items.sort_by_cached_key(|item| item.name.to_lowercase());
        Ok(items)
    }

    pub fn add(&self, login: &NewLogin) -> Result<()> {
        let mut args = vec!["add", login.name.as_str()];
        if !login.username.is_empty() {
            args.push(&login.username);
        }
        if !login.uri.is_empty() {
            args.extend(["--uri", &login.uri]);
        }
        if !login.folder.is_empty() {
            args.extend(["--folder", &login.folder]);
        }

        self.run_with_input(&args, &login_editor_contents(&login.password, &login.notes))
    }

    /// Replaces the password and notes of a login; rbw cannot change its other
    /// fields.
    pub fn edit_login(&self, id: &str, password: &str, notes: &str) -> Result<()> {
        self.run_with_input(&["edit", id], &login_editor_contents(password, notes))
    }

    pub fn edit_note(&self, id: &str, notes: &str) -> Result<()> {
        self.run_with_input(&["edit", id], notes)
    }

    pub fn get_entry(&self, id: &str) -> Result<Entry> {
        self.run_json(&["get", "--raw", id])
    }

    pub fn totp(&self, id: &str) -> Result<String> {
        Ok(self.run_capture(&["code", id])?.trim().to_owned())
    }

    fn run_json<T: DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let output = self.run_capture(args)?;
        serde_json::from_str(&output).map_err(|source| RbwError::JsonCommand {
            command: render_command(args),
            source,
        })
    }

    /// Runs an rbw command that would open `$EDITOR`; rbw reads the contents
    /// from stdin instead when stdin is not a terminal.
    fn run_with_input(&self, args: &[&str], input: &str) -> Result<()> {
        let rendered = render_command(args);
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| RbwError::SpawnCommand {
                command: rendered.clone(),
                source,
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|source| RbwError::WriteInput {
                    command: rendered.clone(),
                    source,
                })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|source| RbwError::SpawnCommand {
                command: rendered.clone(),
                source,
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.is_empty() {
            return Err(RbwError::CommandFailedStatus {
                command: rendered,
                status: output.status.to_string(),
            });
        }
        Err(RbwError::CommandFailed {
            command: rendered,
            stderr,
        })
    }

    fn run_capture(&self, args: &[&str]) -> Result<String> {
        let rendered = render_command(args);
        let output = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|source| RbwError::SpawnCommand {
                command: rendered.clone(),
                source,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stderr.is_empty() {
                return Err(RbwError::CommandFailedStatus {
                    command: rendered,
                    status: output.status.to_string(),
                });
            }
            return Err(RbwError::CommandFailed {
                command: rendered,
                stderr,
            });
        }

        String::from_utf8(output.stdout).map_err(|source| RbwError::CommandUtf8 {
            command: rendered,
            source,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        for name in DISPLAY_ENV {
            command.env_remove(name);
        }
        if let Some(profile) = &self.profile {
            command.env(PROFILE_ENV, profile);
        }
        command
    }
}

/// rbw's editor format for logins: the password on the first line, the notes
/// after it.
fn login_editor_contents(password: &str, notes: &str) -> String {
    format!("{password}\n{notes}\n")
}

fn render_command(args: &[&str]) -> String {
    format!("rbw {}", args.join(" "))
}

#[derive(Deserialize)]
struct ListEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    uris: Option<Vec<String>>,
    #[serde(default, rename = "type")]
    entry_type: Option<String>,
}

pub struct NewLogin {
    pub name: String,
    pub username: String,
    pub uri: String,
    pub folder: String,
    pub password: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Login,
    Note,
}

#[derive(Clone)]
pub struct VaultItem {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub username: Option<String>,
    pub folder: Option<String>,
    pub uris: Vec<String>,
}

impl VaultItem {
    /// `None` for entry types the picker does not offer (cards, identities, SSH
    /// keys).
    fn from_entry(entry: ListEntry) -> Option<Self> {
        let kind = match entry.entry_type.as_deref()? {
            "Login" => ItemKind::Login,
            "Note" => ItemKind::Note,
            _ => return None,
        };

        Some(Self {
            id: entry.id,
            kind,
            name: entry.name.unwrap_or_default(),
            username: entry.user,
            folder: entry.folder,
            uris: entry.uris.unwrap_or_default(),
        })
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref().filter(|value| !value.is_empty())
    }

    pub fn folder(&self) -> Option<&str> {
        self.folder.as_deref().filter(|value| !value.is_empty())
    }

    pub fn first_uri(&self) -> Option<&str> {
        self.uris
            .iter()
            .map(|uri| uri.trim())
            .find(|uri| !uri.is_empty())
    }
}

/// One entry from `rbw get --raw`. Only logins and secure notes are fetched; a
/// note's `data` is `null`.
#[derive(Deserialize)]
pub struct Entry {
    #[serde(default)]
    data: Option<LoginData>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
struct LoginData {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    totp: Option<String>,
}

impl Entry {
    pub fn password(&self) -> Option<&str> {
        self.data
            .as_ref()?
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    pub fn has_totp(&self) -> bool {
        self.data
            .as_ref()
            .and_then(|data| data.totp.as_deref())
            .is_some_and(|value| !value.is_empty())
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, ItemKind, ListEntry, VaultItem};

    #[test]
    fn parses_rbw_list_raw_output() {
        let json = r#"[
            {"id": "a", "name": "GitHub", "user": "octocat", "folder": null,
             "uris": ["", "https://github.com"], "type": "Login"},
            {"id": "b", "name": null, "user": null, "folder": "Private", "uris": null, "type": "Note"},
            {"id": "c", "name": "Visa", "user": null, "folder": null, "uris": null, "type": "Card"}
        ]"#;
        let entries: Vec<ListEntry> = serde_json::from_str(json).unwrap();
        let items: Vec<VaultItem> = entries
            .into_iter()
            .filter_map(VaultItem::from_entry)
            .collect();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, ItemKind::Login);
        assert_eq!(items[0].username(), Some("octocat"));
        assert_eq!(items[0].first_uri(), Some("https://github.com"));
        assert_eq!(items[1].kind, ItemKind::Note);
        assert_eq!(items[1].name, "");
        assert_eq!(items[1].folder(), Some("Private"));
        assert_eq!(items[1].first_uri(), None);
    }

    #[test]
    fn parses_rbw_get_raw_login() {
        let json = r#"{
            "id": "a", "folder": null, "name": "GitHub",
            "data": {"username": "octocat", "password": "secret", "totp": "JBSW",
                     "uris": [{"uri": "https://github.com", "match_type": null}]},
            "fields": [], "notes": "  recovery codes  ", "history": []
        }"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.password(), Some("secret"));
        assert!(entry.has_totp());
        assert_eq!(entry.notes(), Some("recovery codes"));
    }

    #[test]
    fn parses_rbw_get_raw_secure_note() {
        let json = r#"{"id": "b", "folder": null, "name": "Wifi", "data": null,
                       "fields": [], "notes": "pw: hunter2", "history": []}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.password(), None);
        assert!(!entry.has_totp());
        assert_eq!(entry.notes(), Some("pw: hunter2"));
    }

    #[test]
    fn empty_fields_are_missing() {
        let json = r#"{"data": {"password": "", "totp": null}, "notes": "   "}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.password(), None);
        assert!(!entry.has_totp());
        assert_eq!(entry.notes(), None);
    }
}
