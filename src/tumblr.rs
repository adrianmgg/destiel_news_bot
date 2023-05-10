use miette::{Result, IntoDiagnostic, Context};
use oauth2::{basic::BasicClient, ClientId, ClientSecret, AuthUrl, TokenUrl, Scope, reqwest::async_http_client, TokenResponse};
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
    token_result: oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
}

impl TokenInfo {
    fn is_expired(&self) -> Result<bool> {
        match self.token_result.expires_in() {
            Some(duration) => {
                let expires_at = self.request_time + chrono::Duration::from_std(duration).into_diagnostic()?;
                Ok(chrono::Utc::now() > expires_at)
            },
            None => Ok(true),  // TODO log warning?
        }
    }
}

fn setup_client(api_config: &TumblrApiConfig) -> Result<oauth2::Client<oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>, oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>, oauth2::basic::BasicTokenType, oauth2::StandardTokenIntrospectionResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>, oauth2::StandardRevocableToken, oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>>> {
    Ok(BasicClient::new(
        ClientId::new(api_config.client_id.clone()),
        Some(ClientSecret::new(api_config.client_secret.clone())),
        AuthUrl::new("https://www.tumblr.com/oauth2/authorize".to_string()).into_diagnostic()?,
        Some(TokenUrl::new("https://api.tumblr.com/v2/oauth2/token".to_string()).into_diagnostic()?),
    ))
}

pub async fn tumblr_auth_test(api_config: &TumblrApiConfig) -> Result<()> {
    let client = setup_client(api_config)?;

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

    serde_json::to_writer_pretty(token_file, &TokenInfo {
        request_time,
        token_result,
    }).into_diagnostic()?;

    Ok(())
}

pub async fn tumblr_api_test(api_config: &TumblrApiConfig) -> Result<()> {
    let _client = setup_client(api_config)?;

    let token_info_str = std::fs::read_to_string(".oauth2-token.json").into_diagnostic().wrap_err("failed to read saved token")?;
    let token_info: TokenInfo = serde_json::from_str(&token_info_str).into_diagnostic().wrap_err("failed to parse saved token")?;

    if token_info.is_expired()? {
        tracing::warn!("token appears to be expired!");
    }

    Ok(())
}
