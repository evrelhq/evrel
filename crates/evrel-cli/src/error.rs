//! Command-line errors.

use std::{io, path::PathBuf};

use evrel_compiler::CompilerError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("failed to read `{}`", path.display())]
    ReadSource {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Compile(#[from] CompilerError),

    #[error("failed to write generated JavaScript")]
    WriteOutput(#[source] io::Error),
}
