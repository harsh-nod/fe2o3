//! One-quantum scalar dependency progress without recursive owner extraction.

#![forbid(unsafe_code)]

use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrefixError {
    Corrupt,
    Shared,
}

pub(super) fn publication_len(
    direction: usize,
    ready: &VecDeque<u64>,
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &HashMap<u64, SubmissionRecordV1>,
    complete_ready_set: bool,
    shared_read: impl Fn(u64, u64, u64) -> Result<bool, xgmi_directed::OwnerError>,
) -> Result<usize, PrefixError> {
    if direction > 1 {
        return Err(PrefixError::Corrupt);
    }
    let window = ready.len().min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
    let mut prefix = window;
    // Validate the entire bounded window, including entries after the first
    // healthy read/read collision, so Busy cannot conceal a corrupt writer.
    for (index, id) in ready.iter().take(window).enumerate() {
        let record = active.get(id).ok_or(PrefixError::Corrupt)?;
        if record.id != *id
            || *id == 0
            || record.source == 0
            || record.destination == 0
            || record.source == record.destination
            || record.sequence.is_some()
            || !record.ready_indexed
            || completed.contains_key(id)
            || !xgmi_submission_is_ready_v1(record, completed, direction)
        {
            return Err(PrefixError::Corrupt);
        }
        for earlier in ready.iter().take(index) {
            let previous = &active[earlier];
            if earlier == id || previous.stream == record.stream {
                return Err(PrefixError::Corrupt);
            }
            let overlaps = [previous.source, previous.destination]
                .iter()
                .any(|allocation| {
                    *allocation == record.source || *allocation == record.destination
                });
            if overlaps {
                if previous.source != record.source
                    || previous.destination == record.destination
                    || !shared_read(*earlier, *id, record.source)
                        .map_err(|_| PrefixError::Corrupt)?
                {
                    return Err(PrefixError::Corrupt);
                }
                prefix = prefix.min(index);
            }
        }
    }
    if complete_ready_set && prefix != ready.len() {
        Err(PrefixError::Shared)
    } else {
        Ok(prefix)
    }
}

#[derive(Clone, Copy)]
struct Node<'a> {
    id: u64,
    direction: usize,
    dependencies: &'a [u64],
    cursor: usize,
    published: bool,
    sequence: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionError {
    Unknown,
    Unsupported,
    Corrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Complete(BackendPollV1),
    Blocked,
    Observe(u64),
    Publish(usize),
    Fail(u64),
}

trait State {
    fn completed(&self, id: u64) -> Option<BackendPollV1>;
    fn node(&self, id: u64) -> Option<Node<'_>>;
    fn advance(&mut self, id: u64, cursor: usize);
    fn in_flight_ids(&self, direction: usize) -> &[u64];
    fn ready_queue(&self, direction: usize) -> &VecDeque<u64>;
    fn in_flight(&self, direction: usize, focus: u64) -> Option<u64> {
        indexed_xgmi_progress_id_v1(self.in_flight_ids(direction), focus)
    }
    fn ready_contains(&self, direction: usize, id: u64) -> bool;
    fn sequence(&self, direction: usize) -> Option<u64>;
    fn retained(&self, id: u64) -> bool;
    fn depth(&self, id: u64) -> Option<usize>;
}

fn terminal(status: BackendPollV1) -> Result<BackendPollV1, SelectionError> {
    match status {
        BackendPollV1::Pending => Err(SelectionError::Corrupt),
        status => Ok(status),
    }
}

fn dependencies<S: State>(
    state: &S,
    node: Node<'_>,
) -> Result<(usize, Option<u64>, bool), SelectionError> {
    if node.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
        || node.cursor > node.dependencies.len()
    {
        return Err(SelectionError::Corrupt);
    }
    let depth = state.depth(node.id).ok_or(SelectionError::Corrupt)?;
    if depth == 0 || depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        return Err(SelectionError::Corrupt);
    }
    // Revalidate uniqueness without allocation or a quadratic roster scan.
    let mut sorted = [0; MAX_RUNTIME_DEPENDENCIES_V1];
    let sorted = &mut sorted[..node.dependencies.len()];
    sorted.copy_from_slice(node.dependencies);
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SelectionError::Corrupt);
    }
    let mut maximum = 0;
    let mut cursor = node.cursor;
    let mut predecessor = None;
    let mut failed = false;
    for (index, dependency) in node.dependencies.iter().enumerate() {
        let dependency_depth = state.depth(*dependency).ok_or(SelectionError::Corrupt)?;
        if *dependency >= node.id
            || !state.retained(*dependency)
            || dependency_depth == 0
            || dependency_depth >= depth
        {
            return Err(SelectionError::Corrupt);
        }
        maximum = maximum.max(dependency_depth);
        let completed = state.completed(*dependency);
        let active = state.node(*dependency).is_some();
        if active == completed.is_some()
            || completed == Some(BackendPollV1::Pending)
            || index < node.cursor && completed != Some(BackendPollV1::Succeeded)
        {
            return Err(SelectionError::Corrupt);
        }
        if index >= node.cursor {
            match completed {
                Some(BackendPollV1::Succeeded) if predecessor.is_none() => cursor += 1,
                Some(BackendPollV1::Failed { .. }) => failed = true,
                None if predecessor.is_none() => predecessor = Some(*dependency),
                _ => {}
            }
        }
    }
    if depth != maximum + 1 {
        return Err(SelectionError::Corrupt);
    }
    Ok((cursor, predecessor, failed))
}

