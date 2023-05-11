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

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiResponse<RT> {
    pub meta: ApiResponseMeta,
    pub response: RT,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiResponseMeta {
    /// "The 3-digit HTTP Status-Code (e.g., 200)"
    pub status: i32,
    /// "The HTTP Reason-Phrase (e.g., OK)"
    pub msg: String,
    /// unknown/unhandled fields
    #[serde(flatten)]
    pub other_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserInfoResponse {
    pub user: User,
    /// unknown/unhandled fields
    #[serde(flatten)]
    pub other_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    /// "The number of blogs the user is following"
    pub following: i64,
    /// "The default posting format - html, markdown, or raw"
    pub default_post_format: String, // TODO enum
    /// "The user's tumblr short name"
    pub name: String,
    /// "The total count of the user's likes"
    pub likes: i64,
    /// "Each item is a blog the user has permissions to post to"
    pub blogs: Vec<Blog>,
    /// unknown/unhandled fields
    #[serde(flatten)]
    pub other_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Blog {
    /// "the short name of the blog"
    pub name: String,
    /// "the URL of the blog"
    pub url: String,
    /// "the title of the blog"
    pub title: String,
    /// "indicates if this is the user's primary blog"
    pub primary: bool,
    /// "total count of followers for this blog"
    pub followers: i64,
    /// "indicate if posts are tweeted auto, Y, N"
    pub tweet: String, // TODO to bool
    /// "indicates whether a blog is public or private"
    #[serde(rename = "type")]
    pub blog_type: String, // TODO enum
    /// unknown/unhandled fields
    #[serde(flatten)]
    pub other_fields: serde_json::Map<String, serde_json::Value>,
}

// https://www.tumblr.com/docs/en/api/v2#posts---createreblog-a-post-neue-post-format
#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePostRequest {
    /// "An array of NPF content blocks to be used to make the post; in a reblog, this is any content you want to add."
    pub content: Vec<tumblr_api::npf::ContentBlock>,
    // /// "An array of NPF layout objects to be used to lay out the post content."
    // pub layout: Option<Vec<tumblr_api::npf::LayoutObject>>, // TODO
    /// "The initial state of the new post, such as "published" or "queued"."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>, // TODO enum
    /// "The exact future date and time (ISO 8601 format) to publish the post, if desired. This parameter will be ignored unless the state parameter is "queue"."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_on: Option<String>, // TODO some other type
    /// "The exact date and time (ISO 8601 format) in the past to backdate the post, if desired. This backdating does not apply to when the post shows up in the Dashboard."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>, // TODO some other type
    /// "A comma-separated list of tags to associate with the post."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// "A source attribution for the post content."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// "Whether or not to share this via any connected Twitter account on post publish. Defaults to the blog's global setting."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_to_twitter: Option<bool>,
    /// "Whether this should be a private answer, if this is an answer."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    /// "A custom URL slug to use in the post's permalink URL"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// "Who can interact with this when reblogging"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactability_reblog: Option<tumblr_api::api::ReblogInteractability>,
}
// TODO ^ currently just has the making a new post stuff, same endpoint is also the way to do reblogs.
//      maybe best to do it as an enum of new post / reblog? since which fields are required is different
//      between the two
// TODO should we add `other_fields`s to requests too? or just response stuff

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePostResponse {
    // TODO - "intentionally a string instead of an integer for 32bit device compatibility" - should make it an int
    /// "the id of the created post"
    id: String,
    /// unknown/unhandled fields
    #[serde(flatten)]
    pub other_fields: serde_json::Map<String, serde_json::Value>,
}


pub async fn tumblr_api_test(_api_config: &TumblrApiConfig) -> Result<()> {
    // let client = setup_client(api_config)?;

    let token_info_str = std::fs::read_to_string(".oauth2-token.json").into_diagnostic().wrap_err("failed to read saved token")?;
    let token_info: TokenInfo = serde_json::from_str(&token_info_str).into_diagnostic().wrap_err("failed to parse saved token")?;

    if token_info.is_expired()? {
        tracing::warn!("token appears to be expired!");
    }

    let mut headers = reqwest::header::HeaderMap::new();
    // headers.insert("Authorization", format!("Bearer {}", token_info.token_result));
    // TODO - make sure it's actually a bearer one?
    let mut auth_value = reqwest::header::HeaderValue::from_str(format!("Bearer {}", token_info.token_result.access_token().secret()).as_str())
        .into_diagnostic()?;
    auth_value.set_sensitive(true);
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .into_diagnostic()?;

    // let user_info: ApiResponse<UserInfoResponse> = client.get("https://api.tumblr.com/v2/user/info")
    //     .send()
    //     .await
    //     .into_diagnostic()?
    //     .json()
    //     .await
    //     .into_diagnostic()?;

    // tracing::info!("{:#?}", user_info);

    let request_body = CreatePostRequest {
        content: vec![
            tumblr_api::npf::ContentBlock::Text { text: "hello world!".to_string(), subtype: None, indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Chat)".to_string(),              subtype: Some(tumblr_api::npf::TextSubtype::Chat),              indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Heading1)".to_string(),          subtype: Some(tumblr_api::npf::TextSubtype::Heading1),          indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Heading2)".to_string(),          subtype: Some(tumblr_api::npf::TextSubtype::Heading2),          indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Indented)".to_string(),          subtype: Some(tumblr_api::npf::TextSubtype::Indented),          indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: OrderedListItem)".to_string(),   subtype: Some(tumblr_api::npf::TextSubtype::OrderedListItem),   indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Quirky)".to_string(),            subtype: Some(tumblr_api::npf::TextSubtype::Quirky),            indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: Quote)".to_string(),             subtype: Some(tumblr_api::npf::TextSubtype::Quote),             indent_level: None, formatting: None },
            tumblr_api::npf::ContentBlock::Text { text: "hello world! (subtype: UnorderedListItem)".to_string(), subtype: Some(tumblr_api::npf::TextSubtype::UnorderedListItem), indent_level: None, formatting: None },
        ],
        state: None,
        publish_on: None,
        date: None,
        tags: None,
        source_url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_owned()),
        send_to_twitter: None,
        is_private: None,
        slug: Some("this-post-gets-a-custom-slug".to_owned()),
        interactability_reblog: None,
    };

    tracing::info!("{}", serde_json::to_string_pretty(&request_body).into_diagnostic()?);

    let make_post_response: ApiResponse<serde_json::Value> = client.post("https://api.tumblr.com/v2/blog/amggs-theme-testing-thing/posts")
        .json(&request_body)
        .send()
        .await
        .into_diagnostic()?
        .json()
        .await
        .into_diagnostic()?;

    tracing::info!("{:#?}", make_post_response);

    Ok(())
}
