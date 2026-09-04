# Ticket 01 — Workspace Foundation

## Goal

Turn the single-package stub into the matchlab Cargo workspace and add the
scaffolding needed for the v0.1 build: workspace dependency declarations, a
placeholder binary, experimental directory layout, and a CI lint/test gate.

## Scope / Deliverables

- Restructure root `Cargo.toml` into a workspace with:
  - `workspace.members = ["crates/*"]`
  - `[workspace.dependencies]` for `serde` (derive), `serde_yaml = "0.9"`,
    `rand = "0.8"`, `rand_chacha = "0.3"` (spec §3.2)
  - `[workspace.package]` defaults for `edition = "2024"` shared by all crates
- Keep/adapt `src/main.rs` as the binary crate (the `match-lab` package) that
  depends on `matchlab-experiments` and `matchlab-analysis`.
- Create empty placeholder directories under `crates/`:
  - `matchlab-core`, `matchlab-players`, `matchlab-game`, `matchlab-matchmaking`,
    `matchlab-rating`, `matchlab-metrics`, `matchlab-experiments`,
    `matchlab-analysis`
  - (Other crates from spec §3 — detection, ranking, objective, adversarial,
    utility — are **out of scope** for v0.1 and should NOT be created yet.)
- Create the `experiments/` directory layout (spec §3/§17):
  - `experiments/base/` (for inherited base configs)
  - placeholder for `experiments/v0_1_basic.yaml` (filled in Ticket 10)
- Add a CI workflow (`.github/workflows/ci.yml` or equivalent) that runs
  `cargo build` and `cargo test` on every push/PR.
- Add `.gitignore` entries for `results/` output directories.

## Acceptance criteria

- [ ] `cargo build` from the workspace root succeeds.
- [ ] `cargo test` passes (placeholder tests only, if any).
- [ ] Workspace members resolve and `cargo metadata` lists the intended crates.
- [ ] `serde`/`serde_yaml`/`rand`/`rand_chacha` are declared once in
      `[workspace.dependencies]`.
- [ ] CI runs build + test on push/PR.

## Testing

- `cargo build` and `cargo test` green.
- `cargo check --workspace` green.

## Dependencies

None — this is the foundation.

## Notes

- Do **not** create crates that are not part of v0.1 (no
  matchlab-detection/ranking/objective/adversarial/utility yet).
- Keep namespace/crate-name conventions from AGENTS.md (`matchlab-{domain}`).
- The `match-lab` binary package should not be a library crate; the crates
  under `crates/` carry the libraries.
- This ticket may leave crates as empty stubs that fail to compile (empty libs
  are fine); subsequent tickets fill them in. Prefer adding a minimal empty
  `lib.rs` for each crate so the workspace compiles.
