/// Monotonic simulation clock, nanosecond resolution internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimTime(pub u64);

impl SimTime {
    pub const ZERO: Self = Self(0);

    pub fn from_secs(secs: f64) -> Self {
        Self((secs * 1_000_000_000.0) as u64)
    }

    pub fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000_000)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1_000_000_000.0
    }

    pub fn duration_since(self, earlier: SimTime) -> SimTime {
        SimTime(self.0.saturating_sub(earlier.0))
    }

    /// Raw internal value (nanoseconds). Useful as a monotonic tick counter.
    pub fn ticks(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_secs_round_trips() {
        let t = SimTime::from_secs(3.5);
        assert_eq!(t.0, 3_500_000_000);
        assert!((t.as_secs_f64() - 3.5).abs() < 1e-6);
    }

    #[test]
    fn from_millis_matches_seconds() {
        let from_ms = SimTime::from_millis(2000);
        let from_secs = SimTime::from_secs(2.0);
        assert_eq!(from_ms, from_secs);
    }

    #[test]
    fn ticks_returns_raw_nanos() {
        assert_eq!(SimTime::from_secs(1.0).ticks(), 1_000_000_000);
        assert_eq!(SimTime::ZERO.ticks(), 0);
    }

    #[test]
    fn duration_since_orders_correctly() {
        let later = SimTime::from_secs(10.0);
        let earlier = SimTime::from_secs(4.0);
        assert_eq!(later.duration_since(earlier), SimTime::from_secs(6.0));
        // Reversed order never panics (saturating).
        assert_eq!(earlier.duration_since(later), SimTime::ZERO);
    }

    #[test]
    fn duration_since_saturates_not_wraps() {
        // u64::MAX - u64::MIN style subtraction must not wrap around.
        let big = SimTime(u64::MAX);
        let small = SimTime(1);
        assert_eq!(small.duration_since(big), SimTime::ZERO);
        assert_eq!(big.duration_since(small), SimTime(u64::MAX - 1));
    }

    #[test]
    fn ordering_used_by_min_heap() {
        let mut times = vec![
            SimTime::from_secs(3.0),
            SimTime::from_secs(1.0),
            SimTime::from_secs(2.0),
        ];
        times.sort();
        assert_eq!(
            times,
            vec![
                SimTime::from_secs(1.0),
                SimTime::from_secs(2.0),
                SimTime::from_secs(3.0)
            ]
        );
    }
}
