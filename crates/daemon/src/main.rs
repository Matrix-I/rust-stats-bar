use std::process::ExitCode;

fn main() -> ExitCode {
    match async_io::block_on(statsbard::run::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("statsbard: {error}");
            ExitCode::FAILURE
        }
    }
}
