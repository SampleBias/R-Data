# R-Data Agent — AI-Powered Data Science Refactor

## Overview

Transform R-Data into an **AI-powered data science agent** that:
- Is controlled entirely by **natural language**
- Uses **GLM 4.7** (Z.AI) and **SerpAPI** for internet search
- Walks users through analysis and visualization via **tool calling**
- Focuses on **longevity gene expression** (age and genes)
- Keeps the existing TUI with a friendly **equalizer-style loading animation**

Patterned after [Vybrid](https://github.com/SampleBias/vybrid) for tool-calling architecture.

---

## Phase 1: Dependencies & Config

| Task | Details |
|------|---------|
| Add reqwest | HTTP client for GLM API |
| Add dotenvy | Load `.env` (ZAI_API_KEY, SERPAPI_KEY) |
| Add async-stream | SSE streaming for GLM |
| Add console | Styled terminal output |
| Add futures | Stream utilities |
| Config | `api_key`, `api_base_url`, `model` (GLM-4.7), `serpapi_key` |
| .env.example | ZAI_API_KEY, SERPAPI_KEY (optional) |

---

## Phase 2: GLM 4.7 Client

| Task | Details |
|------|---------|
| Create `src/client/glm.rs` | Chat completion, streaming, tool calls (adapt from Vybrid) |
| Message types | `Message`, `ToolCall`, `FunctionCall`, `Tool`, `FunctionDef` |
| Streaming | SSE parsing, `chat_stream()` returns stream of chunks |
| Tool choice | `tool_choice: "auto"` for function calling |

---

## Phase 3: Data Science Tools (Definitions)

Define tools the AI can call to control R-Data:

| Tool | Description | Parameters |
|------|-------------|------------|
| `load_data` | Load CSV/JSON/Excel file(s) | `file_paths: string[]` |
| `get_data_info` | Get current dataset info (genes, ages, layout) | — |
| `run_summary_stats` | Summary statistics | — |
| `run_correlation` | Correlation matrix / heatmap | — |
| `run_histogram` | Histogram for a column | `column`, `bins` |
| `run_expression_vs_age` | Expression vs age (all genes) | — |
| `run_genes_significant_with_age` | Genes p<0.05 with age | — |
| `run_expression_trend` | Expression trend for selected genes | `gene_ids: string[]` |
| `run_young_vs_old` | Young vs Old scatter | `young_ages?`, `old_ages?` |
| `run_volcano_plot` | Volcano plot | — |
| `run_expression_heatmap` | Expression heatmap (genes × ages) | `top_n?` |
| `export_gene_correlation` | Export gene correlation to CSV | — |
| `open_visualization` | Open current chart in browser | — |
| `list_available_analyses` | List analyses available for current data | — |
| `google_search` | Search web (SerpAPI) | `query`, `num_results?` |

---

## Phase 4: Tool Executor

| Task | Details |
|------|---------|
| Create `src/tools/` | `definitions.rs`, `executor.rs` |
| Executor | `execute_tool(name, args)` → dispatches to R-Data operations |
| App context | Tools receive `&App` or shared state (datasets, viz engine) |
| Return format | JSON or plain text for AI consumption |

---

## Phase 5: AI Chat Mode & Conversation

| Task | Details |
|------|---------|
| Add Chat tab | Data | Analysis | Viz | **Chat** |
| Chat input | Natural language prompt (e.g. "Load my data and show genes significant with age") |
| System prompt | Data science agent: longevity, microarray, R-Data capabilities |
| Conversation | `Conversation` struct (messages, tool results) |
| Process flow | User → GLM (with tools) → tool calls → results → follow-up response |
| Streaming | Show "Thinking" and "Assistant" output in TUI |

---

## Phase 6: TUI Integration

| Task | Details |
|------|---------|
| Chat panel | Input area + scrollable conversation |
| Loading state | **Equalizer-style animation** (vertical bars, varying height, grayscale) |
| Tool execution feedback | "Running load_data..." with loading animation |
| Tab flow | Chat as primary entry; Data/Analysis/Viz still usable via keyboard |

---

## Phase 7: Loading Animation

| Task | Details |
|------|---------|
| Design | Clusters of vertical bars, varying heights, white/gray on dark background |
| Animation | Bars pulse/grow/shrink over time (implied motion) |
| Implementation | Custom ratatui widget or canvas drawing |
| Usage | Show during: data load, analysis run, AI processing |

---

## Phase 8: System Prompt & Polish

| Task | Details |
|------|---------|
| System prompt | R-Data agent persona: data science, longevity, gene expression |
| Capabilities | List all tools and when to use them |
| Workflow | "Load data first, then run analyses, then visualize" |
| Help text | Update for Chat tab, natural language usage |

---

## File Structure (Target)

```
src/
├── main.rs
├── client/
│   └── glm.rs           # GLM 4.7 API client
├── config/
│   └── settings.rs
├── data/
│   ├── ingestion.rs
│   └── analysis.rs
├── viz/
│   ├── types.rs
│   ├── engine.rs
│   └── availability.rs
├── runner.rs
├── tools/
│   ├── definitions.rs   # Data science tool schemas
│   └── executor.rs      # Tool dispatcher
├── ui/
│   ├── components.rs
│   ├── tui.rs
│   └── loading.rs       # Equalizer loading widget (optional)
└── conversation.rs      # Chat history + tool results
```

---

## Execution Order

1. **Phase 1** — Dependencies & config ✅
2. **Phase 2** — GLM client ✅
3. **Phase 3** — Tool definitions ✅
4. **Phase 4** — Tool executor (wired to App state) ✅
5. **Phase 5** — Chat tab + AI loop ✅
6. **Phase 6** — TUI integration ✅
7. **Phase 7** — Loading animation ✅
8. **Phase 8** — System prompt & help ✅

## Status: Implemented

All phases complete. Run with `cargo run`. Set `ZAI_API_KEY` in `.env` for AI. Optional `SERPAPI_KEY` for web search.

---

## Notes

- **Not a coding agent** — focused on data analysis and visualization
- **Natural language** — user says "load my data" or "show genes that change with age"
- **R-Data as foundation** — all existing analyses and viz remain; AI invokes them via tools
