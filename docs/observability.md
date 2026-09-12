# Observability

matchlab provides structured logging for debugging, monitoring, and diagnostics during simulation runs. All diagnostic output goes to **stderr**, keeping stdout clean for program output (reports, JSON results, piped data).

## Log Levels

| Level | Purpose | When to use |
|---|---|---|
| `trace` | Extremely verbose, per-event detail | Debugging individual match simulations, Lua script internals |
| `debug` | Verbose operational detail | Diagnosing matchmaking quality, rating system behavior, metric computation |
| `info` | High-level progress and lifecycle events | Default for production runs; shows population generation, experiment start/complete, progress ticks |
| `warn` | Unexpected but non-fatal conditions | Queue stalls, missing optional subsystems, degraded configurations |
| `error` | Fatal or unrecoverable conditions | Failed script loads, invalid configs, metric registration failures |

## CLI Flags

### `--verbose`

Enables debug-level logging. Equivalent to `--log-level debug`.

```bash
matchlab run experiments/v0_1_basic.yaml --verbose
```

### `--log-level <LEVEL>`

Sets the minimum log level. Accepts: `trace`, `debug`, `info`, `warn`, `error`.

```bash
matchlab run experiments/v0_1_basic.yaml --log-level trace
```

### `--log-file <PATH>`

Writes logs to a file in addition to stdout. Creates the file if it does not exist; overwrites if it does.

```bash
matchlab run experiments/v0_1_basic.yaml --log-file /tmp/matchlab.log
```

Useful for long-running studies or CI pipelines where stdout may be truncated.

### `--json-logs`

Outputs logs as JSON Lines (one JSON object per line) instead of the human-readable default format. Suitable for log aggregation tools (e.g., Loki, Elasticsearch, CloudWatch).

```bash
matchlab run experiments/v0_1_basic.yaml --json-logs
```

The JSON output includes: timestamp, level, target (module path), message, and any structured fields.

### Combined Usage

```bash
# Debug-level, JSON output, written to file
matchlab run experiments/v0_1_basic.yaml \
    --log-level debug \
    --log-file results/debug.log \
    --json-logs

# Quiet mode (only warnings and errors)
matchlab run experiments/v0_1_basic.yaml --log-level error

# Verbose human-readable with file backup
matchlab run experiments/v0_1_basic.yaml \
    --verbose \
    --log-file results/run.log
```

## RUST_LOG Environment Variable

The `RUST_LOG` environment variable takes precedence over `--log-level` when set. It supports per-crate filtering using the standard `tracing_subscriber::EnvFilter` syntax.

### Examples

```bash
# Only matchlab-loop at info, everything else at warn
RUST_LOG="matchlab_loop=info,warn" matchlab run experiments/v0_1_basic.yaml

# Matchlab-rating at trace, everything else at info
RUST_LOG="matchlab_rating=trace,info" matchlab run experiments/v0_1_basic.yaml

# All matchlab crates at debug
RUST_LOG="matchlab_loop=debug,matchlab_rating=debug,matchlab_matchmaking=debug" \
    matchlab run experiments/v0_1_basic.yaml

# Lua VM internals at trace
RUST_LOG="mlua=trace,matchlab_loop=debug" \
    matchlab run experiments/v0_1_basic.yaml
```

### Per-Crate Module Names

| Crate | Module path |
|---|---|
| `matchlab-core` | `matchlab_core` |
| `matchlab-lua` | `matchlab_lua` |
| `matchlab-players` | `matchlab_players` |
| `matchlab-game` | `matchlab_game` |
| `matchlab-matchmaking` | `matchlab_matchmaking` |
| `matchlab-rating` | `matchlab_rating` |
| `matchlab-loop` | `matchlab_loop` |
| `matchlab-metrics` | `matchlab_metrics` |
| `matchlab-experiments` | `matchlab_experiments` |
| `matchlab-analysis` | `matchlab_analysis` |
| `matchlab-detection` | `matchlab_detection` |
| `matchlab-ranking` | `matchlab_ranking` |
| `matchlab-adversarial` | `matchlab_adversarial` |
| `matchlab-utility` | `matchlab_utility` |
| `matchlab-optimize` | `matchlab_optimize` |

## Debugging Guides

### Matchmaking Quality Issues

When matches seem imbalanced or queue times are unexpectedly high:

```bash
# Watch matchmaking decisions
RUST_LOG="matchlab_matchmaking=debug" matchlab run experiments/v0_1_basic.yaml --verbose

# See queue state and match formation
RUST_LOG="matchlab_matchmaking=trace,matchlab_loop=debug" matchlab run experiments/v0_1_basic.yaml
```

Key signals to look for:
- `match_quality` metric value in the output report
- Queue entry wait times before formation
- How many players are queued vs. how many form matches

### Rating Convergence Issues

When ratings are not converging or accuracy is poor:

```bash
# Track rating updates
RUST_LOG="matchlab_rating=debug" matchlab run experiments/v0_1_basic.yaml --verbose

# Full lifecycle: population → matches → rating updates
RUST_LOG="matchlab_rating=debug,matchlab_loop=debug,matchlab_game=debug" \
    matchlab run experiments/v0_1_basic.yaml
```

Key signals:
- `rating_accuracy` metric (MAE between rating and true skill)
- Rating update magnitudes after each match
- `convergence` metric showing the accuracy time series

### Simulation Problems

When the simulation produces unexpected results or errors:

```bash
# Full debug output
matchlab run experiments/v0_1_basic.yaml --verbose --log-file /tmp/debug.log

# Or with per-crate control
RUST_LOG="matchlab_loop=trace,matchlab_core=debug" \
    matchlab run experiments/v0_1_basic.yaml --log-file /tmp/trace.log
```

Key signals:
- `population generated` — confirms population size
- `experiment started` / `experiment completed` — lifecycle
- `progress` — periodic progress ticks (every 100 matches by default)
- Error messages with context (config validation, script loading failures)

## Progress Reporting

The simulation loop emits a `progress` log event at `info` level periodically (by default, every 100 matches). This includes the number of completed matches.

```json
{"timestamp":"2026-09-08T12:00:00Z","level":"INFO","fields":{"completed":500},"message":"progress"}
```

In human-readable format:
```
2026-09-08T12:00:00Z  INFO progress{completed=500}: matchlab_loop::machine
```

For long-running experiments, combine with `--log-file` to capture progress without cluttering the terminal. Note that diagnostic logs always go to stderr, so your stdout remains clean for program output.

## File Logging Setup for Long-Running Studies

For experiments that run for hours or days:

```bash
# Write structured JSON logs to a file, keep terminal quiet
matchlab run experiments/studies/elo_vs_glicko.yaml \
    --replicates 500 \
    --log-level info \
    --log-file results/study.log \
    --json-logs
```

The log file is line-delimited JSON, so it can be streamed or post-processed:

```bash
# Tail the log in real time
tail -f results/study.log | jq .

# Count progress ticks
grep '"progress"' results/study.log | jq .fields.completed

# Filter to warnings and errors only
cat results/study.log | jq 'select(.level == "WARN" or .level == "ERROR")'
```
