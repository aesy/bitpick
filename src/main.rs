use std::process::ExitCode;

use bitwarden_tui::error::AppError;

fn main() -> ExitCode {
    match bitwarden_tui::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Cancelled) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
