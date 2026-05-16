use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "llmtop", version, about = "Realtime TUI monitor for local LLMs")]
pub struct Cli {
    /// Ollama base URL.
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    pub ollama_url: String,

    /// Compare per-token cost vs this provider model (claude-sonnet | gpt-4o | gemini-2.5).
    #[arg(long, default_value = "claude-sonnet")]
    pub compare: String,

    /// CO2 grid intensity in gCO2eq per kWh (default = world average).
    #[arg(long, default_value_t = 475.0)]
    pub grid_co2: f64,

    /// Disable color output.
    #[arg(long)]
    pub no_color: bool,

    /// Transparent reverse proxy port in front of Ollama. Point your client at
    /// `http://127.0.0.1:<port>` (e.g. `OLLAMA_HOST=...`) to get live tok/s.
    /// Enabled by default; disable with `--no-proxy`.
    #[arg(long, value_name = "PORT", default_value_t = 11435)]
    pub proxy: u16,

    /// Disable the reverse proxy.
    #[arg(long)]
    pub no_proxy: bool,
}

impl Cli {
    pub fn proxy_port(&self) -> Option<u16> {
        if self.no_proxy { None } else { Some(self.proxy) }
    }
}
