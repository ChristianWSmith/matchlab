use matchlab_core::player::PlayerId;
use matchlab_core::world::World;

pub trait AdversarialAgent: Send + Sync {
    fn tick(&mut self, player_id: PlayerId, world: &mut World);
    fn objective(&self) -> AdversarialObjective;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdversarialObjective {
    MaximizeRating,
    MinimizeGamesPlayed,
    MaximizeWinRate { target_games: u64 },
    MaintainLowRating,
    WinTrade { partner: PlayerId },
    Derate,
}
