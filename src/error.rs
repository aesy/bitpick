use thiserror::Error;

use crate::app::CopyField;
use crate::clipboard::ClipboardError;
use crate::rbw::RbwError;
use crate::tui::TuiError;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Rbw(#[from] RbwError),

    #[error(transparent)]
    Clipboard(#[from] ClipboardError),

    #[error(transparent)]
    Tui(#[from] TuiError),

    #[error("Cancelled")]
    Cancelled,

    #[error("The selected item has no {field}")]
    MissingItemField { field: CopyField },
}
