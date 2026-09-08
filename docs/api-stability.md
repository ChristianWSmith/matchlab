# API Stability Policy

This document defines the stability guarantees for MatchLab's public APIs as of v1.0.

## Stable Public APIs

The following modules and types are considered stable public API. Breaking changes to these will require a semver major version bump.

### Core Types (`matchlab-core`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `player` | `PlayerId`, `SkillVector`, `SkillDimension`, `PlayerReality`, `PlayerObservation`, `Region`, `VisibleRank`, `GeoLocation` | Stable |
| `match_` | `MatchId`, `MatchResult`, `Team`, `MatchState` | Stable |
| `time` | `SimTime` | Stable |
| `rng` | `SimRng`, `StreamSeeds` | Stable |
| `world` | `World` | Stable |
| `event` | `Event`, `EventKind`, `EventEngine` | Stable |

### Experiment Infrastructure (`matchlab-experiments`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `config` | `ExperimentConfig`, `ExperimentSpec` | Stable |
| `identity` | `StudyId`, `ExperimentId`, `ReplicationId`, `RunId`, `RunMetadata`, `RunStatus` | Stable |
| `store` | `ExperimentStore`, `SqliteStore`, `StoreError` | Stable |
| `checkpoint` | `Checkpoint`, `CheckpointManager`, `FileCheckpointManager` | Stable |
| `design` | `ExperimentalDesign`, `PairingKey` | Stable |
| `factorial` | `FactorGrid`, `Condition`, `TypedFactor`, `DesignGenerator` | Stable |
| `runner` | `ExperimentResult`, `ExperimentRunner` | Stable |
| `seed` | `SeedManager`, `derive` | Stable |
| `parallel` | `WorkerJob`, `WorkerResult`, `Worker`, `WorkerPool` | Stable |
| `scheduler` | `ExecutionScheduler`, `SchedulerStatus` | Stable |
| `observation` | `ObservationPolicy`, `ObservationWriter` | Stable |
| `formats` | `ArtifactFormat`, `ExportConfig`, `VersionedArtifact` | Stable |
| `distributed` | `DistributedJob`, `DistributedResult`, `DistributedWorker`, `DistributedCoordinator` | Stable |
| `validation` | `DesignValidation`, `validate_design` | Stable |

### Analysis (`matchlab-analysis`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `effect` | `EffectSize`, `ConfidenceInterval`, `CiMethod`, `EffectSize`, `ArmStat` | Stable |
| `estimand` | `Estimand`, `EstimandDef`, `StatisticalEstimate` | Stable |
| `study` | `StudyStats`, `StudyReportConfig`, `ArmMetricStat`, `PairEffect` | Stable |
| `result` | `StatisticalResult`, `Uncertainty`, `EffectSizeSummary`, `SampleSummary`, `Provenance` | Stable |
| `power` | `PowerSpec`, `StudyPlan`, `PlanAssumptions` | Stable |
| `multiple_comparisons` | `Correction`, `holm`, `benjamini_hochberg` | Stable |
| `api` | `AnalysisAPI` | Stable |
| `dataframe` | `ResearchDataFrame`, `Column`, `DataType`, `DataValue` | Stable |
| `comparison` | `ComparisonEngine`, `ConditionComparison` | Stable |
| `visualization` | `PlotSpec`, `Mark`, `PlotRenderer` | Stable |
| `report_v2` | `ResearchReport`, `ReportProvenance` | Stable |
| `navigator` | `StudyNavigator`, `NavigationView` | Stable |
| `aggregation` | `ReplicationObservation`, `ReplicationTable` | Stable |
| `query` | `AnalysisQuery`, `QueryResult` | Stable |
| `factors` | `EffectTerm`, `EffectStructure` | Stable |
| `robustness` | `RobustnessCheck`, `RobustnessReport` | Stable |
| `incremental` | `IncrementalAggregator`, `WelfordMean`, `WelfordVariance` | Stable |
| `stability` | `StabilityMetrics`, `StabilityVerdict` | Stable |
| `hierarchy` | `ReplicationScalar`, `MetricObservation` | Stable |
| `cohort` | `CohortFilter`, `CohortResult` | Stable |
| `pareto` | `ParetoPoint`, `pareto_front` | Stable |
| `stats` | `Summary`, `summary` | Stable |

