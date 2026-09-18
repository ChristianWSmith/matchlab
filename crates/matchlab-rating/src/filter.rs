//! Field-gating enforcement is handled by `_fair` serialization functions in
//! `matchlab-lua::convert`. Rating, matchmaking, detection, and adversarial
//! adapters pass `DataRequirements` to the `_fair` functions, which only
//! serialize fields the script declared interest in.
