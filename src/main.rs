use clap::Parser;
use destielbot_rs::cli::Cli;
use miette::{Context, IntoDiagnostic, Result};
use reqwest::Url;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt};
use futures::StreamExt;
use custom_debug::Debug;
use tracing_subscriber::{prelude::*, Layer};

#[derive(Debug, Deserialize, JsonSchema)]
enum NewsSource {
    BBC {
        #[debug(format = "{}")]
        url: Url,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NewsSources {
    sources: Vec<NewsSource>,
}

#[derive(Debug)]
struct NewsStory {
    id: String,
    headline: String,
    story_url: String,
}

// from https://stackoverflow.com/a/69458453/8762161
pub fn object_empty_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    for<'a> T: Deserialize<'a>,
{
    #[derive(Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    struct Empty {}

    #[derive(Deserialize, Debug)]
    #[serde(untagged)]
    enum Aux<T> {
        T(T),
        Empty(Empty),
        Null,
    }

    match serde::Deserialize::deserialize(deserializer)? {
        Aux::T(t) => Ok(Some(t)),
        Aux::Empty(_) | Aux::Null => Ok(None),
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BBCApiResponseAsset {
    asset_id: String,
    asset_uri: String,
    headline: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct BBCApiResponse {
    #[serde(deserialize_with = "object_empty_as_none")]
    asset: Option<BBCApiResponseAsset>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{BBCApiResponse, BBCApiResponseAsset};

    #[test]
    fn decode_bbc_response_empty() {
        assert_eq!(
            Some(BBCApiResponse { asset: None }),
            serde_json::from_value::<Option<BBCApiResponse>>(json!({"isError":false,"pollPeriod":30000,"asset":{}})).unwrap()
        );
    }

    #[test]
    fn decode_bbc_response_nonempty() {
        assert_eq!(
            Some(BBCApiResponse { asset: Some(BBCApiResponseAsset { asset_id: "1337".to_string(), asset_uri: "/news/uk-1337".to_string(), headline: "Hello World".to_string() }) }),
            serde_json::from_value::<Option<BBCApiResponse>>(json!({"isError":false,"pollPeriod":30000,"asset":{"assetId":"1337","assetUri":"/news/uk-1337","headline":"Hello World"}})).unwrap()
        );
    }
}

async fn request_news_source(client: reqwest::Client, source: NewsSource) -> Result<Option<NewsStory>> {
    match source {
        NewsSource::BBC { url } => {
            let response: BBCApiResponse = client.get(url)
                .send()
                .await.into_diagnostic()?
                .json()
                .await.into_diagnostic()?;
            match response.asset {
                Some(asset) => Ok(Some(NewsStory{
                    id: format!("BBC_{}", asset.asset_id),
                    headline: asset.headline,
                    story_url: format!("https://bbc.co.uk{}", asset.asset_uri),  // TODO - use Url instead?
                })),
                _ => Ok(None),
            }
        },
    }
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
            let schema_file_path = out_dir.join("news-sources.schema.json");
            let mut schema_file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
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
            let client = reqwest::Client::builder().build().into_diagnostic()?;
            let stories: Vec<_> = tokio_stream::iter(sources.sources)
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
                // TODO would be better to do the filter out nones without the second collect
                .collect::<Vec<_>>()
                .await;
            tracing::info!("{:?}", stories);
        }
    }

    Ok(())
}
