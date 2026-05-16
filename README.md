# llmtop

Terminal GPU monitor that links wattage to the LLM burning it. Tells you VRAM share, joules per token, hosted-API equivalent cost, and session energy/CO2.

Ollama today. llama.cpp next.

![demo](media/demo.gif)

`$/1K` = per-1K output tokens at the hosted-API price. Default: Claude Sonnet. Override with `--compare gpt-4o | gemini-2.5`.

## vs nvtop / nvitop / asitop

| Feature                   | nvtop   | nvitop | asitop     | **llmtop**            |
| ------------------------- | :-----: | :----: | :--------: | :-------------------: |
| GPU util / VRAM / power   | yes     | yes    | yes        | yes                   |
| Knows loaded LLM models   | no      | no     | no         | yes                   |
| Per-model VRAM share      | no      | no     | no         | yes                   |
| Joules per token          | no      | no     | no         | yes                   |
| API-equivalent $ cost     | no      | no     | no         | yes                   |
| Session kWh + CO2         | no      | no     | no         | yes                   |
| Cross-platform            | partial | partial| macOS only | Linux + Win + macOS\* |

\* Apple Silicon: v0.2.

## Install

```bash
cargo install llmtop
```

## Usage

```bash
llmtop                                  # default: proxy on at :11435
llmtop --ollama-url http://gpu:11434    # remote ollama
llmtop --compare gpt-4o                 # change cost-equivalent provider
llmtop --grid-co2 230                   # local grid intensity (gCO2/kWh)
llmtop --proxy 11500                    # custom proxy port
llmtop --no-proxy                       # disable proxy
```

Hotkeys: `q:quit  p:pause  c:clear`.

### Live tokens/sec

Ollama does not expose live throughput on `/api/ps`. llmtop runs a reverse proxy by default on `:11435`, forwards every request to upstream Ollama unchanged, and reads `eval_count` / `eval_duration` from the responses. Point your client at the proxy:

```bash
OLLAMA_HOST=http://127.0.0.1:11435 ollama run qwen2.5-coder:7b "explain quicksort"
```

Direct traffic to `:11434` is invisible — `TOK/S` and `J/TOK` stay idle unless requests pass through the proxy.

## What's measured

| Metric                         | Source                                              |
| ------------------------------ | --------------------------------------------------- |
| GPU util / VRAM / power / temp | NVML (Linux, Windows). IOReport (macOS) in v0.2.    |
| Multi-GPU                      | Aggregated (sum power/VRAM, avg util) in v0.1.      |
| Loaded models, per-model VRAM  | Ollama `/api/ps`                                    |
| Tokens/sec live                | Reverse proxy parses `eval_count` / `eval_duration` from `/api/generate` and `/api/chat` |
| J/token                        | `power_w / tokens_per_sec`                          |
| Session kWh                    | Trapezoidal integration of GPU power over time      |
| API-equivalent $               | Provider price tables in `src/pricing/mod.rs`       |
| CO2eq                          | session kWh × `--grid-co2` (gCO2/kWh)               |

## Roadmap

- v0.2: Apple Silicon (M1–M5) via IOReport, llama.cpp Prometheus, per-GPU breakdown (`--per-gpu`)
- v0.3: vLLM, LM Studio, MLX
- v0.4: Prometheus exporter, JSON metrics, write-to-file mode
- v0.5: AMD ROCm, Intel Arc

## Wrong API price?

Edit `src/pricing/mod.rs` and open a PR.

## License

MIT
