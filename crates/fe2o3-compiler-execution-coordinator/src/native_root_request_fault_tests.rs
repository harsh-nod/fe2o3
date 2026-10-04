//! Denial-only instrumentation. Only an actual native phase calls checkpoint;
//! arming a fault creates no Prepared, approval, helper, channel or trace owner.
use super::*;
use std::{cell::RefCell, os::fd::AsRawFd, os::unix::fs::MetadataExt};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Observation {
    pub phase: &'static str,
    pub action: &'static str,
    pub work: usize,
    pub storage: usize,
    pub peak: usize,
    pub failed_work: Option<usize>,
    pub failed_storage: Option<usize>,
}

struct Armed {
    phase: &'static str,
    action: &'static str,
    ledger: Ledger,
    address: usize,
    rights: [Option<(i32, u64, u64)>; 6],
    observed: Option<Observation>,
}

thread_local! {
    static ARMED: RefCell<Option<Armed>> = const { RefCell::new(None) };
}

pub(super) fn arm(receiver: &Receiver, phase: &'static str, action: &'static str, b: &Budget<'_>) {
    assert!(matches!(
        phase,
        "helper-ready" | "compiler-profile" | "compiler-channel" | "compiler-trace"
    ));
    assert!(matches!(action, "work" | "storage" | "unwind"));
    assert!(b.failed_work().is_none() && b.failed_storage().is_none());
    let rights = receiver.files.each_ref().map(|file| {
        file.as_ref().map(|file| {
            let stat = fs::fstat(file).unwrap();
            (file.as_raw_fd(), stat.st_dev, stat.st_ino)
        })
    });
    ARMED.with(|cell| {
        assert!(
            cell.borrow().is_none(),
            "one consuming fault per genuine subprocess"
        );
        *cell.borrow_mut() = Some(Armed {
            phase,
            action,
            ledger: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
            rights,
            observed: None,
        });
    });
}

pub(super) fn observation() -> Observation {
    ARMED.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .observed
            .expect("actual native checkpoint must be reached")
    })
}

pub(super) fn checkpoint(
    phase: &'static str,
    pid: rustix::process::Pid,
    b: &mut Budget<'_>,
) -> std::result::Result<(), Resource> {
    ARMED.with(|cell| {
        let mut cell = cell.borrow_mut();
        let Some(armed) = cell.as_mut().filter(|armed| armed.phase == phase) else {
            return Ok(());
        };
        assert!(
            armed.observed.is_none(),
            "a failed native phase cannot be retried"
        );
        assert!(b.work_ledger_identity_v1() == armed.ledger);
        assert_eq!(b as *const Budget<'_> as usize, armed.address);
        assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        for (fd, dev, ino) in armed.rights.into_iter().flatten() {
            let actual = std::fs::metadata(format!("/proc/self/fd/{fd}")).unwrap();
            assert_eq!((actual.dev(), actual.ino()), (dev, ino));
        }
        let actual = std::fs::metadata(format!("/proc/{}/exe", pid.as_raw_pid())).unwrap();
        let parent = std::fs::metadata("/proc/self/exe").unwrap();
        if phase == "helper-ready" {
            assert_ne!((actual.dev(), actual.ino()), (parent.dev(), parent.ino()));
        } else {
            assert_eq!(
                (actual.dev(), actual.ino()),
                (parent.dev(), parent.ino()),
                "compiler exec gate must remain closed"
            );
        }
        if matches!(phase, "compiler-channel" | "compiler-trace") {
            let status =
                std::fs::read_to_string(format!("/proc/{}/status", pid.as_raw_pid())).unwrap();
            let tracer: u32 = status
                .lines()
                .find_map(|line| line.strip_prefix("TracerPid:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            if phase == "compiler-channel" {
                assert_eq!(tracer, 0, "channel checkpoint must precede trace seizure");
                eprintln!(
                    "ROOT_REQUEST_COMPILER_CHANNEL_PRETRACE_GATE_CLOSED pid={} tracer={tracer}",
                    pid.as_raw_pid()
                );
            } else {
                let current = std::fs::read_link("/proc/thread-self").unwrap();
                let tid: u32 = current
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(tracer, tid, "original creator must own the actual trace");
            }
        }
        // Accepted work persists even though the following operation refuses.
        b.charge_work(1).unwrap();
        let result = match armed.action {
            "work" => b.charge_work(usize::MAX),
            "storage" => b.reserve_storage(b.storage_limit()),
            "unwind" => Ok(()),
            _ => unreachable!(),
        };
        armed.observed = Some(Observation {
            phase,
            action: armed.action,
            work: b.work(),
            storage: b.storage(),
            peak: b.peak_storage(),
            failed_work: b.failed_work(),
            failed_storage: b.failed_storage(),
        });
        eprintln!(
            "ROOT_REQUEST_POSTCLONE_REACHED phase={phase} kind={} pid={}",
            armed.action,
            pid.as_raw_pid()
        );
        if armed.action == "unwind" {
            panic!("unwind at genuine native phase {phase}");
        }
        assert!(
            result.is_err(),
            "denial-only hook cannot authorize continuation"
        );
        result
    })
}
