//! Party as a first-class queue entity.
//!
//! A party is the unit that enters the queue, waits, and is matched. A solo
//! player is simply a party of size one.
use matchlab_core::player::PlayerId;
use matchlab_core::time::SimTime;
/// A party in the matchmaking queue.
#[derive(Debug, Clone)]
pub struct Party {
    /// Unique party identifier.
    pub id: u64,
    /// Members of the party (at least one).
    pub members: Vec<PlayerId>,
    /// When the party was formed.
    pub created_at: SimTime,
    /// Maximum party size (None = no limit beyond formation).
    pub party_size_limit: Option<usize>,
}
impl Party {
    /// Create a solo party (single player).
    pub fn solo(player_id: PlayerId, created_at: SimTime) -> Self {
        Self {
            id: player_id.0,
            members: vec![player_id],
            created_at,
            party_size_limit: None,
        }
    }
    /// Create a party with multiple players.
    pub fn new(
        id: u64,
        members: Vec<PlayerId>,
        created_at: SimTime,
        party_size_limit: Option<usize>,
    ) -> Result<Self, String> {
        if members.is_empty() {
            return Err("party must have at least one member".to_string());
        }
        if let Some(limit) = party_size_limit {
            if members.len() > limit {
                return Err(format!(
                    "party has {} members, exceeding limit of {}",
                    members.len(),
                    limit
                ));
            }
        }
        Ok(Self {
            id,
            members,
            created_at,
            party_size_limit,
        })
    }
    /// Number of members in the party.
    pub fn size(&self) -> usize {
        self.members.len()
    }
    /// Whether the party contains a specific player.
    pub fn contains(&self, player_id: PlayerId) -> bool {
        self.members.contains(&player_id)
    }
    /// Whether this is a solo party.
    pub fn is_solo(&self) -> bool {
        self.members.len() == 1
    }
}
/// Registry of active parties .
#[derive(Debug, Default)]
pub struct PartyRegistry {
    parties: std::collections::HashMap<u64, Party>,
    next_id: u64,
}
impl PartyRegistry {
    /// Register a new party and return its ID.
    pub fn register(&mut self, party: Party) -> u64 {
        let id = party.id;
        self.parties.insert(id, party);
        id
    }
    /// Get a party by ID.
    pub fn get(&self, party_id: u64) -> Option<&Party> {
        self.parties.get(&party_id)
    }
    /// Get a mutable reference to a party by ID.
    pub fn get_mut(&mut self, party_id: u64) -> Option<&mut Party> {
        self.parties.get_mut(&party_id)
    }
    /// Remove a party from the registry.
    pub fn remove(&mut self, party_id: u64) -> Option<Party> {
        self.parties.remove(&party_id)
    }
    /// Number of active parties.
    pub fn len(&self) -> usize {
        self.parties.len()
    }
    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.parties.is_empty()
    }
    /// Iterate over all parties.
    pub fn parties(&self) -> impl Iterator<Item = &Party> {
        self.parties.values()
    }
    /// Generate the next party ID.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
/// Check whether a proposed match violates party integrity: all members of
/// any party in the match must be in the same match.
pub fn check_party_integrity(
    team_a: &[PlayerId],
    team_b: &[PlayerId],
    party_registry: &PartyRegistry,
) -> Result<(), String> {
    let all_players: Vec<PlayerId> = team_a.iter().chain(team_b).copied().collect();
    for party in party_registry.parties() {
        let party_in_match: Vec<&PlayerId> = party
            .members
            .iter()
            .filter(|m| all_players.contains(m))
            .collect();
        if !party_in_match.is_empty() && party_in_match.len() < party.members.len() {
            return Err(format!(
                "party {} has members not in the match (party integrity violated)",
                party.id
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solo_party_has_one_member() {
        let p = Party::solo(PlayerId(1), SimTime::ZERO);
        assert_eq!(p.size(), 1);
        assert!(p.is_solo());
        assert!(p.contains(PlayerId(1)));
        assert!(!p.contains(PlayerId(2)));
    }
    #[test]
    fn multi_player_party() {
        let p = Party::new(
            42,
            vec![PlayerId(1), PlayerId(2), PlayerId(3)],
            SimTime::ZERO,
            None,
        )
        .unwrap();
        assert_eq!(p.size(), 3);
        assert!(!p.is_solo());
        assert!(p.contains(PlayerId(2)));
    }
    #[test]
    fn party_size_limit_enforced() {
        let p = Party::new(
            1,
            vec![PlayerId(1), PlayerId(2), PlayerId(3)],
            SimTime::ZERO,
            Some(2),
        );
        assert!(p.is_err());
    }
    #[test]
    fn empty_party_rejected() {
        let p = Party::new(1, vec![], SimTime::ZERO, None);
        assert!(p.is_err());
    }
    #[test]
    fn party_registry_register_and_get() {
        let mut reg = PartyRegistry::default();
        let p = Party::solo(PlayerId(1), SimTime::ZERO);
        let id = reg.register(p);
        assert_eq!(id, 1);
        assert!(reg.get(1).is_some());
        assert!(reg.get(99).is_none());
        assert_eq!(reg.len(), 1);
    }
    #[test]
    fn party_registry_remove() {
        let mut reg = PartyRegistry::default();
        reg.register(Party::solo(PlayerId(1), SimTime::ZERO));
        let removed = reg.remove(1);
        assert!(removed.is_some());
        assert!(reg.is_empty());
    }
    #[test]
    fn party_integrity_check_passes() {
        let mut reg = PartyRegistry::default();
        reg.register(Party::new(1, vec![PlayerId(1), PlayerId(2)], SimTime::ZERO, None).unwrap());
        let result = check_party_integrity(&[PlayerId(1), PlayerId(2)], &[PlayerId(3)], &reg);
        assert!(result.is_ok());
    }
    #[test]
    fn party_integrity_check_fails_on_split() {
        let mut reg = PartyRegistry::default();
        reg.register(Party::new(1, vec![PlayerId(1), PlayerId(2)], SimTime::ZERO, None).unwrap());
        let result = check_party_integrity(&[PlayerId(1)], &[PlayerId(3)], &reg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("party integrity"));
    }
    #[test]
    fn party_integrity_ignores_solos() {
        let reg = PartyRegistry::default();
        let result = check_party_integrity(&[PlayerId(1), PlayerId(2)], &[PlayerId(3)], &reg);
        assert!(result.is_ok());
    }
}
