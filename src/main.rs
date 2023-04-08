use clap::Parser;
use destielbot_rs::{cli::{Cli, ConfigFileArgs}, news::{request_news_source, NewsSource}, image::{ImageGenConfig, generate_image}};
use miette::{Context, IntoDiagnostic, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt};
use futures::StreamExt;
use tracing_subscriber::{prelude::*, Layer};

#[derive(Debug, Deserialize, JsonSchema)]
struct Config {
    image_gen_cfg: ImageGenConfig,
    news_sources: Vec<NewsSource>,
}

fn load_config(config_info: &ConfigFileArgs) -> Result<Config> {
    let sources_str = std::fs::read_to_string(&config_info.config_file_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open config file ({})", config_info.config_file_path.display()))?;
    serde_json::from_str::<Config>(&sources_str)
        .into_diagnostic()
        .wrap_err("failed to parse config file")
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut layers = Vec::new();

    layers.push(
        tracing_subscriber::fmt::layer()
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(cli.log_level))
            .boxed()
    );

    if let Some(logfile_path) = &cli.logfile {
        if let Some(parent) = logfile_path.parent() {
            fs::create_dir_all(parent).await.into_diagnostic()?;
        }
        let logfile = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&logfile_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to open log file ({})", logfile_path.display()))?;
        layers.push(
            tracing_subscriber::fmt::layer()
                .with_writer(std::sync::Arc::new(logfile))
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(cli.logfile_level))
                .boxed()
        );
    }

    tracing_subscriber::registry()
        .with(layers)
        .init();

    match cli.command {
        destielbot_rs::cli::Commands::Schema { out_dir } => {
            fs::create_dir_all(&out_dir).await.into_diagnostic()?;
            let schema_file_path = out_dir.join("config.schema.json");
            let mut schema_file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&schema_file_path)
                .await
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to open output file ({})", schema_file_path.display()))?;
            let schema = schemars::schema_for!(Config);
            schema_file
                .write_all(
                    serde_json::to_string_pretty(&schema)
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .await
                .into_diagnostic()?;
        }
        destielbot_rs::cli::Commands::Thing { config_info } => {
            let config = load_config(&config_info)?;
            let client = reqwest::Client::builder().build().into_diagnostic()?;
            let stories: Vec<_> = tokio_stream::iter(config.news_sources)
                .map(|source| {
                    // client is already using an arc internally, so cloning it here doesn't actually clone the underlying stuff
                    request_news_source(client.clone(), source)
                })
                .buffer_unordered(2)
                .filter_map(|x| async move {
                    match x {
                        Ok(Some(story)) => Some(story),
                        Ok(None) => None, // TODO - debug log here that it succeeded but got nothing?
                        Err(e) => {
                            // "{:?}" gives the format we want (miette's fancy stuff)
                            tracing::error!("encountered error while requesting news: {:?}", e);
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
                .await;
            tracing::info!("{:?}", stories);
        },
        destielbot_rs::cli::Commands::ImageTest { config_info } => {
            let config = load_config(&config_info)?;
            for (i, headline) in std::fs::read_to_string("headlines.txt")
                .into_diagnostic()?
                .lines()
                .enumerate()
            {
                let mut outfile = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(format!("./generated_{}.png", i))
                    .into_diagnostic()
                    .wrap_err("failed to open output file")?;
                generate_image(&config.image_gen_cfg, &headline, &mut outfile)?;
            }
        },
    }

    Ok(())
}
