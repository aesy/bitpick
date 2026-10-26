use thiserror::Error;

use crate::bw::BwError;
use crate::clipboard::ClipboardError;
use crate::tui::TuiError;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Bitwarden(#[from] BwError),

    #[error(transparent)]
    Clipboard(#[from] ClipboardError),

    #[error(transparent)]
    Tui(#[from] TuiError),

    #[error("Cancelled")]
    Cancelled,

    #[error("No login items were found in the Bitwarden vault")]
    NoItems,

    #[error("{field} is not available for the selected item")]
    MissingItemField { field: &'static str },

    #[error("Clipboard is unavailable. Use `--print` or install a compatible clipboard backend.")]
    ClipboardUnavailable,
}
