mod data;
mod viz;
mod runner;
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
    let config = ConfigManager::load_config()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new(config)?;

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

