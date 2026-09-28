//! One originating-thread attempt; unresolved custody is retained until process
//! termination. Quarantine is passive retention, never successful cleanup.

use super::*;
use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::Index;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

type Result<T> = std::result::Result<T, RetainedFunctionalRefinementRuntimeErrorV1>;
// One admitted creation can add a child before the over-limit refusal is known.
const CUSTODY_CAPACITY: usize = MAX_TRACEES + 1;
static PROCESS: AtomicU32 = AtomicU32::new(0);
static POISONED: AtomicBool = AtomicBool::new(false);
static SLOT: Mutex<Option<Box<Run>>> = Mutex::new(None);

#[cfg(test)]
thread_local! { static PUBLICATIONS: Cell<Option<usize>> = const { Cell::new(None) }; }

unsafe extern "C" {
    fn gettid() -> i32;
}

pub(crate) struct AttemptV1 {
    slot: MutexGuard<'static, Option<Box<Run>>>,
    process: u32,
    tid: i32,
    thread: thread::ThreadId,
    _not_send: PhantomData<Rc<()>>,
}

pub(super) fn poison() {
    POISONED.store(true, Ordering::Release);
}

fn quarantined() -> RetainedFunctionalRefinementRuntimeErrorV1 {
    controller_error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::Quarantined,
        "proof execution is permanently quarantined; cleanup is not confirmed",
    )
}

impl AttemptV1 {
    #[cfg(test)]
    pub(crate) fn observe_publications() {
        PUBLICATIONS.set(Some(0));
    }

    #[cfg(test)]
    pub(crate) fn observed_publications() -> usize {
        PUBLICATIONS.get().unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn before_publication(&self) {
        let Some(count) = PUBLICATIONS.get() else {
            return;
        };
        self.complete().unwrap();
        let busy = || {
            assert_eq!(
                Self::begin()
                    .err()
                    .expect("publication must retain gate")
                    .kind(),
                RetainedFunctionalRefinementRuntimeErrorKindV1::Busy
            );
        };
        busy();
        thread::spawn(busy).join().unwrap();
        PUBLICATIONS.set(Some(count + 1));
    }

    pub(crate) fn begin() -> Result<Self> {
        let process = std::process::id();
        if !admitted_process(process) {
            return Err(controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::OwnerProcessChanged,
                "proof custody cannot cross a process boundary",
            ));
        }
        if POISONED.load(Ordering::Acquire) {
            return Err(quarantined());
        }
        let mut slot = match SLOT.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::WouldBlock) => {
                return Err(controller_error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::Busy,
                    "another proof attempt owns the process execution gate",
                ));
            }
            Err(TryLockError::Poisoned(_)) => {
                poison();
                return Err(quarantined());
            }
        };
        if POISONED.load(Ordering::Acquire) || slot.is_some() {
            poison();
            return Err(quarantined());
        }
        // SAFETY: gettid has no arguments and returns the calling kernel thread.
        let tid = unsafe { gettid() };
        let thread = thread::current().id();
        *slot = Some(Box::new(Run::new(process, tid, thread)?));
        Ok(Self {
            slot,
            process,
            tid,
            thread,
            _not_send: PhantomData,
        })
    }

    pub(crate) fn check(&self) -> Result<()> {
        // SAFETY: read only the current thread identity, never act on a saved PID.
        if self.process != std::process::id()
            || self.tid != unsafe { gettid() }
            || self.thread != thread::current().id()
        {
            return Err(controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::OwnerProcessChanged,
                "proof attempt left its originating process/thread",
            ));
        }
        if POISONED.load(Ordering::Acquire) {
            return Err(quarantined());
        }
        Ok(())
    }

    pub(crate) fn complete(&self) -> Result<()> {
        self.check()?;
        if self
            .slot
            .as_ref()
            .is_some_and(|run| run.tracees.unresolved())
        {
            poison();
            return Err(quarantined());
        }
        Ok(())
    }

    pub(super) fn run(&mut self) -> Result<&mut Run> {
        self.check()?;
        self.slot.as_deref_mut().ok_or_else(quarantined)
    }

    #[cfg(test)]
    pub(super) fn id(&self) -> u32 {
        self.slot
            .as_ref()
            .and_then(|r| r.root)
            .expect("spawned fixture") as u32
    }
}

// Allocation-free process check, before any inherited mutex or PID operation.
fn admitted_process(process: u32) -> bool {
    match PROCESS.compare_exchange(0, process, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => true,
        Err(admitted) => admitted == process,
    }
}

#[cfg(test)]
pub(super) fn inherited_process_refused() -> bool {
    !admitted_process(std::process::id())
}

impl Drop for AttemptV1 {
    fn drop(&mut self) {
        // No allocation, wait, syscall cleanup, reaper or second lock on unwind.
        // Static storage is intentionally not destroyed while quarantined.
        if self
            .slot
            .as_ref()
            .is_some_and(|run| run.tracees.unresolved())
        {
            poison();
        }
        if !POISONED.load(Ordering::Acquire) {
            self.slot.take();
        }
    }
}

