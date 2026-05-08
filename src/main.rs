use clap::Parser;
use color_eyre::eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use llmtop::{AppEvent, app::App, collectors, config::Cli, ui};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::{io, time::Duration};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, cli).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, cli: Cli) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(cli);

    // Input task — blocking crossterm reads in spawn_blocking.
    let input_tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            if crossterm::event::poll(Duration::from_millis(200)).unwrap_or(false)
                && let Ok(ev) = crossterm::event::read()
                && input_tx.send(AppEvent::Input(ev)).is_err()
            {
                break;
            }
        }
    });

    // Tick — redraw heartbeat.
    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Hardware poll (Day-1 stub).
    let hw_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            let snap = collectors::poll_hardware().await;
            if hw_tx.send(AppEvent::Hardware(snap)).is_err() {
                break;
            }
        }
    });

    // Ollama poll.
    let ollama_url = app.cli.ollama_url.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let models = collectors::poll_ollama(&ollama_url).await;
            if tx.send(AppEvent::Ollama(models)).is_err() {
                break;
            }
        }
    });

    terminal.draw(|f| ui::draw(f, &app))?;
    while let Some(ev) = rx.recv().await {
        match ev {
            AppEvent::Input(input) => {
                if app.handle_input(input) {
                    break;
                }
            }
            AppEvent::Tick => {}
            AppEvent::Hardware(snap) => app.update_hardware(snap),
            AppEvent::Ollama(models) => app.update_ollama(models),
        }
        terminal.draw(|f| ui::draw(f, &app))?;
    }
    Ok(())
}
