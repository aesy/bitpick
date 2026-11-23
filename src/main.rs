use std::process::ExitCode;

use bitpick::error::AppError;

fn main() -> ExitCode {
    match bitpick::run() {
        Ok(()) | Err(AppError::Cancelled) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        },
    }
}
