# R-Data Agent: Longevity Gene Expression Build Plan

## Overview

Transform the application into a **longevity research tool** for analyzing gene expression microarray data, focused on identifying aging markers (expression signature changes from young → old). Remove AI assistant. Add visualization availability logic. Orient for microarray data layout.

---

## Phase 1: Remove AI Assistant

**Scope:** Remove AI tab and all AI-related code.

| Task | Details |
|------|---------|
| Remove AI tab | Change `Tab` enum: Data, Analysis, Visualizations only |
| Remove AITab struct | Delete from components.rs |
| Remove AI agent | Remove ai module, AIClient, config API key |
| Update tab navigation | Remove AI from next_tab/previous_tab cycles |
| Update main.rs | Remove api_key from App::new |
| Simplify config | Remove api_key from config (optional: keep for future) |
| Update help | Remove AI tab from help text |
| Clean Cargo.toml | Remove reqwest, dotenvy if only used for AI |

---

## Phase 2: Microarray Data Layout Support

**Data format (from sample):**
- **Row 1 (header):** `Gene ID` in column A, then **ages as column headers** (17, 18, 21, 24, 27, 28, 29, 34, 35, …)
- **Column A:** Ensembl gene IDs (e.g. `ENSG0000001`, `ENSG000001`)
- **Columns B+:** Expression values (float) for each gene at each age
- Ages may repeat (e.g. 38, 38 or 42, 42, 42) — different samples/replicates at same age

**Example:**
```
Gene ID  | 17      | 18      | 21      | 24   | ...
ENSG0000001 | 6.55 | 6.72   | 7.10   | ...  | ...
ENSG000001  | 8.12 | 8.81   | 8.81   | ...  | ...
```

| Task | Details |
|------|---------|
| Detect layout | First column header = "Gene ID" (or similar); columns 2+ = ages (17, 18, 21, …) |
| Gene column | Column 0 = Ensembl IDs (ENSG… pattern) |
| Age columns | Columns 1..n = numeric headers (ages), cells = expression floats |
| Replicate handling | Same age in multiple columns = replicates; aggregate (mean) when needed |
| Data validation | Expression columns numeric; gene column string |
| Preview display | Show "Genes: N | Age columns: M (range 17–53)" in Data tab |

---

## Phase 3: Visualization Availability (Disable When Unfit)

**Principle:** Show all available visualizations; gray out / prevent selection when data doesn't fit.

| Visualization | Requirements | Disabled When |
|---------------|--------------|---------------|
| **Heatmap** | ≥2 numeric columns | No numeric columns, or <2 columns |
| **Correlation Matrix** | ≥2 numeric columns | Same as heatmap |
| **Histogram** | 1 numeric column | No numeric columns |
| **Box Plot** | 1 numeric column | No numeric columns |
| **Linear Regression** | 2 numeric columns (x, y) | <2 numeric columns |
| **Expression Trend** (new) | Gene × age layout | No age columns, or layout not detected |
| **Young vs Old Scatter** (new) | 2+ age groups | <2 age groups |

**Implementation:**
- Add `available_visualizations(df) -> Vec<(VizType, bool, Option<&str>)>` — returns (viz, available, reason_if_disabled)
- In Analysis tab: show full list with `[s] Summary` or `[s] Summary (disabled: no data)` 
- Key handlers: only trigger analysis when `available == true`
- In Visualizations tab: show same list with availability status

---

## Phase 4: Gene-Expression-Specific Features

| Task | Details |
|------|---------|
| **Expression Trend** | Line plot: expression vs age for selected gene(s). X = age category, Y = expression |
| **Young vs Old Scatter** | Scatter: mean expression Young vs Old across genes. Identifies aging markers |
| **Age Group Box Plot** | Box plot by age category (one box per age column) |
| **Gene selector** | When multiple genes: allow selection for trend plots (e.g. by Ensembl ID) |
| **Heatmap** | Gene × age heatmap. Rows = genes, columns = age. Color = expression |

---

## Phase 5: Rebrand & Help Text

| Task | Details |
|------|---------|
| App title | "Longevity Gene Expression" or "Gene Expression Analyzer" |
| Help text | Update for microarray workflow, remove AI |
| README | Update for longevity research use case |
| Data tab hints | "Load microarray data: genes (rows) × age (columns)" |

---

## Phase 6: Visualization List (Complete)

**Always show in UI:**

1. **s** — Summary statistics (numeric columns)
2. **c** — Correlation matrix / heatmap
3. **h** — Histogram (select column)
4. **b** — Box plot (select column)
5. **r** — Linear regression (select x, y)
6. **t** — Expression trend (gene × age) — *new, microarray-specific*
7. **v** — Young vs Old scatter — *new, microarray-specific*
8. **a** — Age group box plot — *new, by age category*

Each with availability check: `(available, reason)`.

---

## Summary of Files to Modify

| File | Changes |
|------|---------|
| `src/main.rs` | Remove api_key, simplify |
| `src/ui/components.rs` | Remove AI tab, AITab; update Tab enum |
| `src/ui/tui.rs` | Remove AI tab render, handlers; add viz availability |
| `src/ai/*` | Delete or stub out (remove AI agent) |
| `src/config/settings.rs` | Optional: remove api_key |
| `src/data/ingestion.rs` | Add layout detection |
| `src/viz/` | Add expression trend, young vs old scatter |
| `src/viz/types.rs` | Add new viz configs |
| `Cargo.toml` | Remove reqwest, dotenvy if unused |
| `README.md` | Rebrand for longevity |

---

## Estimated Effort

| Phase | Complexity |
|-------|------------|
| Phase 1 (Remove AI) | Low |
| Phase 2 (Microarray layout) | Medium |
| Phase 3 (Viz availability) | Medium |
| Phase 4 (Gene-expr viz) | Medium–High |
| Phase 5 (Rebrand) | Low |
| Phase 6 (Full viz list) | Part of 3–4 |

---

## Approval

Please review and confirm:
1. Phase order and scope
2. Data format assumptions (age as columns, genes as rows)
3. New visualizations (Expression Trend, Young vs Old Scatter, Age Group Box Plot)
4. Any other longevity-specific analyses to include
