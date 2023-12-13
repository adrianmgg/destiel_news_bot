use clap::Parser;
use destielbot_rs::{
    cli::{Cli, ConfigFileArgs},
    image::{generate_image, ImageGenConfig},
    news::{request_news_source, NewsSource, NewsStory},
};
use futures::StreamExt;
use miette::{Context, IntoDiagnostic, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashSet;
use tokio::{fs, io::AsyncWriteExt};
use tracing_subscriber::{prelude::*, Layer};

#[derive(Debug, JsonSchema)]
struct IgnoreSchema {}

#[derive(Debug, Deserialize, JsonSchema)]
struct Config {
    image_gen_cfg: ImageGenConfig,
    /// api endpoints to pull articles from
    news_sources: Vec<NewsSource>,
    postprocessors: Vec<Postprocessor>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Postprocessor {
    #[serde(deserialize_with = "serde_regex::deserialize")]
    #[schemars(with = "String")]
    pattern: regex::Regex,
    substitution: String,
}

impl Postprocessor {
    fn postprocess(&self, story: &mut NewsStory) {
        let old_headline = story.headline.clone();
        // TODO can prob do this better/more optimized
        story.headline = self
            .pattern
            .replace_all(&story.headline, &self.substitution)
            .into();
        tracing::info!("postprocessed headline {:?} -> {:?}", old_headline, &story.headline);
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ApiConfig {
    tumblr_api: TumblrApiConfig,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TumblrApiConfig {
    pub client_id: String,
    pub client_secret: String,
}

fn load_config(config_info: &ConfigFileArgs) -> Result<(Config, ApiConfig)> {
    let config = std::fs::read_to_string(&config_info.config_file_path)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to open config file ({})",
                config_info.config_file_path.display()
            )
        })?;
    let api_config = std::fs::read_to_string(&config_info.apiconfig_file_path)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to open config file ({})",
                config_info.apiconfig_file_path.display()
            )
        })?;
    let config = serde_json::from_str::<Config>(&config)
        .into_diagnostic()
        .wrap_err("failed to parse config file")?;
    let api_config = serde_json::from_str::<ApiConfig>(&api_config)
        .into_diagnostic()
        .wrap_err("failed to parse config file")?;
    Ok((config, api_config))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut layers = Vec::new();

    layers.push(
        tracing_subscriber::fmt::layer()
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                cli.log_level,
            ))
            .boxed(),
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
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                    cli.logfile_level,
                ))
                .boxed(),
        );
    }

    tracing_subscriber::registry().with(layers).init();

    match cli.command {
        destielbot_rs::cli::Commands::Schema { out_dir } => {
            fs::create_dir_all(&out_dir).await.into_diagnostic()?;
            let config_schema_file_path = out_dir.join("config.schema.json");
            let apiconfig_schema_file_path = out_dir.join("api.schema.json");
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&config_schema_file_path)
                .await
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to open output file ({})",
                        config_schema_file_path.display()
                    )
                })?
                .write_all(
                    serde_json::to_string_pretty(&schemars::schema_for!(Config))
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .await
                .into_diagnostic()?;
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&apiconfig_schema_file_path)
                .await
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to open output file ({})",
                        apiconfig_schema_file_path.display()
                    )
                })?
                .write_all(
                    serde_json::to_string_pretty(&schemars::schema_for!(ApiConfig))
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .await
                .into_diagnostic()?;
        }
        destielbot_rs::cli::Commands::Run { config_info } => {
            let (config, apiconfig) = load_config(&config_info)?;
            let client = reqwest::Client::builder().build().into_diagnostic()?;
            let tumblrclient =
                tumblr_api::client::Client::new(tumblr_api::client::Credentials::new_oauth2(
                    apiconfig.tumblr_api.client_id.clone(),
                    apiconfig.tumblr_api.client_secret.clone(),
                ));
            let mut seen_news_urls = HashSet::<String>::new(); // TODO should be saving/loading this so it works across runs?
            loop {
                tracing::debug!("polling news sources");
                // TODO wait the buffer should probably be before the request not after right? oops
                let cur_stories: Vec<_> = tokio_stream::iter(&config.news_sources)
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
                    // TODO can we avoid the collect into vec here? we just re-iter it right after
                    //      the await anyways
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .filter_map(|story| {
                        if seen_news_urls.contains(&story.story_url) {
                            None
                        } else {
                            seen_news_urls.insert(story.story_url.clone());
                            Some(story)
                        }
                    })
                    // postprocess headlines
                    .map(|mut story| {
                        for pp in &config.postprocessors {
                            pp.postprocess(&mut story);
                        }
                        story
                    })
                    .collect();
                if !cur_stories.is_empty() {
                    tracing::info!("got stories: {:?}", &cur_stories);
                    for story in cur_stories {
                        let mut image_data = Vec::<u8>::new();
                        generate_image(&config.image_gen_cfg, &story.headline, &mut image_data)?;
                        let create_post_result = tumblrclient
                            .create_post(
                                // TODO make target blog settable in config file
                                "destiel-news-bot",
                                vec![
                                    // tumblr_api::npf::ContentBlockText::builder(format!(
                                    //     "got news story: {:?}",
                                    //     &story
                                    // ))
                                    // .build(),
                                    tumblr_api::npf::ContentBlockImage::builder(vec![
                                        tumblr_api::npf::MediaObject::builder(
                                            tumblr_api::npf::MediaObjectContent::Identifier(
                                                "image-attachment".into(),
                                            ),
                                        )
                                        .build(),
                                    ])
                                    // TODO make alt text template settable in config file
                                    .alt_text(format!("the destiel confession meme edited to read \"I love you\" / \"{}\"", story.headline))
                                    .build(),
                                ],
                            )
                            .source_url(story.story_url)
                            .add_attachment(
                                reqwest::Body::from(image_data),
                                "image/png",
                                "image-attachment",
                            )
                            .send()
                            .await;
                        match create_post_result {
                            Err(err) => {
                                tracing::error!(
                                    "encountered error trying to post to tumblr: {:?}",
                                    err
                                );
                            }
                            _ => {
                                tracing::info!("posted to tumblr successfully");
                            }
                        }
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(30)).await
            }
        }
        destielbot_rs::cli::Commands::ImageTest { config_info } => {
            let (config, _apiconfig) = load_config(&config_info)?;
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
        }
    }

    Ok(())
}
