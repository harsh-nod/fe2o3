//! Bounded SC scheduler abstraction, not execution of LDS/subgroup terminals.
//!
//! Valid runs assume coherent observations, valid numerical bodies, and that
//! each started body and local collective eventually returns. Safety assertions
//! hold on every prefix without fairness. Quiescent completion is checked only
//! after every enabled transition has been explored, not for stalled prefixes.
//!
//! Arrival counts use the threshold quotient 0/1/63/64: either the leader arrives
//! before the last peer, or last. Intermediate peer arrivals do not change any
//! scheduler-visible value. The real primitive tests separately cover all 64
//! increments. A leader can start its next probe before its peers arrive, but
//! cannot leave that round's first local barrier until they have arrived.
use super::*;
use std::collections::HashSet;
use std::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Phase {
    Start,
    ReadReady,
    Remove(u8),
    Mark(u8),
    Barrier(u8),
    Body(u8),
    AwaitLeader,
    Exit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Worker {
    rounds: u8,
    empty: u8,
    retired: bool,
    phase: Phase,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct State {
    ready: u8,
    claimed: u8,
    done: u8,
    owners: [u8; 3],
    arrivals: [u8; 3],
    failed: u8,
    workers: [Worker; 2],
}

impl State {
    fn fresh() -> Self {
        Self {
            ready: 1,
            claimed: 0,
            done: 0,
            owners: [2; 3],
            arrivals: [0; 3],
            failed: 0,
            workers: [Worker {
                rounds: 0,
                empty: 0,
                retired: false,
                phase: Phase::Start,
            }; 2],
        }
    }

    fn pending(&self, worker: usize) -> bool {
        (0..3).any(|task| {
            self.owners[task] as usize == worker
                && self.failed & (1 << task) == 0
                && self.arrivals[task] < 64
        })
    }

    fn conserved(&self) {
        assert_eq!((self.ready | self.claimed | self.done) & !7, 0);
        assert_eq!(self.ready & self.claimed, 0);
        assert_eq!(self.done & !self.claimed, 0);
        assert_eq!(self.failed & self.done, 0);
        for task in 0..3 {
            let bit = 1 << task;
            assert!(matches!(self.arrivals[task], 0 | 1 | 63 | 64));
            assert_eq!(self.owners[task] < 2, self.claimed & bit != 0);
            assert_eq!(self.done & bit != 0, self.arrivals[task] == 64);
            if task != 0 && self.claimed & bit != 0 {
                assert_ne!(self.done & 1, 0, "key claimed before norm publication");
            }
        }
        for worker in &self.workers {
            assert!(worker.rounds <= MAX_ROUNDS as u8);
            assert!(worker.empty <= MAX_EMPTY_PROBES as u8);
        }
    }

    fn successor_steps(self, fail_task: Option<u8>, output: &mut Vec<Self>) {
        for index in 0..2 {
            let mut next = self;
            let worker = self.workers[index];
            match worker.phase {
                Phase::Start => {
                    if worker.rounds == MAX_ROUNDS as u8 {
                        if self.pending(index) {
                            continue;
                        }
                        next.workers[index].phase = Phase::Exit;
                    } else {
                        next.workers[index].rounds += 1;
                        if worker.retired || self.done == 7 {
                            next.workers[index].retired = true;
                            next.workers[index].phase = Phase::Barrier(3);
                        } else {
                            next.workers[index].phase = Phase::ReadReady;
                        }
                    }
                }
                Phase::ReadReady => {
                    if self.ready == 0 {
                        next.workers[index].empty += 1;
                        next.workers[index].retired =
                            next.workers[index].empty == MAX_EMPTY_PROBES as u8;
                        next.workers[index].phase = Phase::Barrier(3);
                    } else {
                        next.workers[index].phase =
                            Phase::Remove(self.ready.trailing_zeros() as u8);
                    }
                }
                Phase::Remove(task) => {
                    let bit = 1 << task;
                    next.ready &= !bit;
                    next.workers[index].phase = if self.ready & bit == 0 {
                        Phase::Barrier(3)
                    } else {
                        Phase::Mark(task)
                    };
                }
                Phase::Mark(task) => {
                    let bit = 1 << task;
                    assert_eq!(self.claimed & bit, 0, "duplicate successful claim");
                    next.claimed |= bit;
                    next.owners[task as usize] = index as u8;
                    next.workers[index].phase = Phase::Barrier(task);
                }
                Phase::Barrier(task) => {
                    // The new claim itself is pending, but only older task
                    // arrivals delay entry into this round's numerical body.
                    if (0..3).any(|old| {
                        old != task as usize
                            && self.owners[old] as usize == index
                            && self.failed & (1 << old) == 0
                            && self.arrivals[old] < 64
                    }) {
                        continue;
                    }
                    next.workers[index].phase = if task == 3 {
                        Phase::Start
                    } else {
                        Phase::Body(task)
                    };
                }
                Phase::Body(task) => {
                    if fail_task == Some(task) {
                        next.failed |= 1 << task;
                        next.workers[index].retired = true;
                        next.workers[index].phase = Phase::Start;
                    } else {
                        next.workers[index].phase = Phase::AwaitLeader;
                    }
                }
                Phase::AwaitLeader | Phase::Exit => continue,
            }
            output.push(next);
        }
        for task in 0..3 {
            let owner = self.owners[task] as usize;
            if owner >= 2 || self.failed & (1 << task) != 0 {
                continue;
            }
            match self.arrivals[task] {
                0 if self.workers[owner].phase == Phase::AwaitLeader => {
                    let mut leader_first = self;
                    leader_first.arrivals[task] = 1;
                    leader_first.workers[owner].phase = Phase::Start;
                    output.push(leader_first);
                    let mut peers_first = self;
                    peers_first.arrivals[task] = 63;
                    output.push(peers_first);
                }
                1 | 63 => {
                    let mut next = self;
                    if self.arrivals[task] == 63 {
                        next.workers[owner].phase = Phase::Start;
                    }
                    next.arrivals[task] = 64;
                    next.done |= 1 << task;
                    if task == 0 {
                        next.ready |= 6;
                    }
                    output.push(next);
                }
                _ => {}
            }
        }
    }
}

fn explore(fail_task: Option<u8>) -> (usize, usize, [bool; 2]) {
    const MAX_STATES: usize = 1_000_000;
    const MAX_TRANSITIONS: usize = 4_000_000;
    let mut seen = HashSet::new();
    let mut pending = Vec::new();
    let mut next = Vec::new();
    let mut terminal = 0;
    let mut transitions = 0;
    let mut solo = [false; 2];
    seen.insert(State::fresh());
    pending.push(State::fresh());
    while let Some(state) = pending.pop() {
        state.conserved();
        next.clear();
        state.successor_steps(fail_task, &mut next);
        if next.is_empty() {
            terminal += 1;
            assert!(state.workers.iter().all(|w| w.phase == Phase::Exit));
            match fail_task {
                None => {
                    assert_eq!(state.done, 7, "valid quiescent schedule lost a task");
                    assert_eq!(state.arrivals, [64; 3]);
                    for (worker, observed) in solo.iter_mut().enumerate() {
                        *observed |= state.owners == [worker as u8; 3];
                    }
                }
                Some(task) => {
                    assert_ne!(state.failed & (1 << task), 0);
                    assert_eq!(state.done & (1 << task), 0);
                    assert_ne!(state.done, 7, "failed handler presented as completion");
                }
            }
        }
        for successor in next.drain(..) {
            transitions += 1;
            assert!(transitions <= MAX_TRANSITIONS, "scheduler test work cap");
            if seen.insert(successor) {
                assert!(seen.len() <= MAX_STATES, "scheduler test storage cap");
                pending.push(successor);
            }
        }
    }
    assert!(terminal > 0);
    std::println!(
        "wave scheduler fail={fail_task:?}: {} states, {transitions} transitions, {terminal} terminal",
        seen.len()
    );
    (seen.len(), terminal, solo)
}

#[test]
fn wave_scheduler_explores_bounded_valid_schedules() {
    let (_, _, solo) = explore(None);
    assert_eq!(solo, [true, true]);
}

#[test]
fn wave_scheduler_rejected_handlers_never_become_completion() {
    for task in 0..3 {
        explore(Some(task));
    }
}

#[test]
fn wave_scheduler_stalled_prefix_is_safe_not_complete() {
    let mut state = State::fresh();
    state.ready = 0;
    state.claimed = 1;
    state.owners[0] = 0;
    state.workers[0].rounds = 1;
    state.workers[0].phase = Phase::AwaitLeader;
    state.conserved();
    assert_ne!(state.done, 7);
    let mut enabled = Vec::new();
    state.successor_steps(None, &mut enabled);
    assert!(
        !enabled.is_empty(),
        "stalled prefix is not a quiescent terminal"
    );
}
