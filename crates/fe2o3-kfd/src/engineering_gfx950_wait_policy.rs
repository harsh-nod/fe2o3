//! Diagnostic-only host waiting; no packet or completion-authority changes.

use super::{Result, add_counter, explain};
use std::io::Write;
use std::time::{Duration, Instant};

const ACTIVE_WINDOW: Duration = Duration::from_millis(10);
const FALLBACK_SLEEP: Duration = Duration::from_micros(50);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Ordered64WaitPolicy {
    #[default]
    Sleep50usV1,
    ActivePoll10msV1,
}

impl Ordered64WaitPolicy {
    pub(super) fn require_timestamp_compatibility(self) -> Result<()> {
        if self == Self::ActivePoll10msV1 {
            return Err("diagnostic active polling cannot include dispatch timestamps".into());
        }
        Ok(())
    }

    pub(super) fn require_profile(self, ordered64: bool, profile: bool) -> Result<()> {
        if self == Self::ActivePoll10msV1 && ordered64 && !profile {
            return Err("diagnostic active polling requires performance profiling".into());
        }
        Ok(())
    }

    pub(super) fn start(
        self,
        ordered64: bool,
        deadline: Instant,
    ) -> Result<Option<ActivePollWait>> {
        if self == Self::ActivePoll10msV1 && ordered64 {
            ActivePollWait::new(Instant::now(), deadline).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(super) fn write_terminal_report(
        self,
        unique_id: u64,
        counters: ActivePollCounters,
        mut output: impl Write,
    ) -> Result<()> {
        if self == Self::Sleep50usV1 {
            return Ok(());
        }
        let mut record = serde_json::to_vec(&serde_json::json!({
            "schema": "Fe2o3Ordered64WaitPolicyDiagnosticV1",
            "policy": "ActivePoll10msV1",
            "worker_pid": std::process::id(),
            "device_unique_id": unique_id,
            "scope": "successful ordered64 batches in this worker process",
            "active_window_ns": 10_000_000_u64,
            "fallback_sleep_ns": 50_000_u64,
            "closed_cleanly": true,
            "counters": counters,
        }))
        .map_err(explain)?;
        record.push(b'\n');
        if record.len() > 1024 {
            return Err("active polling terminal record exceeds bound".into());
        }
        output.write_all(&record).map_err(explain)?;
        output.flush().map_err(explain)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PauseAction {
    Spin,
    Sleep(Duration),
}

pub(super) struct ActivePollWait {
    active_until: Instant,
    deadline: Instant,
    spin_pauses: u64,
    fallback_sleeps: u64,
}

impl ActivePollWait {
    pub(super) fn new(now: Instant, deadline: Instant) -> Result<Self> {
        if now >= deadline {
            return Err("active polling deadline expired; process teardown required".into());
        }
        Ok(Self {
            active_until: now
                .checked_add(ACTIVE_WINDOW)
                .ok_or("active polling window overflow")?
                .min(deadline),
            deadline,
            spin_pauses: 0,
            fallback_sleeps: 0,
        })
    }

    pub(super) fn next_action(&mut self, now: Instant) -> Result<PauseAction> {
        if now >= self.deadline {
            return Err("active polling deadline expired; process teardown required".into());
        }
        if now < self.active_until {
            add_counter(&mut self.spin_pauses, 1)?;
            Ok(PauseAction::Spin)
        } else {
            add_counter(&mut self.fallback_sleeps, 1)?;
            Ok(PauseAction::Sleep(
                FALLBACK_SLEEP.min(self.deadline.saturating_duration_since(now)),
            ))
        }
    }

    pub(super) fn pause(&mut self) -> Result<()> {
        match self.next_action(Instant::now())? {
            PauseAction::Spin => core::hint::spin_loop(),
            PauseAction::Sleep(duration) => std::thread::sleep(duration),
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(super) struct ActivePollCounters {
    completed_batches: u64,
    spin_pauses: u64,
    fallback_sleeps: u64,
    completed_without_fallback: u64,
    completed_after_fallback: u64,
}

impl ActivePollCounters {
    pub(super) fn record_completed(&mut self, wait: &ActivePollWait) -> Result<()> {
        let mut next = *self;
        add_counter(&mut next.completed_batches, 1)?;
        add_counter(&mut next.spin_pauses, wait.spin_pauses)?;
        add_counter(&mut next.fallback_sleeps, wait.fallback_sleeps)?;
        if wait.fallback_sleeps == 0 {
            add_counter(&mut next.completed_without_fallback, 1)?;
        } else {
            add_counter(&mut next.completed_after_fallback, 1)?;
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_wait_policy_tests.rs"]
mod tests;
