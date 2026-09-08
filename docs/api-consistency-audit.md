# API Consistency Audit 

This document records inconsistencies found in the public API and recommended fixes.

## Constructor Naming Patterns

| Type | Current Pattern | Recommended | Notes |
|------|----------------|-------------|-------|
| `ConfidenceInterval` | `new()` | `new()` | Consistent ✓ |
| `AnalysisAPI` | `from_dataframe()` | `new()` | Inconsistent — `from_*` should be reserved for conversions |
| `ComparisonEngine` | `new()` | `new()` | Consistent ✓ |
| `ResearchDataFrame` | `new()` | `new()` | Consistent ✓ |
| `StudyNavigator` | `new()` | `new()` | Consistent ✓ |
| `AnalysisQuery` | `new()` + builder methods | `new()` | Consistent ✓ |
| `RawDataExporter` | `new()` | `new()` | Consistent ✓ |
| `SqliteStore` | `open()` / `in_memory()` | Keep as-is | Different semantics (file vs memory) |
| `FileCheckpointManager` | `new()` | `new()` | Consistent ✓ |
| `FileObservationWriter` | `new()` | `new()` | Consistent ✓ |
| `WorkerPool` | `new()` | `new()` | Consistent ✓ |
| `ExecutionScheduler` | `new()` | `new()` | Consistent ✓ |
| `DesignGenerator` | `balanced()` / `randomized()` | Keep | Static methods are fine |
| `ParetoExplorer` | `from_comparison_results()` | `from_comparison_results()` | Consistent ✓ |
| `ReportGenerator` | `generate()` (no new) | N/A | Method, not constructor |

## Issues Found

1. **`AnalysisAPI::from_dataframe()`** — FIXED in v1.0.0: renamed to `AnalysisAPI::new(data)` for consistency with other constructors.

2. **`StudyResult` builder pattern** — `RunMetadata` uses a builder pattern while `StudyResult` uses direct construction. This is intentional (RunMetadata has many optional fields) but should be documented.

3. **`EffectSize::new()` takes 6 positional arguments** — this is borderline too many. Consider a builder or struct-update syntax for clarity.

## Recommended Actions

| Priority | Action | Effort |
|----------|--------|--------|
| Low | Rename `AnalysisAPI::from_dataframe` → `AnalysisAPI::new` | Minimal |
| Low | Add `#[doc]` to document builder vs. direct construction patterns | Minimal |
| Low | Consider builder for `EffectSize::new` in future | None for now |

## Summary

The API is largely consistent. The main inconsistency (`from_dataframe` vs `new`) is minor and doesn't affect usability. The codebase uses `new()` consistently for simple constructors and `from_*()` for conversions. No breaking changes needed.
