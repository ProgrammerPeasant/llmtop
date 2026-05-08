pub mod app;
pub mod collectors;
pub mod config;
pub mod pricing;
pub mod ui;

#[derive(Debug)]
pub enum AppEvent {
    Input(crossterm::event::Event),
    Tick,
    Hardware(collectors::HardwareSnapshot),
    Ollama(Vec<collectors::ModelInfo>),
}
