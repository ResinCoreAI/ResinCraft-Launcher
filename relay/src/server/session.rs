//! A tunnel's session time: it ends `session_secs` after it came online unless the host continues. Warnings
//! go out as the time left crosses each warning threshold (10 and 1 minutes by default), once each per
//! period; continuing (or an admin's Extend) starts a new period.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use tokio::sync::Notify;

/// What the clock says on a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Nothing,
    /// Send a warning: this many seconds are left.
    Warn(u64),
    /// Time's up.
    Ended,
}

pub struct SessionClock {
    /// Unix seconds when the session ends; 0: no limit.
    deadline: AtomicU64,
    continues: AtomicU32,
    /// Thresholds (seconds) already warned about in this period.
    warned: Mutex<Vec<u64>>,
    /// Woken when the deadline moves (continue, Extend), so the agent hears about it.
    pub changed: Notify,
}

impl SessionClock {
    /// A clock started `now` (Unix seconds); `session_secs` 0 means no limit.
    pub fn new(now: u64, session_secs: u64) -> SessionClock {
        SessionClock {
            deadline: AtomicU64::new(if session_secs == 0 { 0 } else { now + session_secs }),
            continues: AtomicU32::new(0),
            warned: Mutex::new(Vec::new()),
            changed: Notify::new(),
        }
    }

    pub fn continues(&self) -> u32 {
        self.continues.load(Ordering::Relaxed)
    }

    /// Seconds left (None: no limit).
    pub fn secs_left(&self, now: u64) -> Option<u64> {
        match self.deadline.load(Ordering::Relaxed) {
            0 => None,
            d => Some(d.saturating_sub(now)),
        }
    }

    /// What to do now. `limit_on` is false while the relay's limit is set to 0 (unlimited): nothing happens.
    pub fn tick(&self, now: u64, warn_secs: &[u64], limit_on: bool) -> Tick {
        let Some(left) = self.secs_left(now).filter(|_| limit_on) else { return Tick::Nothing };
        if left == 0 {
            return Tick::Ended;
        }
        let mut warned = self.warned.lock().unwrap();
        let due: Vec<u64> = warn_secs.iter().copied().filter(|t| *t > 0 && left <= *t && !warned.contains(t)).collect();
        if due.is_empty() {
            return Tick::Nothing;
        }
        // Crossing several thresholds at once (a short limit) still sends one warning.
        warned.extend(due);
        Tick::Warn(left)
    }

    /// The host's `continue`: another full session on top of what's left, but never more than one session
    /// plus `window` ahead (how early the host may continue), so continuing early can't pile sessions up.
    /// Returns the seconds left.
    pub fn continue_session(&self, now: u64, session_secs: u64, window: u64) -> Option<u64> {
        let left = self.secs_left(now)?;
        let new_left = (left + session_secs).min(session_secs + window);
        Some(self.set_left(now, new_left))
    }

    /// The admin's Extend: another full session on top of what's left. Returns the seconds left.
    pub fn extend(&self, now: u64, session_secs: u64) -> Option<u64> {
        let left = self.secs_left(now)?;
        Some(self.set_left(now, left + session_secs))
    }

    /// A fresh session from `now` (after the admin took an account's unlimited use away); 0: no limit.
    pub fn restart(&self, now: u64, session_secs: u64) {
        self.deadline.store(if session_secs == 0 { 0 } else { now + session_secs }, Ordering::Relaxed);
        self.warned.lock().unwrap().clear();
        self.changed.notify_one();
    }

    fn set_left(&self, now: u64, left: u64) -> u64 {
        self.deadline.store(now + left, Ordering::Relaxed);
        self.continues.fetch_add(1, Ordering::Relaxed);
        self.warned.lock().unwrap().clear();
        self.changed.notify_one();
        left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3600;
    const WARN: [u64; 2] = [600, 60];

    #[test]
    fn warns_once_per_threshold_then_ends() {
        let c = SessionClock::new(1000, 2 * HOUR);
        let end = 1000 + 2 * HOUR;
        assert_eq!(c.secs_left(1000), Some(2 * HOUR));
        assert_eq!(c.tick(1000, &WARN, true), Tick::Nothing);
        assert_eq!(c.tick(end - 601, &WARN, true), Tick::Nothing);
        assert_eq!(c.tick(end - 600, &WARN, true), Tick::Warn(600));
        assert_eq!(c.tick(end - 599, &WARN, true), Tick::Nothing, "the 10-minute warning goes out once");
        assert_eq!(c.tick(end - 60, &WARN, true), Tick::Warn(60));
        assert_eq!(c.tick(end - 30, &WARN, true), Tick::Nothing);
        assert_eq!(c.tick(end, &WARN, true), Tick::Ended);
        assert_eq!(c.tick(end + 5, &WARN, true), Tick::Ended);
    }

    #[test]
    fn short_limits_send_one_warning() {
        let c = SessionClock::new(0, 30);
        assert_eq!(c.tick(0, &WARN, true), Tick::Warn(30));
        assert_eq!(c.tick(1, &WARN, true), Tick::Nothing);
    }

    #[test]
    fn no_limit_and_limit_switched_off() {
        let c = SessionClock::new(0, 0);
        assert_eq!(c.secs_left(0), None);
        assert_eq!(c.tick(10 * HOUR, &WARN, true), Tick::Nothing);
        assert_eq!(c.continue_session(0, HOUR, 600), None);
        let limited = SessionClock::new(0, 60);
        assert_eq!(limited.tick(100, &WARN, false), Tick::Nothing, "a limit of 0 in the dashboard stops the clock");
    }

    #[test]
    fn continue_adds_a_session_but_never_piles_up() {
        let c = SessionClock::new(0, 2 * HOUR);
        let at = 2 * HOUR - 600; // at the 10-minute warning
        assert_eq!(c.tick(at, &WARN, true), Tick::Warn(600));
        assert_eq!(c.continue_session(at, 2 * HOUR, 600), Some(600 + 2 * HOUR));
        assert_eq!(c.continues(), 1);
        // A new period: warnings come again before the new end.
        let end = at + 600 + 2 * HOUR;
        assert_eq!(c.tick(end - 600, &WARN, true), Tick::Warn(600));
        // Continuing right away again is capped at one session plus the warning window.
        let c2 = SessionClock::new(0, 2 * HOUR);
        assert_eq!(c2.continue_session(10, 2 * HOUR, 600), Some(2 * HOUR + 600));
        assert_eq!(c2.continue_session(11, 2 * HOUR, 600), Some(2 * HOUR + 600));
        // Continuing 15 minutes before the end, with a 15-minute window, loses nothing.
        let c3 = SessionClock::new(0, 2 * HOUR);
        assert_eq!(c3.continue_session(2 * HOUR - 900, 2 * HOUR, 900), Some(2 * HOUR + 900));
    }

    #[test]
    fn restart_starts_a_fresh_period() {
        let c = SessionClock::new(0, HOUR);
        assert_eq!(c.tick(HOUR - 60, &WARN, true), Tick::Warn(60));
        c.restart(2 * HOUR, HOUR);
        assert_eq!(c.secs_left(2 * HOUR), Some(HOUR));
        assert_eq!(c.tick(3 * HOUR - 600, &WARN, true), Tick::Warn(600));
    }

    #[test]
    fn admin_extend_is_not_capped() {
        let c = SessionClock::new(0, HOUR);
        assert_eq!(c.extend(0, HOUR), Some(2 * HOUR));
        assert_eq!(c.extend(0, HOUR), Some(3 * HOUR));
        assert_eq!(c.continues(), 2);
    }
}