fn validate_window<S: State>(state: &S, direction: usize) -> Result<(), SelectionError> {
    if state.in_flight_ids(direction).len() > GFX942_SDMA_MAX_IN_FLIGHT_V1
        || state.in_flight_ids(1 - direction).len() > GFX942_SDMA_MAX_IN_FLIGHT_V1
    {
        return Err(SelectionError::Corrupt);
    }
    let mut previous = None;
    for id in state.in_flight_ids(direction) {
        let node = state.node(*id).ok_or(SelectionError::Corrupt)?;
        if previous.is_some_and(|previous| previous >= *id)
            || node.id != *id
            || node.direction != direction
            || !node.published
            || node.sequence
            || state.ready_contains(direction, *id)
            || state.completed(*id).is_some()
            || state.in_flight_ids(1 - direction).contains(id)
        {
            return Err(SelectionError::Corrupt);
        }
        let (_, predecessor, failed) = dependencies(state, node)?;
        if predecessor.is_some() || failed {
            return Err(SelectionError::Corrupt);
        }
        previous = Some(*id);
    }
    Ok(())
}

fn validate_prefix<S: State>(state: &S, direction: usize) -> Result<(), SelectionError> {
    let queue = state.ready_queue(direction);
    if queue.is_empty() {
        return Err(SelectionError::Corrupt);
    }
    for (index, id) in queue.iter().take(GFX942_SDMA_MAX_IN_FLIGHT_V1).enumerate() {
        let node = state.node(*id).ok_or(SelectionError::Corrupt)?;
        if node.id != *id
            || node.direction != direction
            || node.published
            || node.sequence
            || !state.ready_contains(direction, *id)
            || state.completed(*id).is_some()
            || queue.iter().take(index).any(|earlier| earlier == id)
            || state.in_flight_ids(1 - direction).contains(id)
        {
            return Err(SelectionError::Corrupt);
        }
        let (_, predecessor, failed) = dependencies(state, node)?;
        if predecessor.is_some() || failed {
            return Err(SelectionError::Corrupt);
        }
    }
    Ok(())
}

fn select<S: State>(state: &mut S, root: u64) -> Result<Action, SelectionError> {
    if let Some(status) = state.completed(root) {
        if state.node(root).is_some() {
            return Err(SelectionError::Corrupt);
        }
        return terminal(status).map(Action::Complete);
    }
    if state.node(root).is_none() {
        return Err(SelectionError::Unknown);
    }
    let mut current = root;
    for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let node = state.node(current).ok_or(SelectionError::Corrupt)?;
        if node.id != current
            || node.direction > 1
            || state.sequence(node.direction) != node.sequence.then_some(current)
        {
            return Err(SelectionError::Corrupt);
        }
        if node.sequence {
            return if current == root {
                Err(SelectionError::Unsupported)
            } else {
                Ok(Action::Blocked)
            };
        }
        let direction = node.direction;
        let (cursor, predecessor, failed) = dependencies(state, node)?;
        if node.published {
            validate_window(state, direction)?;
            if failed
                || predecessor.is_some()
                || state.ready_contains(direction, current)
                || state.in_flight(direction, current) != Some(current)
            {
                return Err(SelectionError::Corrupt);
            }
            return Ok(Action::Observe(current));
        }
        if failed {
            return Ok(Action::Fail(current));
        }
        state.advance(current, cursor);
        if let Some(dependency) = predecessor {
            current = dependency;
            continue;
        }
        if !state.ready_contains(direction, current) {
            return Err(SelectionError::Corrupt);
        }
        validate_window(state, direction)?;
        if let Some(id) = state.in_flight(direction, current) {
            // A blocker shares the selected native FIFO, not necessarily the
            // root's dependency graph. Its success never completes the root.
            return Ok(Action::Observe(id));
        }
        validate_prefix(state, direction)?;
        return Ok(Action::Publish(direction));
    }
    Err(SelectionError::Corrupt)
}

trait Driver: State {
    type Error;

    fn selection_error(&mut self, error: SelectionError) -> Self::Error;
    fn observe(&mut self, id: u64) -> Result<(), Self::Error>;
    fn publish(&mut self, direction: usize) -> Result<(), Self::Error>;
    fn fail(&mut self, id: u64);
}

