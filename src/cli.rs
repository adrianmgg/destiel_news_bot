
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help(true))]
pub struct Cli {
    // TODO: log level flag? (like in https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html#quick-start)

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// write schema files
    Schema {
        /// folder to write schemas into (will be created if it doesn't already exist)
        #[arg(short, long, value_name = "FILE", default_value = "./schemas/")]
        out_dir: PathBuf
    },
}
