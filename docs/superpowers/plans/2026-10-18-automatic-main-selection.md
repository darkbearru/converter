# Automatic Main Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for the main integration; independently delegated parser/backend tasks use the agreed file boundaries. Steps use checkbox syntax.

**Goal:** Convert the primary dictionary automatically and offer unrelated dictionaries as separate jobs while accepting recoverable legacy DSL formatting.

**Architecture:** Optional reader analysis supplies a conservative dependency graph. The job manager persists a separate offer and idempotent child-job plan while the main conversion proceeds. Vue presents a standard PrimeVue Dialog.

**Tech Stack:** Rust, Tauri 2, Vue 3, TypeScript, PrimeVue Aura.

**Spec:** `docs/superpowers/specs/2026-10-18-automatic-main-selection-design.md`.

## Global Constraints

- Preserve source files, existing history and output files; no user corpus in Git.
- URL is archive or single dictionary; no neighbor discovery.
- Only primary automatic; unrelated bases require explicit agreement; no merging.
- Existing 740×500 /540×400 system-theme UI, native runtime without PHP/Python.
- No mobile SDK, push or Windows native acceptance claim.

## Review Focus

- Missing/ambiguous referenced names must retain manual selection.
- Escaped/commented references must not select a primary.
- Repeated acceptance and interrupted persistence must not duplicate child jobs.
- A completed parent must still permit the pending offer to be resolved.
- Independent children must not inherit the primary's result, journal or title override.

### Task 1: Compatible DSL formatting

**Files:** `crates/converter-core/src/dsl/{markup,renderer}.rs`, converter CSS, rendering tests.

- [x] Reproduce crossing i/lang and p/trn failures with synthetic RED tests.
- [ ] Normalize only recoverable formatting, preserve abbreviation/reference/resource semantics and report repairs.
- [ ] Verify complete archive reads and limits/text preservation; run `cargo test -p converter-core --test dsl_rendering`.

### Task 2: Conservative source graph

**Files:** `formats/reader.rs`, `dsl/relationships.rs`, `dsl/mod.rs`, `prepare/selection.rs`, selection tests.

- [ ] Add failing tests for primary+index+independent, ambiguous names, cycles, missing targets and escaped references.
- [ ] Implement the interfaces from the spec; a single acyclic reference sink is primary, isolated nodes are independent.
- [ ] Run focused core tests; verify the real archive selects Universal and offers only PhraseBook.

### Task 3: Durable offer and child jobs

**Files:** converter-app contracts, store, manager/package module, lifecycle tests; Tauri command wiring.

- [ ] Write RED workflow tests, including repeat/decline/restart and accepting after success.
- [ ] Persist selected primary and optional offer; implement `resolve_additional` with predetermined child IDs and recovery.
- [ ] Reset child lifecycle/result/diagnostic/metadata state; inherit source resources, output and encoding.
- [ ] Run converter-app tests and regenerate contract fixtures.

### Task 4: Optional Dialog

**Files:** Vue contracts/native bridge, job store, additional-dictionaries Dialog, App.vue, UI tests/captures.

- [ ] Write failing tests for offered independent bases and decision dispatch.
- [ ] Present the first pending offer, disable duplicate submission, persist decline on close/Escape, show recoverable command errors.
- [ ] Verify accepted children appear as separate tasks, repeated events do not reopen a resolved offer, both themes/sizes fit.

### Task 5: Integration and delivery

- [ ] Run workspace tests/clippy/contracts and Vue/browser checks appropriate to changed paths.
- [ ] Convert the real archive in isolated temporary storage, inspect primary/child outputs and report actual warnings.
- [ ] Obtain code/UI review, reconcile documentation, build macOS APP, verify the native flow without altering old user jobs.
- [ ] Commit the reviewed changes locally and report evidence and remaining boundaries.
