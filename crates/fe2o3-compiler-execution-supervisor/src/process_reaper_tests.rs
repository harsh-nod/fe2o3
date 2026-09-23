use super::*;
use fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1;
use rustix::io::{Errno, fcntl_dupfd_cloexec, fcntl_getfd};
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::{
    Pid, PidfdFlags, Signal, WaitId, WaitIdOptions, getpid, pidfd_open, pidfd_send_signal, waitid,
};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::process::{Child, Command, Stdio};

const MAX_POLLS: usize = 500;
const POLL_DELAY: Duration = Duration::from_millis(10);

fn reserve_pool(reaper: &DeferredReaperV1) -> Vec<ReapSlotV1<'_>> {
    (0..MAX_PROTECTED_ISSUER_PROCESSES_V1)
        .map(|_| reaper.reserve_slot().unwrap())
        .collect()
}

fn assert_full(reaper: &DeferredReaperV1) {
    assert!(matches!(
        reaper.reserve_slot(),
        Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity)
    ));
    assert!(reaper.thread_started.get().is_none());
}

struct FakePipeCleanup<'a>(&'a ReapCellV1);

impl Drop for FakePipeCleanup<'_> {
    fn drop(&mut self) {
        let mut record = self
            .0
            .child
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(mut child) = record.take() {
            // This synthetic pipe has no actual child. Retire it explicitly so
            // unresolved-custody Drop does not intentionally leak the test FD.
            child.terminal_reaped();
        }
    }
}

#[test]
fn pending_cleanup_retains_its_descriptor_and_full_pool_capacity() {
    let reaper = DeferredReaperV1::new();
    let mut slots = reserve_pool(&reaper);
    let slot = slots.remove(MAX_PROTECTED_ISSUER_PROCESSES_V1 / 2);
    let cell = slot.cell;
    let (reader, _writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let descriptor = reader.as_raw_fd();
    let _cleanup = FakePipeCleanup(cell);
    // Invalid pidfd operations target only this pipe; the engine has no raw-PID fallback.
    slot.defer(ChildCleanupV1::new(Some(reader), getpid(), None));
    assert_eq!(cell.state.load(Ordering::Acquire), DEFERRED);

    for _ in 0..3 {
        reaper.pump();
        assert_eq!(cell.state.load(Ordering::Acquire), DEFERRED);
        let record = cell.child.lock().unwrap();
        let child = record.as_ref().expect("pending custody was discarded");
        assert_eq!(child.pid(), getpid());
        assert_eq!(child.pidfd().unwrap().as_raw_fd(), descriptor);
        assert!(fcntl_getfd(child.pidfd().unwrap()).is_ok());
        assert!(child.last_errno().is_some());
        assert_full(&reaper);
        assert!(
            slots
                .iter()
                .all(|slot| slot.cell.state.load(Ordering::Acquire) == RESERVED)
        );
    }
}

#[test]
fn missing_pidfd_quarantine_retains_capacity_across_every_pump() {
    let reaper = DeferredReaperV1::new();
    let mut slots = reserve_pool(&reaper);
    let slot = slots.remove(MAX_PROTECTED_ISSUER_PROCESSES_V1 / 2);
    let cell = slot.cell;
    slot.defer(ChildCleanupV1::new(None, getpid(), None));
    assert_eq!(cell.state.load(Ordering::Acquire), DEFERRED);

    for _ in 0..3 {
        reaper.pump();
        assert_eq!(cell.state.load(Ordering::Acquire), QUARANTINED);
        let record = cell.child.lock().unwrap();
        let child = record.as_ref().expect("quarantined custody was discarded");
        assert_eq!(child.pid(), getpid());
        assert!(child.pidfd().is_none());
        assert_eq!(child.last_errno(), None);
        assert_full(&reaper);
        assert!(
            slots
                .iter()
                .all(|slot| slot.cell.state.load(Ordering::Acquire) == RESERVED)
        );
    }
}

struct LiveChildGuard<'a> {
    child: Child,
    pidfd: Option<OwnedFd>,
    cell: &'a ReapCellV1,
    reaped: bool,
}

