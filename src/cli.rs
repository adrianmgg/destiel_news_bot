
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help(true))]
pub struct Cli {
    /// lowest log level to display to stdout (error, warn, info, debug, or trace)
    #[arg(long, default_value = "warn")]
    pub log_level: tracing::Level,
    
    /// lowest log level to write to log file (error, warn, info, debug, or trace)
    #[arg(long, default_value = "warn")]
    pub logfile_level: tracing::Level,

    #[arg(long)]
    pub logfile: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// write schema files
    Schema {
        /// folder to write schemas into (will be created if it doesn't already exist)
        #[arg(short, long, value_name = "FILE", default_value = "schemas")]
        out_dir: PathBuf,
    },
    Thing {
        /// json file with list of api sources
        #[arg(long = "sources", value_name = "FILE", default_value = "sources.json")]
        sources_file_path: PathBuf,
    },
}
