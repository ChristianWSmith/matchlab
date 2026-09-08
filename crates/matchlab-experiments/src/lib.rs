//! matchlab-experiments: runner, YAML config, inheritance, factorial design,
//! replication.
pub mod checkpoint;
pub mod config;
pub mod counterfactual;
pub mod design;
pub mod distributed;
pub mod factorial;
pub mod formats;
pub mod identity;
pub mod inherit;
pub mod observation;
pub mod package;
pub mod parallel;
pub mod replicate;
pub mod runner;
pub mod scheduler;
pub mod seed;
pub mod store;
pub mod study;
pub mod validation;
pub use config::ExperimentConfig;
pub use counterfactual::{
    CounterfactualComparison, GameHistory, MatchObjectiveVector, REPLAY_VALID_METRICS,
    ReplayEngine, StrategicCounterfactual, StrategicMode, compare_counterfactuals,
    counterfactual_eval,
};
pub use design::{ExperimentalDesign, PairingKey};
pub use factorial::{
    Condition, DesignGenerator, Factor, FactorGrid, FactorialDesign, FractionalFactorial,
    ReplicationAssignment, TypedFactor, set_nested_value,
};
pub use identity::{
    ExperimentId, ExperimentMetadata, ReplicationId, ReplicationMetadata, RunId, RunMetadata,
    RunStatus, StudyId, StudyMetadata,
};
pub use replicate::{
    ArmConfig, ArmResult, DesignType, ExtendedStudyResult, HierarchyNode, HierarchyView,
    ReplicateResult, ReplicationRunner, ReplicationSpec, SeedStrategy, StudyExtension, StudyResult,
};
pub use runner::{ExperimentResult, ExperimentRunner};
pub use seed::SeedManager;
pub use store::{ExperimentStore, SqliteStore, StoreError};
pub use study::{ArmSpec, StudyConfig, StudyOutputSpec, StudyRunner, StudySpec};
