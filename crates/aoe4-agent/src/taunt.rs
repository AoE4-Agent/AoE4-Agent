//! Cooldown and consecutive-frame gates for taunt decisions.
//!
//! The vision layer says "enemy pixels are near the town center". This
//! module decides whether that observation is stable enough, and whether
//! the matching taunt is off cooldown, before any keystroke is sent.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauntRule {
    pub id: String,
    pub command: String,
    pub label: String,
    pub cooldown: Duration,
    pub min_clusters: usize,
}

#[derive(Debug, Clone)]
pub struct CooldownGate {
    last: BTreeMap<String, Instant>,
}

impl CooldownGate {
    pub fn new() -> Self {
        Self {
            last: BTreeMap::new(),
        }
    }

    pub fn ready(&self, id: &str, cooldown: Duration, now: Instant) -> bool {
        match self.last.get(id) {
            None => true,
            Some(then) => now.saturating_duration_since(*then) >= cooldown,
        }
    }

    pub fn mark(&mut self, id: &str, now: Instant) {
        self.last.insert(id.to_string(), now);
    }
}

impl Default for CooldownGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Fires only after `required` consecutive positive observations, then
/// consumes the matching rule's cooldown and resets the streak so a single
/// detection cannot retrigger on the next frame.
#[derive(Debug, Clone)]
pub struct TauntBrain {
    gate: CooldownGate,
    streak: u32,
    required: u32,
}

impl TauntBrain {
    pub fn new(required_frames: u32) -> Self {
        Self {
            gate: CooldownGate::new(),
            streak: 0,
            required: required_frames.max(1),
        }
    }

    /// `near_clusters` is the count of enemy blobs already filtered to the
    /// town-center disk. Returns the index of the rule to send, if any.
    pub fn observe(
        &mut self,
        rules: &[TauntRule],
        near_clusters: usize,
        now: Instant,
    ) -> Option<usize> {
        let active = rules.iter().any(|rule| near_clusters >= rule.min_clusters);
        if !active {
            self.streak = 0;
            return None;
        }
        self.streak = self.streak.saturating_add(1).min(self.required);
        if self.streak < self.required {
            return None;
        }
        for (index, rule) in rules.iter().enumerate() {
            if near_clusters >= rule.min_clusters && self.gate.ready(&rule.id, rule.cooldown, now) {
                self.gate.mark(&rule.id, now);
                self.streak = 0;
                return Some(index);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, clusters: usize, secs: u64) -> TauntRule {
        TauntRule {
            id: id.to_string(),
            command: format!("/{id}"),
            label: id.to_string(),
            cooldown: Duration::from_secs(secs),
            min_clusters: clusters,
        }
    }

    #[test]
    fn cooldown_is_per_id_and_elapsed_time() {
        let mut gate = CooldownGate::new();
        let t0 = Instant::now();
        assert!(gate.ready("a", Duration::from_secs(10), t0));
        gate.mark("a", t0);
        assert!(!gate.ready("a", Duration::from_secs(10), t0));
        assert!(gate.ready("b", Duration::from_secs(10), t0));
        assert!(!gate.ready("a", Duration::from_secs(10), t0 + Duration::from_secs(9)));
        assert!(gate.ready("a", Duration::from_secs(10), t0 + Duration::from_secs(10)));
        assert!(gate.ready("a", Duration::ZERO, t0));
    }

    #[test]
    fn streak_must_be_continuous_before_a_send() {
        let mut brain = TauntBrain::new(3);
        let rules = vec![rule("63", 1, 30)];
        let t0 = Instant::now();
        assert!(brain.observe(&rules, 1, t0).is_none());
        assert!(brain.observe(&rules, 1, t0).is_none());
        assert!(
            brain.observe(&rules, 0, t0).is_none(),
            "a gap resets the streak"
        );
        assert!(brain.observe(&rules, 1, t0).is_none());
        assert!(brain.observe(&rules, 1, t0).is_none());
        assert_eq!(brain.observe(&rules, 2, t0), Some(0));
        assert!(brain.observe(&rules, 2, t0).is_none());
        assert!(brain.observe(&rules, 2, t0).is_none());
        assert!(
            brain.observe(&rules, 2, t0).is_none(),
            "the streak is full again but the taunt is still on cooldown"
        );
        assert_eq!(
            brain.observe(&rules, 2, t0 + Duration::from_secs(30)),
            Some(0)
        );
    }

    #[test]
    fn later_rule_fires_when_the_first_is_cooling_down() {
        let mut brain = TauntBrain::new(1);
        let rules = vec![rule("63", 1, 30), rule("12", 2, 30)];
        let t0 = Instant::now();
        assert_eq!(brain.observe(&rules, 2, t0), Some(0));
        assert_eq!(brain.observe(&rules, 2, t0), Some(1));
        assert!(brain.observe(&rules, 2, t0).is_none());
    }
}
