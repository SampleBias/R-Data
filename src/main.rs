mod data;
mod viz;
mod ai;
mod ui;
mod config;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use std::io;

use config::ConfigManager;
use ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present (API key can be set via R_DATA_AGENT_API_KEY)
    dotenvy::dotenv().ok();

    let config = ConfigManager::load_config()?;
    let api_key = config.api_key.clone();
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new(api_key)?;

    if let Err(err) = app.run(&mut terminal).await {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        eprintln!("Error: {}", err);
        return Err(err);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;

    Ok(())
}

