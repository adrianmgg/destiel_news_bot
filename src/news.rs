use miette::{IntoDiagnostic, Result};
use reqwest::Url;
use schemars::JsonSchema;
use serde::Deserialize;
use custom_debug::Debug;

#[derive(Debug, Deserialize, JsonSchema)]
pub enum NewsSource {
    BBC {
        #[debug(format = "{}")]
        url: Url,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NewsSources {
    pub sources: Vec<NewsSource>,
}

#[derive(Debug)]
pub struct NewsStory {
    pub id: String,
    pub headline: String,
    pub story_url: String,
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

    use crate::news::{BBCApiResponse, BBCApiResponseAsset};

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

pub async fn request_news_source(client: reqwest::Client, source: NewsSource) -> Result<Option<NewsStory>> {
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
