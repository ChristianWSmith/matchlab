# Documentation Architecture 

## Recommended Structure

```
README.md                    # Concise overview, getting started
AGENTS.md                    # Agent orientation (internal)
docs/
├── api-stability.md         # API stability policy
├── api-consistency-audit.md # API consistency findings
├── validation-audit.md      # Validation coverage matrix
├── methodology/             # Statistical methods, assumptions, limitations
│   └── (future)
└── examples/                # Working example experiments (future)
```

## Current State

- `README.md`: Comprehensive overview with examples, architecture, CLI usage
- `AGENTS.md`: Internal agent orientation document
- `docs/api-stability.md`: v1.0 API stability policy
- `docs/api-consistency-audit.md`: API consistency audit findings
- `docs/validation-audit.md`: Validation coverage matrix

## Principles

1. **README is concise**: explains why MatchLab exists and how to get started, not the entire platform
2. **AGENTS.md is internal**: agent orientation for code agents, not user-facing
3. **API stability is documented**: which APIs are stable, which are internal
4. **Validation coverage is visible**: researchers can see what's tested and what's not
5. **Methodology is explicit**: assumptions, limitations, and statistical methods are documented separately

## What's Missing (Future Work)

- `docs/methodology/`: Detailed statistical methods, assumptions, limitations
- `docs/examples/`: Working example experiments with expected results
- `CHANGELOG.md`: Version history (not yet created for v1.0)
- `CONTRIBUTING.md`: Contribution guidelines (not yet created)
- `SECURITY.md`: Security policy (not yet created)
