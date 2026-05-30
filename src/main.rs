mod album_art;
mod audio;
mod builder;
mod cli;
mod error;
mod metadata;
mod midi;
mod song_ini;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match builder::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // `{:#}` prints the full anyhow context chain on one line.
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
