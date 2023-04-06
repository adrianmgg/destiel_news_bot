
use clap::Parser;
use destielbot_rs::cli::Cli;
use miette::{Context, IntoDiagnostic, Result};
use reqwest::Url;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt};
use futures::StreamExt;
use custom_debug::Debug;

#[derive(Debug, Deserialize, JsonSchema)]
enum NewsSource {
    BBC {
        #[debug(format = "{}")]
        url: Url,
    },
    Reuters {
        #[debug(format = "{}")]
        url: Url,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NewsSources {
    sources: Vec<NewsSource>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        destielbot_rs::cli::Commands::Schema { out_dir } => {
            fs::create_dir_all(&out_dir).await.into_diagnostic()?;
            let schema_file_path = out_dir.join("news-sources.schema.json");
            let mut schema_file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&schema_file_path)
                .await
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to open output file ({})", schema_file_path.display()))?;
            let schema = schemars::schema_for!(NewsSources);
            schema_file
                .write_all(
                    serde_json::to_string_pretty(&schema)
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .await
                .into_diagnostic()?;
        }
        destielbot_rs::cli::Commands::Thing { sources_file_path } => {
            let sources_str = fs::read_to_string(&sources_file_path)
                .await
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to open sources list ({})", sources_file_path.display()))?;
            let sources: NewsSources = serde_json::from_str(&sources_str)
                .into_diagnostic()
                .wrap_err("failed to parse sources list")?;
            let mut stream = tokio_stream::iter(sources.sources)
                .map(|source| {
                    async move {
                        tracing::info!("start {:?}", source);
                        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                        tracing::info!("end   {:?}", source);
                    }
                })
                .buffer_unordered(2);
            while let Some(_) = stream.next().await {
                // ...
            }
        }
    }

    Ok(())
}