#[cfg(test)]
pub(super) fn inspect_retained<T>(inspect: impl FnOnce(&Run) -> T) -> T {
    let slot = SLOT.try_lock().unwrap_or_else(|error| match error {
        TryLockError::Poisoned(error) => error.into_inner(),
        TryLockError::WouldBlock => panic!("attempt guard must be dropped before inspection"),
    });
    assert!(POISONED.load(Ordering::Acquire));
    inspect(slot.as_deref().expect("quarantined owner retained"))
}

pub(super) struct Run {
    pub origin: (u32, i32, thread::ThreadId),
    pub tracees: Tracees,
    pub root: Option<i32>,
    pub seized: bool,
    pub child: seized_spawn::SeizedChild,
    pub prepared: Option<seized_spawn::PreparedSpawn>,
    pub backing: Option<Arc<RetainedRuntimeClosureV2>>,
    pub sealed: Option<SealedGeneratedProofSourceV3>,
    pub descriptors: Vec<std::os::fd::OwnedFd>,
    pub stdout_capture: Capture,
    pub stderr_capture: Capture,
}

impl Run {
    fn new(process: u32, tid: i32, thread: thread::ThreadId) -> Result<Self> {
        Ok(Self {
            origin: (process, tid, thread),
            tracees: Tracees::new()?,
            root: None,
            seized: false,
            child: seized_spawn::SeizedChild::empty(),
            prepared: None,
            backing: None,
            sealed: None,
            descriptors: Vec::new(),
            stdout_capture: Capture {
                bytes: Vec::new(),
                eof: false,
            },
            stderr_capture: Capture {
                bytes: Vec::new(),
                eof: false,
            },
        })
    }

    pub(super) fn check_thread(&self) -> Result<()> {
        // SAFETY: current thread identity only; a recycled saved TID is not authority.
        if self.origin
            != (
                std::process::id(),
                unsafe { gettid() },
                thread::current().id(),
            )
        {
            return Err(process_failure("proof run left its originating thread"));
        }
        Ok(())
    }
}

/// Fixed-reservation, sorted inventory. No insertion allocates after fork.
pub(super) struct Tracees {
    entries: Vec<(i32, Tracee)>,
    uncertain: Cell<Option<i32>>,
}

impl Tracees {
    pub(super) fn new() -> Result<Self> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(CUSTODY_CAPACITY)
            .map_err(|_| process_failure("reserve bounded proof task custody"))?;
        Ok(Self {
            entries,
            uncertain: Cell::new(None),
        })
    }

    pub(super) fn unresolved(&self) -> bool {
        self.uncertain.get().is_some()
            || self.values().any(|t| {
                !t.terminal_consumed || t.current_stop.is_some_and(TraceeStop::unregistered_birth)
            })
    }

    pub(super) fn uncertain(&self, pid: i32) {
        if self.uncertain.get().is_none() {
            self.uncertain.set(Some(pid));
        }
    }
    pub(super) fn has_uncertain(&self) -> bool {
        self.uncertain.get().is_some()
    }

    pub(super) fn insert(&mut self, pid: i32, task: Tracee) -> Result<()> {
        match self.entries.binary_search_by_key(&pid, |(pid, _)| *pid) {
            Ok(index) => self.entries[index].1 = task,
            Err(index) if self.entries.len() < CUSTODY_CAPACITY => {
                self.entries.insert(index, (pid, task))
            }
            Err(_) => {
                self.uncertain(pid);
                return Err(process_failure(
                    "proof task emergency custody capacity exhausted",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn get(&self, pid: &i32) -> Option<&Tracee> {
        self.entries
            .binary_search_by_key(pid, |(pid, _)| *pid)
            .ok()
            .map(|i| &self.entries[i].1)
    }
    pub(super) fn get_mut(&mut self, pid: &i32) -> Option<&mut Tracee> {
        self.entries
            .binary_search_by_key(pid, |(pid, _)| *pid)
            .ok()
            .map(|i| &mut self.entries[i].1)
    }
    pub(super) fn remove(&mut self, pid: &i32) -> Option<Tracee> {
        self.entries
            .binary_search_by_key(pid, |(pid, _)| *pid)
            .ok()
            .map(|i| self.entries.remove(i).1)
    }
    pub(super) fn contains_key(&self, pid: &i32) -> bool {
        self.get(pid).is_some()
    }
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&i32, &Tracee)> {
        self.entries.iter().map(|(pid, task)| (pid, task))
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = &i32> {
        self.entries.iter().map(|(pid, _)| pid)
    }
    pub(super) fn values(&self) -> impl Iterator<Item = &Tracee> {
        self.entries.iter().map(|(_, task)| task)
    }
    pub(super) fn pids(&self) -> impl Iterator<Item = i32> + use<> {
        let mut pids = [0; CUSTODY_CAPACITY];
        for (out, (pid, _)) in pids.iter_mut().zip(&self.entries) {
            *out = *pid;
        }
        pids.into_iter().take(self.entries.len())
    }
}

impl Index<&i32> for Tracees {
    type Output = Tracee;
    fn index(&self, pid: &i32) -> &Self::Output {
        self.get(pid).expect("retained proof task")
    }
}

impl<'a> IntoIterator for &'a Tracees {
    type Item = (&'a i32, &'a Tracee);
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, (i32, Tracee)>, fn(&'a (i32, Tracee)) -> Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(pid, task)| (pid, task))
    }
}
