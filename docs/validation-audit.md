# Validation Audit Matrix

This document records the validation coverage of every major subsystem as of v1.0.

## Legend

- ✓ = covered
- Partial = partially covered
- (future) = planned but not yet implemented

## Validation Matrix

| Subsystem | Unit Tests | Property Tests | Known Answer | Integration | E2E |
|-----------|:----------:|:--------------:|:------------:|:-----------:|:---:|
| **Configuration** | ✓ | | ✓ | ✓ | |
| **Population** | ✓ | | ✓ | ✓ | |
| **Skill** | ✓ | ✓ | ✓ | ✓ | |
| **Performance** | ✓ | | ✓ | ✓ | |
| **Game/Outcome** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Rating** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Matchmaking** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Strategy** | ✓ | | ✓ | ✓ | |
| **Detection** | ✓ | | ✓ | ✓ | |
| **Ecosystem** | ✓ | | | ✓ | ✓ |
| **Execution** | ✓ | | ✓ | ✓ | ✓ |
| **Persistence** | ✓ | ✓ | ✓ | ✓ | |
| **Analysis** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Visualization** | ✓ | | | ✓ | ✓ |
| **Reporting** | ✓ | | | ✓ | ✓ |
| **Plugins (Lua)** | ✓ | | ✓ | ✓ | ✓ |
| **CLI** | ✓ | | ✓ | ✓ | ✓ |

## Subsystem Details

### Configuration
- Unit: serde deserialization, validation, inheritance
- Known-answer: specific manifests parse correctly
- Integration: full experiment config round-trips

### Population
- Unit: archetype sampling, proportion allocation
- Known-answer: known distributions produce expected means/variances
- Integration: population feeds into simulation correctly

### Skill
- Unit: SkillVector operations, dynamics advance
- Property: normalization round-trips, bounds respected
- Known-answer: linear dynamics reproduce current behavior
- Integration: skill feeds into performance model

### Performance
- Unit: Gaussian noise model, deterministic model
- Known-answer: zero noise → deterministic, nonzero noise → variance
- Integration: performance feeds into outcome model

### Game/Outcome
- Unit: logistic model, variance model, composition model
- Property: probabilities in [0,1], sum to 1
- Known-answer: logistic win probability matches formula
- Integration: outcome feeds into rating system
- E2E: full simulation produces expected match results

### Rating
- Unit: Elo, Glicko-2, TrueSkill, flat
- Property: rating updates bounded, zero-info outcomes don't create info
- Known-answer: Elo update matches formula, Glicko-2 matches paper
- Integration: rating system updates from match results
- E2E: rating convergence over simulation

### Matchmaking
- Unit: batch, expanding_window, strict, hub_spoke matchmakers
- Property: hard constraints never violated, selected matches are valid candidates
- Known-answer: batch produces balanced teams
- Integration: matchmaker feeds into game model
- E2E: full matchmaking pipeline produces expected queue times

### Strategy
- Unit: strategic agent actions, manipulation strategies
- Known-answer: honest agents don't manipulate
- Integration: strategies affect matchmaking outcomes

### Detection
- Unit: detection system observe/evaluate/recommend
- Known-answer: perfect detection catches all anomalies
- Integration: detection affects agent behavior

### Ecosystem
- Unit: population dynamics, entry/exit
- Integration: ecosystem loop connects matchmaking to population
- E2E: full ecosystem simulation runs

### Execution
- Unit: scheduler, parallel executor, worker pool
- Known-answer: deterministic execution produces identical results
- Integration: scheduler manages job lifecycle
- E2E: large studies execute correctly

### Persistence
- Unit: SQLite store, checkpoint manager, observation writer
- Property: serialization round-trips, checkpoint restores correctly
- Known-answer: stored results match fresh computation

### Analysis
- Unit: effect sizes, CIs, p-values, power, comparisons
- Property: CIs contain true value at nominal rate, p-values uniform under null
- Known-answer: t-test matches formula, bootstrap coverage correct
- Integration: analysis works on stored results
- E2E: full analysis pipeline produces expected outputs

### Visualization
- Unit: plot spec generation, text/json rendering
- Integration: plots render from analysis results
- E2E: plots are generated for canonical studies

### Reporting
- Unit: report generation, markdown rendering
- Integration: reports compile from analysis results
- E2E: reports are generated for canonical studies

### Plugins (Lua)
- Unit: plugin loading, Lua VM, RNG
- Known-answer: shipped plugins produce expected results
- Integration: plugins integrate with Rust systems
- E2E: plugin-based experiments run correctly

### CLI
- Unit: argument parsing, command dispatch
- Known-answer: commands produce expected output
- Integration: CLI drives full pipeline
- E2E: end-to-end CLI workflows

## Coverage Gaps Identified

| Gap | Severity | Priority |
|-----|----------|----------|
| Property tests for game models | Medium | |
| Property tests for rating updates | Medium | |
| Property tests for matchmaking | Medium | |
| Cross-implementation validation for Elo/Glicko | High | |
| Determinism torture test | Medium | |
| Memory leak detection | Medium | |
| Performance regression suite | Medium | |
