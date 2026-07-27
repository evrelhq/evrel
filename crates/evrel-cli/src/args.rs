//! Command-line arguments.

use std::path::PathBuf;

use clap::Parser;

/// Evrel JavaScript and TypeScript compiler.
#[derive(Debug, Parser)]
#[command(name = "evrel", version, about)]
pub(crate) struct Arguments {
    /// JavaScript or TypeScript module to compile.
    pub(crate) input: PathBuf,
}
