//! Monotonic, recording-relative session clock.

use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
struct ClockState {
    started_at: Option<Instant>,
    stopped_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_duration: Duration,
}

/// Provides recording-relative monotonic timestamps without using wall-clock
/// time. Pause intervals are excluded from `elapsed` once they are known.
#[derive(Debug, Default)]
pub struct SessionClock {
    state: Mutex<ClockState>,
}

impl SessionClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = ClockState {
                started_at: Some(Instant::now()),
                ..ClockState::default()
            };
        }
    }

    pub fn stop(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.started_at.is_some() && state.stopped_at.is_none() {
                state.stopped_at = Some(Instant::now());
            }
        }
    }

    pub fn pause(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.started_at.is_some() && state.stopped_at.is_none() && state.paused_at.is_none() {
                state.paused_at = Some(Instant::now());
            }
        }
    }

    pub fn resume(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(paused_at) = state.paused_at.take() {
                let end = state.stopped_at.unwrap_or_else(Instant::now);
                state.paused_duration += end.duration_since(paused_at);
            }
        }
    }

    pub fn reset(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = ClockState::default();
        }
    }

    pub fn elapsed(&self) -> Duration {
        let Ok(state) = self.state.lock() else {
            return Duration::ZERO;
        };
        let Some(started_at) = state.started_at else {
            return Duration::ZERO;
        };

        let end = state.stopped_at.unwrap_or_else(Instant::now);
        let paused_now = state
            .paused_at
            .map(|paused_at| end.saturating_duration_since(paused_at))
            .unwrap_or(Duration::ZERO);
        end.saturating_duration_since(started_at)
            .saturating_sub(state.paused_duration)
            .saturating_sub(paused_now)
    }

    pub fn pts_ns(&self) -> u64 {
        self.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn clock_is_zero_before_start_and_monotonic_after_start() {
        let clock = SessionClock::new();
        assert_eq!(clock.elapsed(), Duration::ZERO);

        clock.start();
        let first = clock.pts_ns();
        sleep(Duration::from_millis(1));
        let second = clock.pts_ns();

        assert!(second >= first);
        assert!(second > 0);
    }

    #[test]
    fn pause_interval_is_not_counted_in_active_time() {
        let clock = SessionClock::new();
        clock.start();
        sleep(Duration::from_millis(2));
        clock.pause();
        let paused_value = clock.pts_ns();
        sleep(Duration::from_millis(4));
        assert!(clock.pts_ns().saturating_sub(paused_value) < 3_000_000);

        clock.resume();
        sleep(Duration::from_millis(2));
        assert!(clock.pts_ns() > paused_value);
    }

    #[test]
    fn stop_freezes_elapsed_time_until_next_start() {
        let clock = SessionClock::new();
        clock.start();
        sleep(Duration::from_millis(1));
        clock.stop();
        let stopped = clock.pts_ns();
        sleep(Duration::from_millis(2));

        assert_eq!(clock.pts_ns(), stopped);
    }
}
