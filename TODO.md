**TL;DR:** Pure static analysis using deterministic AST rules and statistical algorithms to produce **Impact Packs**, **Clone Packs**, **Branch/File-Split Packs**, and **Code Quality Packs**. No AI/ML dependencies; only analyzers and concrete refactoring plans.

### Guardrails (non-negotiables)

* **No AI/ML dependencies.** Only: (a) static program facts, (b) deterministic heuristics, (c) statistical analysis algorithms.
* **CPU-first:** works on dev laptops/CI; no GPU required.
* **Offline:** models vendored/cached locally; analysis doesn’t phone home.
* **Deterministic outputs:** same repo + config ⇒ same packs.

---

## 1) Model choices (small, fast, permissive)

* **Statistical algorithms:** Pattern matching, frequency analysis, and rule-based heuristics
* **No external models:** All analysis runs using deterministic algorithms and statistical analysis
* **Lightweight:** Pure Rust implementation with minimal dependencies

---

## 2) What we ship (LLM-free features)

### A) **Impact Packs (cycle/chokepoint)** — already analytic

* Keep the greedy feedback-vertex-set approximation on the **import graph**.
* Output: minimal node set to break SCCs; chokepoint modules by high approximate betweenness + cross-community edges.
* Config knobs: `max_packs`, `centrality_samples`, `non_overlap`.

### B) **Clone Consolidation (Clone Packs)** — uses your `echo` lib

* Medoid snippet + parameter/delta extraction via **CST token diff** (tree-sitter/libcst).
* Deterministic name proposal for the extracted template: verb from effect rules; noun from return/primary type.
* No code edits; just a plan with parameters, optional blocks, and callsite count.

### C) **Filesystem Structure Packs** — branch + file-split

* **Branch Reorg:** directory imbalance score (files/subdirs/LOC dispersion + entropy) → propose 2–4 subdirs via min-cut on the directory’s internal import subgraph; fallback to size-balanced clusters.
* **File-Split:** huge file thresholds (LOC/bytes) + community detection over intra-file entity cohesion graph → 2–3 split candidates.
* Deterministic steps list; import-update counts estimated via xrefs.

### D) **Code Quality Analyzer** — pattern-based analysis (no lint style)

* **Behavior signature (static):** side-effects (I/O/DB/net), mutation, async/promise, return kind (scalar/Optional/iter), resource handles.
* **Pattern matching:** Rule-based analysis of naming conventions and code patterns.
* **Rule checks:** effect verbs (`get/is/set/create…`) vs observed effects; cardinality (`user` vs `users`), optionality (`find/try` vs non-Optional), async (`fetchAsync` vs blocking), type roles (`*Map/*Set/*Id/*Count`).
* **Consistency analysis:** build project lexicon (top nouns from types/directories/schemas); flag naming inconsistencies via frequency analysis.
* **Packs:**

  * **Rename Pack:** top-2 deterministic names (verb+noun+qualifiers) + impact (external callsites) + reason strings.
  * **Contract Mismatch Pack:** if name implies API properties (e.g., `try_get`/`*_iter`) that the function *doesn’t* provide; suggest either (rename) or (minimal contract change), but **never generate code**.

---

## 3) Deterministic proposal generators (no AI beyond embeddings)

### 3.1 Verb selection (functions)

Map observed primary effect → verb. Examples (overrideable in YAML):

```
http_get → fetch/get
db_read → get/find
db_write → create/insert/update/upsert/delete
parse → parse/deserialize
format → format/serialize
validate → validate/check
cache_lookup → get_cached
iterator → iter/list
```

### 3.2 Noun selection

Prefer, in order: (1) returned type head (e.g., `User`, `Config`); (2) dominant parameter/object touched; (3) directory/domain term.
Pluralize if return is collection/iterator; add qualifiers from distinctive params: `by_id`, `from_path`, `with_timeout`.

### 3.3 Name construction