impl Drop for LiveChildGuard<'_> {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        // No worker exists: teardown takes exclusive wait custody after pumping stops.
        // Signal only the retained pidfd, never a scalar PID that could have been reused.
        if let Some(pidfd) = &self.pidfd {
            let _ = pidfd_send_signal(pidfd, Signal::KILL);
        }
        for _ in 0..MAX_POLLS {
            let terminal = match &self.pidfd {
                Some(pidfd) => waitid(
                    WaitId::PidFd(pidfd.as_fd()),
                    WaitIdOptions::EXITED | WaitIdOptions::NOHANG,
                )
                .map(|status| {
                    status.is_some_and(|status| {
                        matches!(
                            status.raw_code(),
                            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
                        )
                    })
                })
                .map_err(std::io::Error::from),
                // Setup failed before pidfd acquisition and before queue transfer.
                None => self.child.try_wait().map(|status| status.is_some()),
            };
            match terminal {
                Ok(true) => {
                    let mut record = self
                        .cell
                        .child
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    if let Some(mut child) = record.take() {
                        child.terminal_reaped();
                    }
                    self.cell.state.store(EMPTY, Ordering::Release);
                    return;
                }
                Err(error) if error.raw_os_error() == Some(libc::ECHILD) => return,
                Ok(false) | Err(_) => std::thread::sleep(POLL_DELAY),
            }
        }
        // Do not mark a real pending child terminal merely to dispose of the fixture.
        eprintln!("bounded live-child fixture cleanup exhausted; custody remains unresolved");
    }
}

#[test]
fn terminal_cleanup_reclaims_exactly_its_slot_after_spawn_lease_transfer() {
    let reaper = DeferredReaperV1::new();
    let mut slots = reserve_pool(&reaper);
    let slot = slots.remove(MAX_PROTECTED_ISSUER_PROCESSES_V1 / 2);
    let cell = slot.cell;
    let spawn_lease = try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let child = Command::new("/bin/true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = LiveChildGuard {
        child,
        pidfd: None,
        cell,
        reaped: false,
    };
    let pid = Pid::from_raw(i32::try_from(guard.child.id()).unwrap()).unwrap();
    guard.pidfd = Some(pidfd_open(pid, PidfdFlags::empty()).unwrap());
    let queued_pidfd = fcntl_dupfd_cloexec(guard.pidfd.as_ref().unwrap(), 0).unwrap();
    // The opaque public lease moves into queue custody; its private coordinator
    // count is covered by artifact-transaction tests, not observable in this module.
    slot.defer(ChildCleanupV1::new(
        Some(queued_pidfd),
        pid,
        Some(spawn_lease),
    ));
    assert_eq!(cell.state.load(Ordering::Acquire), DEFERRED);
    {
        let record = cell.child.lock().unwrap();
        let queued_cleanup = record.as_ref().unwrap();
        assert_eq!(queued_cleanup.pid(), pid);
        assert!(queued_cleanup.retains_spawn_lease());
    }
    assert_full(&reaper);

    for _ in 0..MAX_POLLS {
        reaper.pump();
        if cell.state.load(Ordering::Acquire) == EMPTY {
            break;
        }
        std::thread::sleep(POLL_DELAY);
    }
    assert_eq!(
        cell.state.load(Ordering::Acquire),
        EMPTY,
        "owned child did not reach terminal cleanup within the pump budget"
    );
    assert!(cell.child.lock().unwrap().is_none());
    assert_eq!(
        reaper
            .cells
            .iter()
            .filter(|cell| cell.state.load(Ordering::Acquire) == EMPTY)
            .count(),
        1
    );
    assert!(
        slots
            .iter()
            .all(|slot| slot.cell.state.load(Ordering::Acquire) == RESERVED)
    );
    assert!(matches!(
        waitid(
            WaitId::PidFd(guard.pidfd.as_ref().unwrap().as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(Errno::CHILD)
    ));
    guard.reaped = true;
    let replacement = reaper.reserve_slot().unwrap();
    assert!(std::ptr::eq(replacement.cell, cell));
    assert_full(&reaper);
    replacement.complete();
}
