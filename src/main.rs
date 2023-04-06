use std::{io::{Write, BufReader}, fs::OpenOptions};

use clap::Parser;
use destielbot_rs::cli::Cli;
use miette::{Context, IntoDiagnostic, Result};
use reqwest::Url;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
enum NewsSource {
    BBC { url: Url },
    Reuters { url: Url },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NewsSources {
    sources: Vec<NewsSource>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        destielbot_rs::cli::Commands::Schema { out_dir } => {
            std::fs::create_dir_all(&out_dir).into_diagnostic()?;
            let schema_file_path = out_dir.join("news-sources.schema.json");
            let mut schema_file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&schema_file_path)
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to open {}", schema_file_path.display()))?;
            let schema = schemars::schema_for!(NewsSources);
            schema_file
                .write(
                    serde_json::to_string_pretty(&schema)
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .into_diagnostic()?;
        }
        destielbot_rs::cli::Commands::Thing { sources_file_path } => {
            let sources_file = OpenOptions::new()
                .read(true)
                .create(false)
                .open(&sources_file_path)
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to open {}", sources_file_path.display()))?;
            let sources_reader = BufReader::new(sources_file);
            let sources: NewsSources = serde_json::from_reader(sources_reader)
                .into_diagnostic()
                .wrap_err("failed to parse sources list")?;
            dbg!(sources);
        }
    }

    Ok(())
}
