# llmtop

> The only GPU monitor that knows what model is running and how much each token costs you in energy and dollar-equivalent.

`llmtop` is a realtime terminal monitor for **local LLM servers** (Ollama, llama.cpp coming soon). It links your GPU's wattage to the model that's burning it — so you can finally answer:

- Which loaded model is eating my VRAM right now?
- How many joules per token does this 32B coder cost me?
- If I were paying Claude Sonnet API instead, what would this generation have cost?
- How much energy did this work session burn? How much CO₂?

```
╭─ llmtop · NVIDIA RTX 4090 · 24 GB ───────────────────────────╮
│ MODEL                  VRAM      TOK/S   POWER   J/TOK  $/1K* │
│ qwen2.5-coder:32b      18.2 GB    47.3   312 W    6.6   0.015 │
│ deepseek-r1:7b          4.1 GB     0.0    78 W   idle   ----- │
│                                                               │
│ ── GPU util · 87% ────────────────────────────────────────────│
│  ▁▂▃▅▆▇▇▇▆▇▇▇▆▆▇▇▇▆▇▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇                  │
│ ── Power · 391 W / 450 W ────────────────────────────────────│
│  ▃▄▅▇▇▆▇▇▇▆▇▇▇▆▆▇▇▇▆▇▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇▆▇▇▇                  │
│                                                               │
│ Session 0:42:11 · 0.84 kWh · $0.13 · 0.34 kg CO₂eq            │
│ [q]uit  [p]ause  [c]lear                                      │
╰───────────────────────────────────────────────────────────────╯
```

\* per-1K output tokens, equivalent cost on Claude Sonnet (configurable: `--compare gpt-4o | gemini-2.5`)

## Why not nvtop / nvitop / asitop?

| Feature | nvtop | nvitop | asitop | **llmtop** |
|---|:---:|:---:|:---:|:---:|
| GPU util / VRAM / power | ✅ | ✅ | ✅ | ✅ |
| **Knows loaded LLM models** | ❌ | ❌ | ❌ | ✅ |
| **Per-model VRAM share** | ❌ | ❌ | ❌ | ✅ |
| **Joules per token** | ❌ | ❌ | ❌ | ✅ |
| **API-equivalent $ cost** | ❌ | ❌ | ❌ | ✅ |
| **Session kWh + CO₂** | ❌ | ❌ | ❌ | ✅ |
| Cross-platform | partial | ⚠️ | macOS only | ✅ Linux + Win + macOS |

## Install

```bash
cargo install llmtop
```

Pre-built binaries (Linux, macOS, Windows) — see [Releases](../../releases).

## Usage

Run alongside Ollama:

```bash
llmtop                                  # default: poll http://127.0.0.1:11434
llmtop --ollama-url http://gpu:11434    # remote ollama
llmtop --compare gpt-4o                 # change cost-equivalent provider
llmtop --grid-co2 230                   # your local grid intensity (gCO₂/kWh)
```

Hotkeys: `q` quit · `p` pause · `c` clear session totals.

## What's measured

| Metric | Source |
|---|---|
| GPU util / VRAM / power / temp | NVML (Linux + Windows), IOReport (macOS, planned v0.2) |
| Loaded models + per-model VRAM | Ollama `/api/ps` |
| Tokens/sec (live) | proxy mode (planned v0.2) — `--proxy` flag |
| J/token | `power_w / tokens_per_sec` |
| Session kWh | trapezoidal integration of GPU power over time |
| API-equivalent $ | configurable provider price tables (`pricing/mod.rs`) |
| CO₂eq | session kWh × `--grid-co2` (gCO₂/kWh) |

## Roadmap

- [ ] **v0.2** — Apple Silicon (M1–M5) via IOReport, llama.cpp Prometheus endpoint, proxy mode for live tokens/sec
- [ ] **v0.3** — vLLM, LM Studio, MLX
- [ ] **v0.4** — Prometheus exporter, JSON metrics, write-to-file mode
- [ ] **v0.5** — Multi-GPU, AMD ROCm, Intel Arc

## Found wrong API price? PR welcome

Edit `src/pricing/mod.rs`. The cost-equivalent table is community-maintained.

## License

MIT