Apply language style (snake/camel/Pascal). Expand abbreviations via `abbrev_map` and allowlist `allowed_abbrevs`.
If candidate > 40 chars, compress qualifiers (`by_id` → `by_id` stays; drop extra adjectives).

---

## 4) Scoring & gating (noise control)

* **Mismatch score** (0–1): `0.5*(1 - cosine) + 0.2*effect + 0.1*cardinality + 0.1*optional + 0.1*async_or_idempotence`.
* **Confidence dampers:** −0.15 if behavior inference weak (dynamic calls), −0.1 if name has <2 tokens.
* **Fire thresholds (defaults):** `names.min_mismatch=0.65`, `names.min_impact=3` external refs.
* **Pack ranking:** `priority = value / (effort+ε)` where

  * Rename value = `mismatch * log1p(external_refs)`; effort = `refs_to_update`.
  * Contract value = mismatch + optionality/cardinality penalty; effort = `public_API? × 2 + refs_to_update`.

---

## 5) Performance profile (typical monorepo)

* **Parsing/graphs:** unchanged from your current pipeline.
* **Embeddings:** batch 512–2048 texts per pass; e5-small on CPU ≈ 5k–20k items/minute depending on host; cache across runs.
* **Memory:** ≤1 GB typical (models + batches).
* **Runtime overhead:** +10–25% over your current static pipeline for name analysis; other packs are graph-level and near-linear.

---

## 6) Config (only semantic & structure dials)

```yaml
names:
  enabled: true
  pattern_analysis: true           # rule-based analysis
  min_mismatch: 0.65
  min_impact: 3
  protect_public_api: true
  abbrev_map: { usr: user, cfg: config, btn: button }
  allowed_abbrevs: ["id","url","db","io","api"]
  io_libs:
    python: ["requests","aiohttp","sqlalchemy","boto3","os","pathlib"]
    typescript: ["node:fs","fs","axios","fetch","pg","mongodb"]
    rust: ["reqwest","tokio::fs","sqlx","rusqlite"]
fsdir:
  max_files_per_dir: 25
  max_subdirs_per_dir: 10
  max_dir_loc: 2000
  min_branch_recommendation_gain: 0.15
fsfile:
  huge_loc: 800
  huge_bytes: 128000
impact_packs:
  enable_cycle_packs: true
  enable_chokepoint_packs: true
  non_overlap: true
  max_packs: 20
```

---

## 7) Deliverables to add

* `names/` module (gloss, behavior\_sig, embed backend, score, pack builder).
* `structure/` already added; include ranking glue and summary tables.
* **FastAPI/MCP**: extend with

  * `GET /results/{id}/rename_packs`
  * `GET /results/{id}/contract_mismatch_packs`
  * `GET /results/{id}/impact_packs` (now returns cycle/chokepoint/branch/file-split/clone)

---

## 8) Test plan (LLM-free)

* **Unit:** verb/noun inference tables; abbreviation expansion; effect detectors per language (mock call graph + known imports).
* **Golden:** tiny repos:

  * `get_user()` mutates DB → expect `EffectMismatch` + rename to `update_user`/`upsert_user`.
  * `find_user()` returns `User` (non-Optional) → `OptionalityMismatch`.
  * `users()` returns iterator → `CardinalityMismatch` with `iter_users`.
  * Overcrowded dir → Branch Pack with ≥0.15 imbalance gain.
  * 1.6k-LOC file with 3 cohesion communities → File-Split with 2–3 suggestions.
* **Perf:** benchmark analysis throughput; assert cached re-run <30% of cold time.

---

## 9) Roadmap (still LLM-free)

* **Dry-run rename feasibility:** static xref to list all affected imports/exports; no edits—just a “blast radius” report.
* **Barrel/alias helpers:** language-specific alias plans (TS barrels, Python re-exports) to soften public renames.
* **Vocab policy mode:** optional allow/deny head verbs/nouns enforced across project (pattern-based, not lint style).

