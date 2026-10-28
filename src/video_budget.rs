//! One cooperative wall-clock budget for a video observation workflow.
//!
//! Callers check between filesystem operations, hashing chunks, provider calls
//! and parsing stages, and pass `remaining()` to bounded child capture. This
//! clock does not preempt a blocking filesystem syscall, parser or scheduler;
//! it refuses further work or a successful report once a checkpoint observes
//! cancellation or expiry. It is not a provider memory or scratch-space quota.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};

const MAX_ELAPSED_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OperationBudget {
    started: Instant,
    deadline: Instant,
    limit: Duration,
}

impl OperationBudget {
    pub(crate) fn new(elapsed_ms: u64) -> Result<Self> {
        ensure!(
            (1..=MAX_ELAPSED_MS).contains(&elapsed_ms),
            "video workflow elapsed budget must be between 1 and 30000 milliseconds"
        );
        let started = Instant::now();
        let limit = Duration::from_millis(elapsed_ms);
        let deadline = started
            .checked_add(limit)
            .context("video workflow deadline is outside the monotonic clock range")?;
        Ok(Self { started, deadline, limit })
    }

    pub(crate) fn check(&self, cancel: &impl Fn() -> bool) -> Result<()> {
        self.remaining(cancel).map(|_| ())
    }

    /// Remaining time from the original start, never a newly reset deadline.
    pub(crate) fn remaining(&self, cancel: &impl Fn() -> bool) -> Result<Duration> {
        ensure!(!cancel(), "video workflow cancelled");
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .context("video workflow elapsed budget exhausted")
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub(crate) fn limit(&self) -> Duration {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_duration_before_constructing_deadline() {
        for invalid in [0, MAX_ELAPSED_MS + 1, u64::MAX] {
            assert!(OperationBudget::new(invalid).is_err());
        }
        let budget = OperationBudget::new(MAX_ELAPSED_MS).unwrap();
        assert_eq!(budget.limit(), Duration::from_secs(30));
        let remaining = budget.remaining(&|| false).unwrap();
        assert!(!remaining.is_zero() && remaining <= budget.limit());
    }

    #[test]
    fn cancellation_refuses_both_checkpoint_and_child_time_allocation() {
        let budget = OperationBudget::new(MAX_ELAPSED_MS).unwrap();
        assert_eq!(budget.check(&|| true).unwrap_err().to_string(), "video workflow cancelled");
        assert_eq!(budget.remaining(&|| true).unwrap_err().to_string(), "video workflow cancelled");
        // The budget is still the same clock; checking it never consumes or
        // resets it, and a caller owns its cancellation signal.
        assert!(budget.check(&|| false).is_ok());
    }

    #[test]
    fn elapsed_deadline_refuses_without_sleeping_or_resetting_the_budget() {
        let deadline = Instant::now();
        let limit = Duration::from_millis(1);
        let budget = OperationBudget {
            started: deadline.checked_sub(limit).unwrap(),
            deadline,
            limit,
        };
        assert_eq!(
            budget.remaining(&|| false).unwrap_err().to_string(),
            "video workflow elapsed budget exhausted"
        );
        assert!(budget.check(&|| false).is_err());
        assert!(budget.elapsed() >= limit);
        assert_eq!(budget.limit(), limit);
    }
}
