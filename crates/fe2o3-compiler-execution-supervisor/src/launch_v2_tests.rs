use super::test_support::*;
use super::*;
use crate::AdmittedIssuerProgramV2 as Program;
use crate::authority_v2_test_process::{IO_TIMEOUT, frame, receive_packet, send_packet};
use crate::native_consuming_test_process::Family;
use fe2o3_broker_authority_service::{
    CURRENT_PROCESS_START_TIME_WORK_V2, LiveClientPidfdIdentityV2 as Client,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV2 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOperationV2 as Operation, ProtectedStaticExecutableV2 as Image,
};
use fe2o3_static_preexec_manifest::{PREEXEC_SOURCE_FD_BASE, StaticPreexecObjectClassV1 as Class};
use rustix::{
    fs::{Mode, OFlags},
    io::FdFlags,
};
use std::{
    collections::BTreeMap,
    os::fd::{AsFd, AsRawFd},
    time::Instant,
};

const WORK_LIMIT: usize = 100_000_000_000;
const STORAGE_LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;
const FAMILY: Family = Family::V2;

#[path = "native_consuming_v2_tests.rs"]
mod consuming;
pub(crate) use consuming::exercise as exercise_consuming;

include!("launch_native_witness_tests.rs");

impl HandoffWitness {
    fn expected_after_prepare(&self, client_pid: u32) -> BTreeMap<i32, FdIdentity> {
        let mut expected = fd_inventory();
        for witness in [&self.control, &self.service] {
            let pin = witness.pin.as_raw_fd();
            let object = (
                witness.object.device(),
                witness.object.inode(),
                witness.object.mode(),
            );
            assert_eq!(expected[&pin].object, object);
            let consumed: Vec<_> = expected
                .iter()
                .filter_map(|(&fd, identity)| {
                    (fd != pin && identity.object == object).then_some(fd)
                })
                .collect();
            assert_eq!(consumed.len(), 1, "exactly one consumed socket per witness");
            assert!(expected.remove(&consumed[0]).is_some());
        }
        // This target stays alive in the submitter fixture. Its sole local pidfd
        // belongs to Accepted; an anonymous-inode key cannot distinguish it.
        let client_pid = i32::try_from(client_pid).unwrap();
        let consumed: Vec<_> = expected
            .iter()
            .filter_map(|(&fd, identity)| (identity.pid == Some(client_pid)).then_some(fd))
            .collect();
        assert_eq!(consumed.len(), 1, "exactly one consumed client pidfd");
        assert!(expected.remove(&consumed[0]).is_some());
        expected
    }
}

fn expected_work(fixture: &crate::tests::Fixture) -> (usize, usize) {
    let m = fixture.measurement();
    let measurement = Measurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024).unwrap();
    let s = Supervisor::WORK + crate::authority_v2::tests::nested_work(fixture);
    let h = Accepted::WORK + s + MANIFEST_WORK + Client::REVALIDATION_WORK;
    let t = 3 * Program::WORK
        + 2 * Image::quota(measurement, Operation::Transfer)
            .unwrap()
            .work()
        + PolicyCap::IO_WORK
        + Key::IO_WORK;
    let c = s + t + Anchor::CLONE_TRANSFER_WORK;
    let r = s + t + Anchor::VALIDATE_TRANSFER_WORK;
    let pc = Accepted::WORK + Client::CLONE_TRANSFER_WORK;
    let pv = Accepted::WORK + Client::VALIDATE_TRANSFER_WORK;
    let k = Capability::IO_WORK;
    let p = CURRENT_PROCESS_START_TIME_WORK_V2;
    // Expand the production call graph, independently of observed work deltas.
    let check = s + h + 2 * k + p + r + pv;
    let revalidate = Prepared::WORK + check;
    let prepare = Prepared::WORK + s + h + c + pc + MANIFEST_WORK + 2 * k + p + check;
    (prepare, revalidate)
}

fn prepared_witnesses(owner: &Prepared) -> Vec<Witness> {
    prepared_witnesses_with_readiness(owner, true)
}

