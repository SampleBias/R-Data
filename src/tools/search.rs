use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    organic_results: Option<Vec<OrganicResult>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrganicResult {
    title: String,
    link: String,
    snippet: Option<String>,
}

pub async fn google_search(query: &str, num_results: usize) -> Result<String> {
    let api_key = std::env::var("SERPAPI_KEY")
        .ok()
        .or_else(|| {
            crate::config::ConfigManager::load_config()
                .ok()
                .and_then(|c| c.serpapi_key)
        })
        .context("SERPAPI_KEY not set. Add it to .env or config for web search.")?;

    let url = format!(
        "https://serpapi.com/search.json?q={}&api_key={}&num={}",
        urlencoding::encode(query),
        api_key,
        num_results.min(20)
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to send SerpAPI request")?;

    if !response.status().is_success() {
        anyhow::bail!("SerpAPI error: {}", response.status());
    }

    let body: SerpApiResponse = response
        .json()
        .await
        .context("Failed to parse SerpAPI response")?;

    if let Some(err) = body.error {
        anyhow::bail!("SerpAPI error: {}", err);
    }

    let results = body.organic_results.unwrap_or_default();
    let mut output = Vec::new();
    for (i, r) in results.iter().take(num_results).enumerate() {
        let snippet = r.snippet.as_deref().unwrap_or("").replace('\n', " ");
        output.push(format!(
            "{}. {} | {} | {}",
            i + 1,
            r.title,
            r.link,
            snippet
        ));
    }

    Ok(if output.is_empty() {
        "No results found.".to_string()
    } else {
        output.join("\n\n")
    })
}