### Simulation (`matchlab-loop`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `ecosystem` | `EcosystemLoop`, `EcosystemTickResult` | Stable |
| `machine` | `LoopConfig`, `MachineState` | Stable |

### Matchmaking (`matchlab-matchmaking`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `matchmaker` | `Matchmaker`, `ProposedMatch` | Stable |
| `queue` | `Queue`, `QueueEntry` | Stable |
| `party` | `Party`, `PartyRegistry` | Stable |
| `latency` | `LatencyModel`, `LatencyMatrix` | Stable |
| `constraint` | `Constraint`, `HardConstraint` | Stable |
| `objective` | `MatchObjective`, `MatchObjectiveVector`, `ObjectiveWeights` | Stable |
| `policy` | `MatchPolicy` | Stable |
| `candidate` | `CandidateGenerator` | Stable |
| `selection` | `MatchSelectionPolicy` | Stable |

### Rating (`matchlab-rating`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `system` | `RatingSystem`, `RatingState` | Stable |
| `filter` | `filter_match_result` | Stable |

### Game (`matchlab-game`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `outcome` | `OutcomeModel` | Stable |
| `performance` | `PerformanceModel`, `GaussianNoiseModel`, `DeterministicModel` | Stable |
| `team` | `TeamModel`, `AdditiveTeamModel`, `WeightedTeamModel`, `ComplementaryTeamModel` | Stable |

### Players (`matchlab-players`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `archetype` | `ArchetypeConfig`, `DistributionConfig` | Stable |
| `distribution` | `SkillDistribution`, `Marginal` | Stable |
| `dynamics` | `SkillDynamics`, `LinearDynamics`, `ExperienceDynamics`, `DecayDynamics` | Stable |
| `population_dynamics` | `PopulationDynamics`, `PopulationEvent` | Stable |
| `population` | `PopulationConfig`, `PopulationGenerator` | Stable |

### Metrics (`matchlab-metrics`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `collector` | `MetricCollector`, `MetricResult` | Stable |
| `engine` | `MetricsEngine` | Stable |
| `incremental` | `IncrementalAggregator`, `WelfordMean`, `WelfordVariance` | Stable |
| `stability` | `StabilityMetrics`, `StabilityVerdict` | Stable |

### Detection (`matchlab-detection`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `detector` | `DetectionSystem`, `DetectionResult`, `EcosystemDetector` | Stable |
| `intervention` | `InterventionAction`, `EcosystemIntervention` | Stable |

### Adversarial (`matchlab-adversarial`)

| Module | Key Types | Status |
|--------|-----------|--------|
| `agent` | `StrategicAgent`, `AdversarialAgent`, `AgentObservations`, `AgentAction` | Stable |
| `objectives` | `PlayerObjective`, `WinRateObjective`, `RatingGainObjective`, etc. | Stable |
| `policy` | `AdaptivePolicy`, `LossAversionPolicy`, `ThresholdQueuePolicy`, etc. | Stable |
| `manipulation` | `ManipulationStrategy`, `RatingDumpStrategy`, etc. | Stable |

### CLI (`match-lab`)

| Command | Status |
|---------|--------|
| `matchlab run` | Stable |
| `matchlab study` | Stable |
| `matchlab compare` | Stable |
| `matchlab analyze` | Stable |
| `matchlab compare-stats` | Stable |
| `matchlab power` | Stable |

## Unstable APIs

The following are explicitly unstable and may change without notice:

- Internal implementation details in `*_lua.rs` files
- Test helpers
- Debug/logging internals
- `matchlab-validation` test-side references

## Breaking Change Policy

Breaking changes to stable APIs require:
1. Documentation in CHANGELOG.md
2. Deprecation period of at least one minor version
3. Migration guide for affected users

## Breaking Changes in v1.0.0

- `AnalysisAPI::from_dataframe()` renamed to `AnalysisAPI::new()` — the `from_*` pattern was inconsistent with the rest of the API where `new()` is the standard constructor.
