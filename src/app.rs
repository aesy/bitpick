use std::fmt;
use std::time::Duration;

use clap::{Parser, ValueEnum};

use crate::clipboard::{self, Clipboard, ClipboardError};
use crate::error::{AppError, Result};
use crate::rbw::{Rbw, VaultItem};
use crate::tui::{ActionVerb, FzfFilter, PickerConfig, PickerOutcome, run_picker};

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

    /// rbw profile to use (sets `RBW_PROFILE`). Repeat to switch between profiles with Tab;
    /// an empty name is rbw's default profile.
    #[arg(long = "profile", value_name = "NAME")]
    pub profiles: Vec<String>,

    /// Print the selected value to stdout instead of copying it.
    #[arg(long)]
    pub print: bool,

    /// Force Enter to copy a specific field.
    #[arg(short, long, value_enum)]
    pub field: Option<CopyField>,

    /// Seconds after which the clipboard is cleared. Set 0 to disable.
    #[arg(long, default_value_t = 45)]
    pub clipboard_clear: u64,

    #[arg(long = "__clipboard-worker", hide = true)]
    pub clipboard_worker: Option<u64>,
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

    if let Some(secs) = cli.clipboard_worker {
        clipboard::run_worker((secs > 0).then(|| Duration::from_secs(secs)))?;
        return Ok(());
    }

    let vaults = if cli.profiles.is_empty() {
        vec![Rbw::new(None)?]
    } else {
        cli.profiles
            .iter()
            .map(|profile| Rbw::new(Some(profile.clone())))
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let filter = FzfFilter::new()?;

    let (mut clipboard, initial_status) = if cli.print {
        (None, None)
    } else {
        match Clipboard::new() {
            Ok(clipboard) => (Some(clipboard), None),
            Err(error) => (None, Some(error.to_string())),
        }
    };

    let verb = if cli.print {
        ActionVerb::Print
    } else {
        ActionVerb::Copy
    };

    let outcome = run_picker(
        &vaults,
        filter,
        PickerConfig {
            initial_query: cli.query.unwrap_or_default(),
            fixed_action: cli.field,
            initial_status,
            verb,
            sync: cli.sync,
        },
        |vault, item, action| -> Result<String> { selected_value(vault, item, action) },
    )?;

    match outcome {
        PickerOutcome::Cancelled => Err(AppError::Cancelled),
        PickerOutcome::Selected { value, .. } => {
            if cli.print {
                println!("{value}");
                return Ok(());
            }

            let clipboard = clipboard.as_mut().ok_or(ClipboardError::Unavailable)?;
            let clear_after =
                (cli.clipboard_clear > 0).then(|| Duration::from_secs(cli.clipboard_clear));

            if clipboard.needs_worker() {
                clipboard::spawn_worker(&value, clear_after)?;
                return Ok(());
            }

            clipboard.copy(&value)?;

            if let Some(delay) = clear_after
                && let Err(error) = clipboard::spawn_worker(&value, Some(delay))
            {
                eprintln!("Warning: The clipboard will not be cleared automatically: {error}");
            }

            Ok(())
        }
    }
}

pub fn selected_value(rbw: &Rbw, item: &VaultItem, field: CopyField) -> Result<String> {
    let value = match field {
        CopyField::Username => item.username().map(ToOwned::to_owned),
        CopyField::Uri => item.first_uri().map(ToOwned::to_owned),
        CopyField::Password => rbw.get_entry(&item.id)?.password().map(ToOwned::to_owned),
        CopyField::Notes => rbw.get_entry(&item.id)?.notes().map(ToOwned::to_owned),
        CopyField::Totp => {
            if rbw.get_entry(&item.id)?.has_totp() {
                Some(rbw.totp(&item.id)?)
            } else {
                None
            }
        }
    };

    value
        .filter(|value| !value.is_empty())
        .ok_or(AppError::MissingItemField { field })
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
