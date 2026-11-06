# llmtop

[![ci](https://github.com/ProgrammerPeasant/llmtop/actions/workflows/ci.yml/badge.svg)](https://github.com/ProgrammerPeasant/llmtop/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/llmtop.svg)](https://crates.io/crates/llmtop)
[![license](https://img.shields.io/crates/l/llmtop.svg)](LICENSE)

Terminal GPU monitor that links wattage to the LLM burning it. Tells you VRAM share, joules per token, hosted-API equivalent cost, and session energy/CO2.

Works with Ollama and llama.cpp (`llama-server`).

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
llmtop --llamacpp-url http://127.0.0.1:8080   # also poll llama-server
llmtop --no-ollama --llamacpp-url http://127.0.0.1:8080   # llama.cpp only
```

Hotkeys: `p` pause, `c` clear, `g` tok/s + VRAM charts, `?` help, `q` quit.

Colors follow the [tui-tools](https://github.com/tui-tools/tui-kit) look: Tokyo Night by default, your active Omarchy theme when one is installed, any Omarchy-style `colors.toml` via `TUI_THEME=/path/to/colors.toml`, and none at all with `NO_COLOR=1` or `--no-color`.

### Live tokens/sec

Ollama does not expose live throughput on `/api/ps`. llmtop runs a reverse proxy by default on `:11435`, forwards every request to upstream Ollama unchanged, and reads `eval_count` / `eval_duration` from the responses. Point your client at the proxy:

```bash
OLLAMA_HOST=http://127.0.0.1:11435 ollama run qwen2.5-coder:7b "explain quicksort"
```

Direct traffic to `:11434` is invisible — `TOK/S` and `J/TOK` stay idle unless requests pass through the proxy.

### llama.cpp

Start `llama-server` with `--metrics` so llmtop can read throughput from its Prometheus endpoint, then point llmtop at it:

```bash
llama-server -m model.gguf --metrics
llmtop --llamacpp-url http://127.0.0.1:8080
```

No proxy is needed — tok/s comes straight from `/metrics`. Ollama is still polled alongside; add `--no-ollama` to skip it (and the `:11435` proxy). llama-server does not report per-model VRAM, so the `VRAM` column shows `—` for llama.cpp rows. The `SRC` column shows which backend each row came from.

## What's measured

| Metric                         | Source                                              |
| ------------------------------ | --------------------------------------------------- |
| GPU util / VRAM / power / temp | NVML (Linux, Windows). IOReport (macOS) in v0.2.    |
| Multi-GPU                      | Aggregated (sum power/VRAM, avg util) in v0.1.      |
| Loaded models, per-model VRAM  | Ollama `/api/ps`; llama.cpp `/v1/models` (no VRAM)  |
| Context window (`CTX`)         | llama.cpp: `n_ctx` from `/slots` (fallback `/props`). Ollama: `context_length` from `/api/ps` (newer releases) |
| Tok/s + VRAM history (`g`)     | Combined tok/s across all models; GPU VRAM used from NVML / IOReport |
| Tokens/sec live                | Ollama: reverse proxy parses `eval_count` / `eval_duration` from `/api/generate` and `/api/chat`. llama.cpp: `/metrics` (`--metrics`) counters plus in-flight `n_decoded` from `/slots` |
| J/token                        | `power_w / tokens_per_sec`                          |
| Session kWh                    | Trapezoidal integration of GPU power over time      |
| API-equivalent $               | Provider price tables in `src/pricing/mod.rs`       |
| CO2eq                          | session kWh × `--grid-co2` (gCO2/kWh)               |

## Roadmap

- v0.2: Apple Silicon (M1–M5) via IOReport, per-GPU breakdown (`--per-gpu`)
- v0.3: vLLM, LM Studio, MLX
- v0.4: Prometheus exporter, JSON metrics, write-to-file mode
- v0.5: AMD ROCm, Intel Arc

## Wrong API price?

Edit `src/pricing/mod.rs` and open a PR.

## License

MIT
