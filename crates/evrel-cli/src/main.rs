//! Evrel command-line interface.

mod args;
mod error;

use std::{
    fs,
    io::{self, Write},
    process::ExitCode,
};

use clap::Parser;
use evrel_compiler::{CompileInput, compile};

use crate::{args::Arguments, error::CliError};

fn main() -> ExitCode {
    match run(Arguments::parse()) {
        Ok(()) => ExitCode::SUCCESS,

        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Arguments) -> Result<(), CliError> {
    let source = fs::read_to_string(&arguments.input).map_err(|source| CliError::ReadSource {
        path: arguments.input.clone(),
        source,
    })?;

    let source_name = arguments.input.to_string_lossy();
    let output = compile(CompileInput::new(&source_name, &source))?;

    io::stdout()
        .lock()
        .write_all(output.code().as_bytes())
        .map_err(CliError::WriteOutput)
}
