use miette::{Context, IntoDiagnostic, Result};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, ClientSecret, Scope,
    TokenResponse, TokenUrl,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
        Some(TokenUrl::new("https://api.tumblr.com/v2/oauth2/token".to_string()).into_diagnostic()?),
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

pub async fn tumblr_api_test(_api_config: &TumblrApiConfig) -> Result<()> {
    // let client = setup_client(api_config)?;

    let token_info_str = std::fs::read_to_string(".oauth2-token.json")
        .into_diagnostic()
        .wrap_err("failed to read saved token")?;
    let token_info: TokenInfo = serde_json::from_str(&token_info_str)
        .into_diagnostic()
        .wrap_err("failed to parse saved token")?;

    if token_info.is_expired()? {
        tracing::warn!("token appears to be expired!");
    }

    let mut headers = reqwest::header::HeaderMap::new();
    // headers.insert("Authorization", format!("Bearer {}", token_info.token_result));
    // TODO - make sure it's actually a bearer one?
    let mut auth_value = reqwest::header::HeaderValue::from_str(
        format!("Bearer {}", token_info.token_result.access_token().secret()).as_str(),
    )
    .into_diagnostic()?;
    auth_value.set_sensitive(true);
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .into_diagnostic()?;

    let request_body = serde_json::json!({
        "content": [
            {"type": "image", "media": [{
                "type": "image/png",
                "identifier": "image-attachment-0",
                "width": 813,
                "height": 924,
            }]},
            {"type": "text", "text": "hello world!"},
        ]
    });

    tracing::info!(
        "{}",
        serde_json::to_string_pretty(&request_body).into_diagnostic()?
    );

    let image_bytes = std::fs::read("./generated_0.png").into_diagnostic()?;

    let body_part = reqwest::multipart::Part::text(serde_json::to_string(&request_body).into_diagnostic()?)
        .mime_str("application/json").into_diagnostic()?;
    let image_part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("generated_0.png")
        .mime_str("image/png").into_diagnostic()?;
    let form = reqwest::multipart::Form::new()
        .part("json", body_part)
        .part("image-attachment-0", image_part);

    let make_post_response: tumblr_api::api::ApiResponse<serde_json::Value> = client
        .post("https://api.tumblr.com/v2/blog/amggs-theme-testing-thing/posts")
        // .json(&request_body)
        .multipart(form)
        .send()
        .await
        .into_diagnostic()?
        .json()
        .await
        .into_diagnostic()?;

    tracing::info!("{:#?}", make_post_response);

    Ok(())
}
