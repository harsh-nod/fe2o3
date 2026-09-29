//! Actual artifact/OS-lock tests, not protected compiler observation or proof.
use super::*;
use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    publish_compiler_module_handoff_v5 as publish,
    try_acquire_artifact_process_spawn_lease_v1 as spawn_lease, with_artifact_process_spawn_v1,
};
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5 as decode_work,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    os::fd::AsRawFd,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    time::{Duration, Instant},
};

const LIMIT: usize = fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;
const BYTES: &[u8] = include_bytes!("../tests/fixtures/locked-publication-v5.bin");

fn isolated() -> bool {
    const CHILD: &str = "FE2O3_ROOT_PUBLICATION_LOCK_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return false;
    }
    let name = concat!(
        module_path!(),
        "::actual_pair_retains_locks_on_refusal_and_unwind"
    );
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(
            name.strip_prefix("fe2o3_broker_authority_service::")
                .unwrap(),
        )
        .arg("--nocapture")
        .env(CHILD, "1");
    let mut child = with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated publication lock test: {status}");
            return true;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("isolated publication lock test timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn kernel_lock(path: &Path, held: bool) {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.join(".fe2o3-artifacts.lock"))
        .unwrap();
    // SAFETY: a zeroed flock has no invalid representations. Its initialized
    // fields request a nonblocking OFD lock on this live, distinct open description.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) };
    if held {
        assert_eq!(result, -1);
        assert!(matches!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EACCES | libc::EAGAIN)
        ));
    } else {
        assert_eq!(result, 0);
    }
}

#[test]
fn actual_pair_retains_locks_on_refusal_and_unwind() {
    if isolated() {
        return;
    }
    for mode in ["success", "scope-refusal", "unwind", "token-work-refusal"] {
        let directory = tempfile::tempdir().unwrap();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(BYTES.len() + METADATA).unwrap();
        b.charge_work(decode_work(BYTES.len()).unwrap()).unwrap();
        let handoff = Handoff::decode_owned(BYTES.to_vec()).unwrap();
        assert_eq!(handoff.backing_capacity(), BYTES.len());
        let producer =
            ProducerIdentity::from_codegen("conditional", Some(Path::new("/conditional.rs")))
                .unwrap();
        let attempt = begin_build_attempt(
            directory.path(),
            &producer,
            BuildInvocation::from_bytes([3; 32]),
            BuildSession::from_bytes([4; 16]),
        )
        .unwrap();
        let receipt = publish(directory.path(), &producer, attempt, &handoff, &mut b).unwrap();
        let quote = custody_quote(directory.path(), &producer, receipt).unwrap();
        let bound = quote.retained_storage();
        let cleanup = Holder::<Owners>::quota(bound + size_of::<Owners>()).unwrap();
        assert!(cleanup.retirement_work() < Owners::RETIRE_WORK_PER_BYTE * LIMIT);
        b.reserve_storage(bound + size_of::<Owners>()).unwrap();
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let lease_work = if mode == "token-work-refusal" {
            let barrier = retirement_barrier().unwrap();
            let before = b.work();
            let (lease, charge) =
                acquire_quoted(directory.path(), &producer, &quote, &barrier, &mut b).unwrap();
            let work = b.work() - before;
            b.reserve_storage(charge.retained_storage()).unwrap();
            drop(lease);
            b.release_storage(charge.retained_storage()).unwrap();
            work
        } else {
            0
        };
        let mut owners = Owners::new(quote);
        let result = catch_unwind(AssertUnwindSafe(|| {
            b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| -> Result<()> {
                let entry = b.storage();
                if mode == "token-work-refusal" {
                    b.charge_work(usize::MAX - b.work() - lease_work)?;
                }
                owners.acquire(directory.path(), &producer, receipt, b)?;
                if mode == "unwind" {
                    panic!("post-acquisition fixture unwind");
                }
                if mode == "scope-refusal" {
                    b.release_storage(b.storage() - entry + 1)?;
                }
                Ok(())
            })
        }));
        match mode {
            "success" => result.unwrap().unwrap(),
            "scope-refusal" => assert!(matches!(
                result,
                Ok(Err(RootPublicationCustodyErrorV3(Failure::Occurrence(
                    NativeOccurrenceError::Resource(Resource::Accounting)
                ))))
            )),
            "token-work-refusal" => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        let locked = mode != "token-work-refusal";
        if locked {
            let (lease, token) = owners.current().unwrap();
            lease.validate_current_token(token).unwrap();
            assert!(
                lease.storage().retained_storage() + token.storage().retained_storage() <= bound
            );
        } else {
            assert!(owners.publication.is_some());
            assert!(owners.token.is_none());
            assert_eq!(b.work(), usize::MAX);
        }
        kernel_lock(directory.path(), locked);
        assert!(
            owners
                .acquire(directory.path(), &producer, receipt, &mut b)
                .is_err()
        );
        kernel_lock(directory.path(), locked);

        let competing = spawn_lease().unwrap();
        assert!(owners.try_prepare_retirement().is_none());
        kernel_lock(directory.path(), locked);
        drop(competing);
        drop(owners.try_prepare_retirement().unwrap());
        kernel_lock(directory.path(), locked);
        let prepared = owners.try_prepare_retirement().unwrap();
        assert!(spawn_lease().is_err());
        owners.retire(prepared);
        kernel_lock(directory.path(), false);
        drop(spawn_lease().unwrap());
    }
}

#[test]
fn concrete_publication_payload_and_barrier_are_send_but_not_observation_custody() {
    fn send<T: Send + 'static>() {}
    send::<Owners>();
    send::<Barrier>();
    assert!(Owners::RETIRE_SCRATCH >= size_of::<Barrier>());
    assert!(Owners::RETIRE_WORK > 0);
}
