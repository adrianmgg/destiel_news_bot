use miette::{Context, IntoDiagnostic, Result};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, ClientSecret, Scope,
    TokenResponse, TokenUrl,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tumblr_api::client::Credentials;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TumblrApiConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokenInfo {
    request_time: chrono::DateTime<chrono::Utc>,
    token_result:
        oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
}

impl TokenInfo {
    fn is_expired(&self) -> Result<bool> {
        match self.token_result.expires_in() {
            Some(duration) => {
                let expires_at =
                    self.request_time + chrono::Duration::from_std(duration).into_diagnostic()?;
                Ok(chrono::Utc::now() > expires_at)
            }
            None => Ok(true), // TODO log warning?
        }
    }
}

pub async fn tumblr_auth_test(api_config: &TumblrApiConfig) -> Result<()> {
    let client = BasicClient::new(
        ClientId::new(api_config.client_id.clone()),
        Some(ClientSecret::new(api_config.client_secret.clone())),
        AuthUrl::new("https://www.tumblr.com/oauth2/authorize".to_string()).into_diagnostic()?,
        Some(
            TokenUrl::new("https://api.tumblr.com/v2/oauth2/token".to_string())
                .into_diagnostic()?,
        ),
    );

    let request_time = chrono::Utc::now();

    let token_result = client
        .exchange_client_credentials()
        .add_scope(Scope::new("write".to_string()))
        .request_async(async_http_client)
        .await
        .into_diagnostic()?;

    let token_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(".oauth2-token.json")
        .into_diagnostic()
        .wrap_err("failed to open token file for writing")?;

    serde_json::to_writer_pretty(
        token_file,
        &TokenInfo {
            request_time,
            token_result,
        },
    )
    .into_diagnostic()?;

    Ok(())
}

pub async fn tumblr_api_test(api_config: &TumblrApiConfig) -> Result<()> {
    let client = tumblr_api::client::Client::new(Credentials::new_oauth2(
        api_config.client_id.clone(),
        api_config.client_secret.clone(),
    ));

    let image_bytes = std::fs::read("./generated_0.png").into_diagnostic()?;

    let make_post_response = client
        .create_post(
            "amggs-theme-testing-thing",
            vec![
                tumblr_api::npf::ContentBlockImage::builder(vec![
                    tumblr_api::npf::MediaObject::builder(
                        tumblr_api::npf::MediaObjectContent::Identifier(
                            "image-attachment-0".into(),
                        ),
                    )
                    .mime_type("image/png")
                    .build(),
                ])
                .build(),
                tumblr_api::npf::ContentBlockText::builder(
                    "hello world (posted using tumblr_api's Client!)",
                )
                .build(),
            ],
        )
        .add_attachment(image_bytes.into(), "image/png", "image-attachment-0")
        .send()
        .await
        .into_diagnostic()?;

    tracing::info!("{:#?}", make_post_response);

    Ok(())
}
