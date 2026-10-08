//! CPU observation bounds only, not GPU timestamps or completion authority.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadWindow {
    pub(super) before_ns: u64,
    pub(super) after_ns: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SignalState {
    Pending,
    Completed,
    Unexpected,
}

#[derive(Debug)]
pub(super) struct Observation {
    pub(super) valid: bool,
    pub(super) reads: u64,
    pub(super) pending_reads: u64,
    pub(super) completed_reads: u64,
    pub(super) unexpected_reads: u64,
    pub(super) last_pending: Option<ReadWindow>,
    pub(super) first_completed: Option<ReadWindow>,
    pub(super) max_inter_read_ns: u64,
    pub(super) read_ns: u64,
    pub(super) pauses: u64,
    pub(super) pause_ns: u64,
    pub(super) max_pause_ns: u64,
    pub(super) post_read_ns: u64,
    pub(super) currentness_ns: u64,
    pub(super) retirement_ns: u64,
    pub(super) ready_ns: Option<u64>,
    previous_read: Option<ReadWindow>,
    last_event_ns: u64,
}

impl Default for Observation {
    fn default() -> Self {
        Self {
            valid: true,
            reads: 0,
            pending_reads: 0,
            completed_reads: 0,
            unexpected_reads: 0,
            last_pending: None,
            first_completed: None,
            max_inter_read_ns: 0,
            read_ns: 0,
            pauses: 0,
            pause_ns: 0,
            max_pause_ns: 0,
            post_read_ns: 0,
            currentness_ns: 0,
            retirement_ns: 0,
            ready_ns: None,
            previous_read: None,
            last_event_ns: 0,
        }
    }
}

impl Observation {
    fn add(valid: &mut bool, total: &mut u64, amount: u64) {
        match total.checked_add(amount) {
            Some(value) => *total = value,
            None => *valid = false,
        }
    }

    fn interval(&mut self, before_ns: u64, after_ns: u64) -> Option<u64> {
        if before_ns < self.last_event_ns || after_ns < before_ns {
            self.valid = false;
            return None;
        }
        self.last_event_ns = after_ns;
        Some(after_ns - before_ns)
    }

    pub(super) fn read(&mut self, before_ns: u64, after_ns: u64, state: SignalState) {
        let Some(elapsed) = self.interval(before_ns, after_ns) else {
            return;
        };
        let window = ReadWindow {
            before_ns,
            after_ns,
        };
        if let Some(previous) = self.previous_read {
            // Conservative upper bound including both acquisition intervals.
            self.max_inter_read_ns = self.max_inter_read_ns.max(after_ns - previous.before_ns);
        }
        self.previous_read = Some(window);
        Self::add(&mut self.valid, &mut self.reads, 1);
        Self::add(&mut self.valid, &mut self.read_ns, elapsed);
        match state {
            SignalState::Pending => {
                Self::add(&mut self.valid, &mut self.pending_reads, 1);
                if self.first_completed.is_some() {
                    self.valid = false;
                } else {
                    self.last_pending = Some(window);
                }
            }
            SignalState::Completed => {
                Self::add(&mut self.valid, &mut self.completed_reads, 1);
                self.first_completed.get_or_insert(window);
            }
            SignalState::Unexpected => {
                Self::add(&mut self.valid, &mut self.unexpected_reads, 1);
                self.valid = false;
            }
        }
    }

    pub(super) fn pause(&mut self, before_ns: u64, after_ns: u64) {
        if let Some(elapsed) = self.interval(before_ns, after_ns) {
            Self::add(&mut self.valid, &mut self.pauses, 1);
            Self::add(&mut self.valid, &mut self.pause_ns, elapsed);
            self.max_pause_ns = self.max_pause_ns.max(elapsed);
        }
    }

    pub(super) fn post_read(
        &mut self,
        before_ns: u64,
        after_ns: u64,
        currentness_ns: u64,
        ready: bool,
    ) {
        if let Some(elapsed) = self.interval(before_ns, after_ns) {
            self.valid &= currentness_ns <= elapsed;
            Self::add(&mut self.valid, &mut self.post_read_ns, elapsed);
            Self::add(&mut self.valid, &mut self.currentness_ns, currentness_ns);
            if ready {
                self.valid &= self.first_completed.is_some();
                self.ready_ns.get_or_insert(after_ns);
            }
        }
    }

    pub(super) fn retirement(&mut self, before_ns: u64, after_ns: u64) {
        if let Some(elapsed) = self.interval(before_ns, after_ns) {
            self.valid &= self.ready_ns.is_some();
            Self::add(&mut self.valid, &mut self.retirement_ns, elapsed);
        }
    }

    pub(super) fn completion_bracket_ns(&self) -> Option<[u64; 2]> {
        if !self.valid {
            return None;
        }
        let completed = self.first_completed?;
        Some([
            self.last_pending.map_or(0, |read| read.before_ns),
            completed.after_ns,
        ])
    }
}

#[cfg(test)]
#[path = "engineering_native_wait_observation_tests.rs"]
mod tests;
