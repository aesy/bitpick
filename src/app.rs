use std::fmt;
use std::time::Duration;

use clap::{Parser, ValueEnum};

use crate::bw::{BitwardenCli, VaultItem};
use crate::clipboard::{self, Clipboard};
use crate::error::{AppError, Result};
use crate::tui::{ActionVerb, PickerConfig, PickerOutcome, run_picker};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Fast Bitwarden credential picker powered by ratatui"
)]
pub struct Cli {
    /// Initial search text shown in the TUI.
    #[arg(short, long)]
    pub query: Option<String>,

    /// Sync the vault before loading items.
    #[arg(long)]
    pub sync: bool,

    /// Print the selected value to stdout instead of copying it.
    #[arg(long)]
    pub print: bool,

    /// Force Enter to copy a specific field.
    #[arg(short, long, value_enum)]
    pub field: Option<CopyField>,

    /// Seconds after which the clipboard is cleared. Set 0 to disable.
    #[arg(long, default_value_t = 45)]
    pub clipboard_clear: u64,

    #[arg(long = "__clear-clipboard-after", hide = true)]
    pub clear_clipboard_after: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CopyField {
    Username,
    Password,
    Totp,
    Uri,
    Notes,
}

impl CopyField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Username => "username",
            Self::Password => "password",
            Self::Totp => "TOTP",
            Self::Uri => "URI",
            Self::Notes => "notes",
        }
    }
}

impl fmt::Display for CopyField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(secs) = cli.clear_clipboard_after {
        clipboard::run_clear_timer(Duration::from_secs(secs))?;
        return Ok(());
    }

    let bitwarden = BitwardenCli::new()?;
    bitwarden.ensure_session()?;

    if cli.sync {
        bitwarden.sync()?;
    }

    let items = bitwarden.list_login_items()?;
    if items.is_empty() {
        return Err(AppError::NoItems);
    }

    let mut clipboard = if cli.print {
        None
    } else {
        Clipboard::new().ok()
    };
    let initial_status = if cli.print || clipboard.is_some() {
        None
    } else {
        Some(AppError::ClipboardUnavailable.to_string())
    };

    let verb = if cli.print {
        ActionVerb::Print
    } else {
        ActionVerb::Copy
    };

    let outcome = run_picker(
        &items,
        PickerConfig {
            initial_query: cli.query.unwrap_or_default(),
            fixed_action: cli.field,
            initial_status,
            verb,
        },
        |item, action| -> Result<String> { selected_value(&bitwarden, item, action) },
    )?;

    match outcome {
        PickerOutcome::Cancelled => Err(AppError::Cancelled),
        PickerOutcome::Selected { value, .. } => {
            if cli.print {
                println!("{value}");
                return Ok(());
            }

            let clipboard = clipboard.as_mut().ok_or(AppError::ClipboardUnavailable)?;
            clipboard.copy(&value)?;

            if cli.clipboard_clear > 0 {
                let _ =
                    clipboard::spawn_clear_timer(&value, Duration::from_secs(cli.clipboard_clear));
            }
            Ok(())
        }
    }
}

pub fn selected_value(
    bitwarden: &BitwardenCli,
    item: &VaultItem,
    field: CopyField,
) -> Result<String> {
    match field {
        CopyField::Username => item
            .username()
            .map(ToOwned::to_owned)
            .ok_or(AppError::MissingItemField { field: "username" }),
        CopyField::Password => item
            .password()
            .map(ToOwned::to_owned)
            .ok_or(AppError::MissingItemField { field: "password" }),
        CopyField::Totp => {
            if !item.has_totp() {
                return Err(AppError::MissingItemField { field: "TOTP" });
            }
            bitwarden
                .get_totp(&item.id)?
                .ok_or(AppError::MissingItemField { field: "TOTP" })
        }
        CopyField::Uri => item
            .first_uri()
            .map(ToOwned::to_owned)
            .ok_or(AppError::MissingItemField { field: "URI" }),
        CopyField::Notes => item
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or(AppError::MissingItemField { field: "notes" }),
    }
}

#[cfg(test)]
mod tests {
    use super::CopyField;

    #[test]
    fn copy_field_labels_are_stable() {
        assert_eq!(CopyField::Username.label(), "username");
        assert_eq!(CopyField::Password.label(), "password");
        assert_eq!(CopyField::Totp.label(), "TOTP");
        assert_eq!(CopyField::Uri.label(), "URI");
        assert_eq!(CopyField::Notes.label(), "notes");
    }
}