fn progress<D: Driver>(driver: &mut D, root: u64) -> Result<BackendPollV1, D::Error> {
    match select(driver, root).map_err(|error| driver.selection_error(error))? {
        Action::Complete(status) => return Ok(status),
        Action::Blocked => return Ok(BackendPollV1::Pending),
        Action::Observe(id) => driver.observe(id)?,
        Action::Publish(direction) => driver.publish(direction)?,
        Action::Fail(id) => driver.fail(id),
    }
    if let Some(status) = driver.completed(root) {
        if driver.node(root).is_some() {
            return Err(driver.selection_error(SelectionError::Corrupt));
        }
        terminal(status).map_err(|error| driver.selection_error(error))
    } else if driver.node(root).is_some() {
        Ok(BackendPollV1::Pending)
    } else {
        Err(driver.selection_error(SelectionError::Corrupt))
    }
}

impl State for KfdNativeXgmiRuntimeBackendV1 {
    fn completed(&self, id: u64) -> Option<BackendPollV1> {
        self.submissions.get(&id).map(|record| record.status)
    }

    fn node(&self, id: u64) -> Option<Node<'_>> {
        self.active.get(&id).map(|active| Node {
            id: active.id,
            direction: active.direction,
            dependencies: &active.dependencies,
            cursor: active.dependency_cursor,
            published: active.ticket.is_some(),
            sequence: active.sequence.is_some(),
        })
    }

    fn advance(&mut self, id: u64, cursor: usize) {
        self.active
            .get_mut(&id)
            .expect("selected scalar owner")
            .dependency_cursor = cursor;
    }

    fn in_flight_ids(&self, direction: usize) -> &[u64] {
        &self.in_flight_by_direction[direction]
    }

    fn ready_queue(&self, direction: usize) -> &VecDeque<u64> {
        &self.ready_by_direction[direction]
    }

    fn ready_contains(&self, direction: usize, id: u64) -> bool {
        self.active
            .get(&id)
            .is_some_and(|active| active.direction == direction && active.ready_indexed)
    }

    fn sequence(&self, direction: usize) -> Option<u64> {
        self.sequence_by_direction[direction]
    }

    fn retained(&self, id: u64) -> bool {
        self.dependency_retain_counts
            .get(&id)
            .is_some_and(|count| *count != 0)
    }

    fn depth(&self, id: u64) -> Option<usize> {
        self.dependency_depths.get(&id).copied()
    }
}

impl Driver for KfdNativeXgmiRuntimeBackendV1 {
    type Error = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

    fn selection_error(&mut self, error: SelectionError) -> Self::Error {
        match error {
            SelectionError::Unknown => Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown scalar XGMI progress submission",
            ),
            SelectionError::Unsupported => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "scalar XGMI progress does not admit ordered sequences",
            ),
            SelectionError::Corrupt => {
                self.terminal_error("scalar XGMI progress custody is inconsistent")
            }
        }
    }

    fn observe(&mut self, id: u64) -> Result<(), Self::Error> {
        self.poll_v1(id).map(|_| ())
    }

    fn publish(&mut self, direction: usize) -> Result<(), Self::Error> {
        match self.publish_ready_peer_batch(direction, false)? {
            XgmiBatchPublicationOutcomeV1::Published
            | XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure => Ok(()),
            XgmiBatchPublicationOutcomeV1::NoReadyWork
            | XgmiBatchPublicationOutcomeV1::AlreadyInFlight => {
                Err(self.terminal_error("scalar XGMI publication changed after selection"))
            }
        }
    }

    fn fail(&mut self, id: u64) {
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_segments_diagnostic.as_mut() {
            recorder.invalidate();
        }
        let active = self
            .active
            .remove(&id)
            .expect("failed scalar dependency owner");
        self.finish_failed(active);
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    /// Advances one scalar copy or one of its retained dependencies.
    ///
    /// Each call performs at most one FIFO batch publication (up to 63 copies),
    /// one published-ticket observation, or one failed-dependency settlement.
    /// Selection is iterative, with at most 256 dependency levels and 256
    /// dependencies per level. Ready membership uses the retained active record, not
    /// a backlog scan. Successful prefixes are retained and revalidated. Completion
    /// wakeups remain bounded by the admitted waiter roster, not constant work.
    /// There is no completion wait, sleep, recursive polling or background task;
    /// native driver calls do not acquire a hard wall-clock bound from this API.
    ///
    /// Only this submission's retained result can return terminal success or
    /// failure. An error grants no additional release or quiescence authority.
    /// Unwind after consuming native custody retains the existing fail-stop
    /// policy; this method does not promise resumable native panic recovery.
    /// An active ordered root is unsupported; an ordered dependency requires its
    /// existing progress path and leaves this root Pending. A blocking ticket in
    /// the same native FIFO may be observed even when unrelated to the root.
    /// Ordinary poll/wait/flush contracts are unchanged.
    /// This backend operation does not enable Context pending-producer reads or
    /// add Worker transport, native qualification or formal refinement evidence.
    pub fn progress_peer_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        progress(self, submission)
    }
}
