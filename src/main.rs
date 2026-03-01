mod client;
mod config;
mod conversation;
mod data;
mod runner;
mod tools;
mod viz;
mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    event::{DisableBracketedPaste, EnableBracketedPaste},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use std::io;

use config::ConfigManager;
use ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    if std::env::args().any(|a| a == "--test-api") {
        return test_api().await;
    }

    let config = ConfigManager::load_config()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new(config)?;

    if let Err(err) = app.run(&mut terminal).await {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        )?;
        eprintln!("Error: {}", err);
        return Err(err);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;

    Ok(())
}

/// Test Z.AI Coding Plan API key (endpoint: api.z.ai/api/coding/paas/v4)
async fn test_api() -> Result<()> {
    use client::GlmClient;

    let config = ConfigManager::load_config()?;
    let api_key = config
        .api_key
        .clone()
        .ok_or_else(|| anyhow::anyhow!("No API key. Set ZAI_API_KEY or ZHIPU_API_KEY in .env"))?;
    let base_url = config
        .api_base_url
        .clone()
        .unwrap_or_else(|| "https://api.z.ai/api/coding/paas/v4".to_string());
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "glm-4.7-flash".to_string());

    let client = GlmClient::new(api_key, base_url, model.clone());
    println!("Testing Z.AI Coding Plan API...");
    println!("  Endpoint: {}", client.chat_completions_url());
    println!("  Model: {}", model);
    println!();
    let msg = client
        .chat(
            vec![crate::client::glm::Message {
                role: "user".to_string(),
                content: Some("Say hello in one word.".to_string()),
                tool_calls: None,
                tool_call_id: None,
            }],
            None,
        )
        .await?;

    let content = msg.content.unwrap_or_default();
    println!("✓ API key valid!");
    println!("  Response: {}", content.trim());
    Ok(())
}

