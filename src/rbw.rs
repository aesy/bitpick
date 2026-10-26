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

    #[error("Unlocking the vault with `rbw unlock` failed ({status})")]
    UnlockFailed { status: String },

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

pub struct Rbw {
    program: PathBuf,
}

impl Rbw {
    pub fn new() -> Result<Self> {
        let program = which("rbw").map_err(|_| RbwError::MissingProgram)?;
        Ok(Self { program })
    }

    pub fn is_unlocked(&self) -> bool {
        Command::new(&self.program)
            .arg("unlocked")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Unlocks the agent if needed. Must run while the terminal is free, since rbw may prompt
    /// through pinentry on it.
    pub fn ensure_unlocked(&self) -> Result<()> {
        if self.is_unlocked() {
            return Ok(());
        }

        let status = Command::new(&self.program)
            .arg("unlock")
            .status()
            .map_err(|source| RbwError::SpawnCommand {
                command: "rbw unlock".to_owned(),
                source,
            })?;

        if !status.success() {
            return Err(RbwError::UnlockFailed {
                status: status.to_string(),
            });
        }

        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        self.run_capture(&["sync"]).map(|_| ())
    }

    pub fn list_login_items(&self) -> Result<Vec<VaultItem>> {
        let entries: Vec<ListEntry> = self.run_json(&["list", "--raw"])?;

        let mut items: Vec<VaultItem> = entries
            .into_iter()
            .filter(|entry| entry.entry_type.as_deref() == Some("Login"))
            .map(VaultItem::from)
            .collect();
        items.sort_by_cached_key(|item| item.name.to_lowercase());
        Ok(items)
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

    fn run_capture(&self, args: &[&str]) -> Result<String> {
        let rendered = render_command(args);
        let output = Command::new(&self.program)
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

#[derive(Clone)]
pub struct VaultItem {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub folder: Option<String>,
    pub uris: Vec<String>,
}

impl From<ListEntry> for VaultItem {
    fn from(entry: ListEntry) -> Self {
        Self {
            id: entry.id,
            name: entry.name.unwrap_or_default(),
            username: entry.user,
            folder: entry.folder,
            uris: entry.uris.unwrap_or_default(),
        }
    }
}

impl VaultItem {
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

/// One entry from `rbw get --raw`. Only login entries are fetched, so `data` is login data.
#[derive(Deserialize)]
pub struct Entry {
    data: LoginData,
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
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
    }

    pub fn has_totp(&self) -> bool {
        self.data
            .totp
            .as_deref()
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
    use super::{Entry, ListEntry, VaultItem};

    #[test]
    fn parses_rbw_list_raw_output() {
        let json = r#"[
            {"id": "a", "name": "GitHub", "user": "octocat", "folder": null,
             "uris": ["", "https://github.com"], "type": "Login"},
            {"id": "b", "name": null, "user": null, "folder": "Cards", "uris": null, "type": "Card"}
        ]"#;
        let entries: Vec<ListEntry> = serde_json::from_str(json).unwrap();
        let items: Vec<VaultItem> = entries.into_iter().map(VaultItem::from).collect();

        assert_eq!(items[0].username(), Some("octocat"));
        assert_eq!(items[0].first_uri(), Some("https://github.com"));
        assert_eq!(items[1].name, "");
        assert_eq!(items[1].folder(), Some("Cards"));
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
    fn empty_fields_are_missing() {
        let json = r#"{"data": {"password": "", "totp": null}, "notes": "   "}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.password(), None);
        assert!(!entry.has_totp());
        assert_eq!(entry.notes(), None);
    }
}