fn inspect_prepared(owner: &Prepared, client_pid: u32, anchor_pid: u32) {
    let manifest = owner.static_manifest();
    assert_eq!(manifest.parent_pid(), std::process::id() as i32);
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let start_time: u64 = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(start_time, 0);
    assert_eq!(manifest.parent_start_time(), start_time);
    assert_eq!(manifest.descriptors().len(), 12);
    assert_eq!(SOURCE_COUNT_V1, 12);
    assert_eq!(DESTINATION_FDS_V1, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert_eq!(
        manifest.executable(),
        &checks::object_identity(&owner.issuer, "issuer").unwrap()
    );
    let objects = checks::source_identities(&owner.sources).unwrap();
    for (index, entry) in manifest.descriptors().iter().enumerate() {
        assert_eq!(entry.source_fd(), PREEXEC_SOURCE_FD_BASE + index as i32);
        assert_eq!(entry.destination_fd(), index as i32);
        assert_eq!(entry.object(), &objects[index]);
        assert_eq!(
            entry.object().class(),
            if matches!(
                index,
                CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
            ) {
                Class::ProcessPidfd
            } else {
                Class::Fstat
            }
        );
    }
    assert_eq!(owner.service_manifest().client().pid(), client_pid);
    for (index, pid) in [
        (CLIENT_PIDFD_SOURCE_INDEX, client_pid),
        (EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX, anchor_pid),
    ] {
        let info = std::fs::read_to_string(format!(
            "/proc/self/fdinfo/{}",
            rustix::path::DecInt::from_fd(&owner.sources[index]).as_str()
        ))
        .unwrap();
        let expected = format!("Pid:\t{pid}");
        assert!(info.lines().any(|line| line == expected));
    }
    // Stdin's writer is deliberately not retained; it is already at EOF.
    assert_eq!(
        rustix::io::read(&owner.sources[STDIN_SOURCE_INDEX], &mut [0; 1]).unwrap(),
        0
    );
    for (writer, reader) in [
        (&owner.sources[STDOUT_SOURCE_INDEX], &owner.stdout_reader),
        (&owner.sources[STDERR_SOURCE_INDEX], &owner.stderr_reader),
        (
            &owner.sources[READINESS_SOURCE_INDEX],
            &owner.readiness_reader,
        ),
    ] {
        assert_eq!(rustix::io::write(writer, b"prepared").unwrap(), 8);
        let mut bytes = [0; 8];
        assert_eq!(rustix::io::read(reader, &mut bytes).unwrap(), 8);
        assert_eq!(&bytes, b"prepared");
    }
    image::validate(&owner.static_manifest_file, manifest, owner.manifest_object).unwrap();
    let debug = format!("{owner:?}");
    assert!(debug.contains("prepared-custody-only"));
    assert!(!debug.contains("OwnedFd"));
}

include!("launch_native_limits_tests.rs");

fn preparation_boundaries(
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    outer: &mut Budget<'_>,
    exact_work: usize,
) {
    let mut peak = 0;
    for case in LIMITS {
        let (accepted, control, pid, pidfds) = accept_with_witness(supervisor, submitter, outer);
        let consumed = accepted.retained_storage();
        let floor = supervisor.retained_storage() + consumed;
        let (work_limit, storage_limit, prepaid) = case.inputs(floor, exact_work, peak);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        assert_eq!(
            pidfds, 0,
            "the fixture client target has no other local pidfds"
        );
        let expected_descriptors = control.expected_after_prepare(pid);
        let result = supervisor.prepare_launch(accepted, &mut budget);
        assert_eq!(budget.storage(), prepaid, "{case:?}");
        if case.succeeds() {
            let (owner, delta) = result.unwrap();
            assert_eq!(budget.work(), exact_work);
            assert_eq!(
                owner.retained_storage(),
                consumed + delta.additional_storage()
            );
            if matches!(case, Limit::Roomy) {
                peak = budget.peak_storage();
            }
            assert_eq!(
                budget.peak_storage(),
                peak - if matches!(case, Limit::ExactFloor) {
                    EXTRA
                } else {
                    0
                }
            );
            budget.reserve_storage(delta.additional_storage()).unwrap();
            let retained = owner.retained_storage();
            let witnesses = prepared_witnesses(&owner);
            drop(owner);
            budget.release_storage(retained).unwrap();
            for witness in witnesses {
                witness.assert_released();
            }
        } else {
            case.check_error(&result.unwrap_err(), &budget, peak);
            budget.release_storage(consumed).unwrap();
        }
        assert_eq!(budget.storage(), prepaid - consumed);
        assert_eq!(work.failed_work(), case.failed_work(exact_work), "{case:?}");
        // Compare before opening another protocol session or dropping the pins.
        // A leaked staging FD still fails if it reused a consumed descriptor number.
        assert_eq!(
            fd_inventory(),
            expected_descriptors,
            "prepared cleanup {case:?}"
        );
        outer.release_storage(consumed).unwrap();
        control.assert_released();
        assert_eq!(pidfd_references(pid), pidfds, "consumed handoff {case:?}");
        let floor = outer.storage();
        supervisor.revalidate(outer).unwrap();
        assert_eq!(outer.storage(), floor);
    }
}

include!("launch_native_revalidation_tests.rs");

/// Only the existing real distinct-UID fixture calls this; no local admission shortcut.
pub(crate) fn exercise(peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    assert_eq!(rustix::process::geteuid().as_raw(), 65_533);
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (fixture, supervisor) = crate::authority_v2::tests::bound_fixture(peer, pidfd, &mut budget);
    let (expected_prepare_work, expected_revalidate_work) = expected_work(&fixture);
    let supervisor_storage = supervisor.retained_storage();
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    let anchor_pid = supervisor.external_anchor_process().pid();
    send_packet(submitter, &frame(b"ANC2", anchor_pid), &[]).unwrap();
    let (accepted, control, pid, pidfds) = accept_with_witness(&supervisor, submitter, &mut budget);
    let anchor_pidfds = pidfd_references(anchor_pid);
    let expected_manifest = *accepted.manifest().canonical_bytes();
    let consumed = accepted.retained_storage();
    let floor = budget.storage();
    let before = budget.work();
    let (mut owner, delta) = supervisor.prepare_launch(accepted, &mut budget).unwrap();
    let prepare_work = budget.work() - before;
    assert_eq!(prepare_work, expected_prepare_work);
    assert!(prepare_work > Prepared::WORK);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        owner.retained_storage(),
        consumed + delta.additional_storage()
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(
        budget.storage(),
        EXTRA + supervisor_storage + owner.retained_storage()
    );
    assert_eq!(
        owner.service_manifest().canonical_bytes(),
        &expected_manifest
    );
    assert_eq!(pidfd_references(pid), pidfds + 2);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds + 1);
    inspect_prepared(&owner, pid, anchor_pid);
    let before = budget.work();
    owner.revalidate(&supervisor, &mut budget).unwrap();
    let revalidate_work = budget.work() - before;
    assert_eq!(revalidate_work, expected_revalidate_work);
    assert!(revalidate_work > Prepared::WORK);
    assert!(prepare_work > revalidate_work);
    revalidation_boundaries(&owner, &supervisor, expected_revalidate_work);
    mutations(&mut owner, &supervisor, &mut budget);
    let witnesses = prepared_witnesses(&owner);
    let retained = owner.retained_storage();
    drop(owner);
    budget.release_storage(retained).unwrap();
    for witness in witnesses {
        witness.assert_released();
    }
    control.assert_released();
    drop(control);
    assert_eq!(pidfd_references(pid), pidfds);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);

    preparation_boundaries(&supervisor, submitter, &mut budget, expected_prepare_work);
    let (accepted, control, pid, pidfds) = accept_with_witness(&supervisor, submitter, &mut budget);
    let consumed = accepted.retained_storage();
    let mut denied_work = Work::new(WORK_LIMIT);
    let mut denied = Budget::new(&mut denied_work, STORAGE_LIMIT);
    assert!(denied.charge_work(WORK_LIMIT + 1).is_err());
    assert!(denied.reserve_storage(STORAGE_LIMIT + 1).is_err());
    denied
        .reserve_storage(EXTRA + supervisor_storage + consumed)
        .unwrap();
    let (owner, delta) = supervisor.prepare_launch(accepted, &mut denied).unwrap();
    denied.reserve_storage(delta.additional_storage()).unwrap();
    owner.revalidate(&supervisor, &mut denied).unwrap();
    assert_eq!(denied.work(), prepare_work + revalidate_work);
    let retained = owner.retained_storage();
    drop(owner);
    denied.release_storage(retained).unwrap();
    assert_eq!(denied.storage(), EXTRA + supervisor_storage);
    assert_eq!(denied.failed_storage(), Some(STORAGE_LIMIT + 1));
    assert_eq!(denied_work.failed_work(), Some(WORK_LIMIT + 1));
    budget.release_storage(consumed).unwrap();
    control.assert_released();
    drop(control);
    assert_eq!(pidfd_references(pid), pidfds);
    assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
    drop(supervisor);
    budget.release_storage(supervisor_storage).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
    drop(fixture);
}

include!("launch_native_io_tests.rs");
