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
    sources: Vec<NewsSource>
}



fn main() {
    println!("Hello, world!");
    let schema = schemars::schema_for!(NewsSources);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
