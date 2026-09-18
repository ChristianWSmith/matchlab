use crate::data_requirements::DataRequirements;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct GlobalSubscription {
    pub observation_fields: HashSet<String>,
    pub match_result_fields: HashSet<String>,
    pub queue_fields: HashSet<String>,
    pub snapshot_fields: HashSet<String>,
    pub population_fields: HashSet<String>,
    pub reality_fields: HashSet<String>,
    pub behavior_fields: HashSet<String>,
    pub completed_match_fields: HashSet<String>,
}

impl GlobalSubscription {
    pub fn aggregate(reqs: &[&DataRequirements]) -> Self {
        let mut sub = Self::default();
        for req in reqs {
            sub.observation_fields
                .extend(req.observation_fields.iter().cloned());
            sub.match_result_fields
                .extend(req.match_result_fields.iter().cloned());
            sub.queue_fields.extend(req.queue_fields.iter().cloned());
            sub.snapshot_fields
                .extend(req.snapshot_fields.iter().cloned());
            sub.population_fields
                .extend(req.population_fields.iter().cloned());
            sub.reality_fields
                .extend(req.reality_fields.iter().cloned());
            sub.behavior_fields
                .extend(req.behavior_fields.iter().cloned());
            sub.completed_match_fields
                .extend(req.completed_match_fields.iter().cloned());
        }
        sub
    }

    pub fn needs_observation_field(&self, field: &str) -> bool {
        self.observation_fields.contains(field)
    }

    pub fn needs_match_result_field(&self, field: &str) -> bool {
        self.match_result_fields.contains(field)
    }

    pub fn needs_skill_in_observations(&self) -> bool {
        self.observation_fields.contains("skill_overall")
            || self.observation_fields.contains("skill_vector")
    }

    pub fn needs_population(&self) -> bool {
        !self.population_fields.is_empty()
    }

    pub fn needs_completed_matches(&self) -> bool {
        !self.completed_match_fields.is_empty()
    }

    pub fn needs_behavior_field(&self, field: &str) -> bool {
        self.behavior_fields.contains(field)
    }
}
