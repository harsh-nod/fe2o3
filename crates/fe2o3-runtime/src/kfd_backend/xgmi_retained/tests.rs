use super::*;
use std::cell::Cell;
use std::rc::Rc;

type TestPhase = BatchPhase<Box<u64>, Box<u64>, Box<u64>, Box<u64>>;

#[test]
fn retained_native_publish_once_preserves_pending_ticket_identity() {
    for count in [1, 2, 63] {
        let requests: Vec<_> = (0..count).map(Box::new).collect();
        let pointers: Vec<_> = requests
            .iter()
            .map(|value| &**value as *const u64)
            .collect();
        let calls = Cell::new(0);
        let phase: TestPhase = publish_once(BatchPhase::Prepared(requests), |requests| {
            calls.set(calls.get() + 1);
            Ok(requests)
        });
        let phase = publish_once(phase, |_| panic!("pending work was republished"));
        let BatchPhase::Pending(tickets) = phase else {
            panic!("missing exact tickets")
        };
        assert_eq!(calls.get(), 1);
        assert_eq!(
            tickets
                .iter()
                .map(|value| &**value as *const u64)
                .collect::<Vec<_>>(),
            pointers
        );
    }
}

#[test]
fn retained_native_publish_once_never_retries_terminal_or_ready_phases() {
    let error = Box::new(19);
    let address = &*error as *const u64;
    let phase: TestPhase = publish_once(BatchPhase::Prepared(vec![Box::new(1)]), |_| Err(error));
    let phase = publish_once(phase, |_| panic!("failed publication was retried"));
    let BatchPhase::SubmitFailure(error) = phase else {
        panic!("failure lost")
    };
    assert_eq!(&*error as *const u64, address);
    let phase: TestPhase = publish_once(BatchPhase::WaitFailure(error), |_| {
        panic!("wait failure retried")
    });
    let BatchPhase::WaitFailure(error) = phase else {
        panic!("wait failure lost")
    };
    assert_eq!(&*error as *const u64, address);
    let phase: TestPhase = publish_once(BatchPhase::Ready, |_| panic!("ready work republished"));
    assert!(matches!(phase, BatchPhase::Ready));
}

#[test]
fn retained_native_pending_retries_keep_exact_tickets_deadline_and_single_publication() {
    struct Script {
        submits: usize,
        deadlines: Vec<Instant>,
        pending: bool,
    }
    impl Work for Script {
        type Request = Box<u64>;
        type Ticket = Box<u64>;
        type SubmitFailure = Vec<Box<u64>>;
        type WaitFailure = Vec<Box<u64>>;
        type Completed = Vec<Box<u64>>;
        fn submit(&mut self, requests: Vec<Box<u64>>) -> Result<Vec<Box<u64>>, Vec<Box<u64>>> {
            self.submits += 1;
            Ok(requests)
        }
        fn wait(
            &mut self,
            tickets: Vec<Box<u64>>,
            deadline: Instant,
        ) -> Result<Vec<Box<u64>>, Vec<Box<u64>>> {
            self.deadlines.push(deadline);
            if self.pending {
                Err(tickets)
            } else {
                Ok(tickets)
            }
        }
    }
    let deadline = Instant::now();
    let requests = vec![Box::new(11), Box::new(22)];
    let pointers: Vec<_> = requests
        .iter()
        .map(|value| &**value as *const u64)
        .collect();
    let mut phase: WorkPhase<Script> = BatchPhase::Prepared(requests);
    let mut script = Script {
        submits: 0,
        deadlines: Vec::new(),
        pending: true,
    };
    for _ in 0..4 {
        let Advanced::Waited(Err(tickets)) = advance(phase, deadline, &mut script) else {
            panic!("timeout must return the exact pending tickets")
        };
        assert_eq!(
            tickets
                .iter()
                .map(|value| &**value as *const u64)
                .collect::<Vec<_>>(),
            pointers
        );
        phase = BatchPhase::Pending(tickets);
        assert!(!phase.ready_to_finish());
    }
    script.pending = false;
    let Advanced::Waited(Ok(completed)) = advance(phase, deadline, &mut script) else {
        panic!("expected completed roster")
    };
    assert_eq!(
        completed
            .iter()
            .map(|value| &**value as *const u64)
            .collect::<Vec<_>>(),
        pointers
    );
    assert_eq!(script.submits, 1);
    assert_eq!(script.deadlines, vec![deadline; 5]);
}

#[test]
fn retained_native_finish_admission_accepts_only_ready_never_pending_or_terminal() {
    let phases: [TestPhase; 5] = [
        BatchPhase::Prepared(vec![Box::new(1)]),
        BatchPhase::Pending(vec![Box::new(2)]),
        BatchPhase::Ready,
        BatchPhase::SubmitFailure(Box::new(3)),
        BatchPhase::WaitFailure(Box::new(4)),
    ];
    for (index, phase) in phases.iter().enumerate() {
        assert_eq!(phase.ready_to_finish(), index == 2);
    }
    let source = include_str!("../xgmi_retained.rs");
    let finish = source
        .split("pub(crate) fn finish_retained_peer_batch_v1(")
        .nth(1)
        .unwrap();
    assert!(
        finish.find("if !retained.phase.ready_to_finish()").unwrap()
            < finish
                .find("self.abandon_retained_peer_batch_v1()")
                .unwrap()
    );
    assert!(
        finish
            .find("self.abandon_retained_peer_batch_v1()")
            .unwrap()
            < finish.find("owner.finish()").unwrap()
    );
}

#[test]
fn retained_native_transfer_guard_returns_exact_owner_without_drop() {
    struct Probe(Rc<Cell<usize>>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let value = Box::new(Probe(Rc::clone(&drops)));
    let address = &*value as *const Probe;
    let returned = TransferGuard::new(value).take();
    assert_eq!(&*returned as *const Probe, address);
    assert_eq!(drops.get(), 0);
    drop(returned);
    assert_eq!(drops.get(), 1);
}

#[cfg(unix)]
#[test]
fn retained_native_transfer_guard_aborts_before_resource_drop_on_unwind() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_RETAINED_TRANSFER_ABORT_CHILD";
    if std::env::var_os(CHILD).is_some() {
        struct Probe;
        impl Drop for Probe {
            fn drop(&mut self) {
                std::process::exit(71);
            }
        }
        let _guard = TransferGuard::new(Probe);
        panic!("transfer interrupted");
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "kfd_backend::xgmi_retained::tests::retained_native_transfer_guard_aborts_before_resource_drop_on_unwind", "--nocapture"])
        .env(CHILD, "1").output().unwrap();
    assert_eq!(
        output.status.signal(),
        Some(6),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn retained_native_production_path_keeps_profile_deadline_and_failures_distinct() {
    let source = include_str!("../xgmi_retained.rs");
    assert!(source.contains("Gfx942NativeXgmiSdmaOwnedRetainedPairV1::begin("));
    let compact: String = source.split_whitespace().collect();
    assert!(compact.contains("advance(phase,retained.deadline,"));
    assert!(source.contains("self.owner.wait_batch_until(tickets, deadline)"));
    assert!(!source.contains("wait_batch_for("));
    assert!(!source.contains(".into_retained_tickets()"));
    assert!(source.contains("Err(failure) => Phase::WaitFailure(failure)"));
    assert!(
        source.find("owner.finish()").unwrap()
            < source
                .find("self.commit_batch_status(&ids, BackendPollV1::Succeeded)")
                .unwrap()
    );
}
