use super::*;
use crate::AdmittedIssuerProgramV3 as Program;
use crate::authority_v2_test_process::*;
use crate::handoff_v3::tests::fixture;
use crate::launch_v2::test_support::*;
use fe2o3_broker_authority_service::{
    CURRENT_PROCESS_START_TIME_WORK_V2, LiveClientPidfdIdentityV2 as Client,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV3 as Key,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOperationV2 as Operation, ProtectedStaticExecutableV2 as Image,
};
use fe2o3_static_preexec_manifest::StaticPreexecObjectClassV1 as Class;
use rustix::{
    fs::{Mode, OFlags},
    io::FdFlags,
};
use std::{os::fd::AsFd, time::Instant};

const WORK_LIMIT: usize = 100_000_000_000;
const STORAGE_LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;

include!("launch_native_io_tests.rs");
include!("launch_native_limits_tests.rs");
include!("launch_native_revalidation_tests.rs");

// Real distinct-UID transport and native admission, including the growth reservation.
pub(crate) fn accept(supervisor: &Supervisor, submitter: &OwnedFd, b: &mut Budget<'_>) -> Accepted {
    let control = fixture::request(submitter, 0);
    b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let floor = b.storage();
    let (accepted, delta) = supervisor.accept_handoff(control, IO_TIMEOUT, b).unwrap();
    assert_eq!(b.storage(), floor);
    b.reserve_storage(delta.additional_storage()).unwrap();
    accepted
}

fn expected_work(fixture: &crate::tests::Fixture) -> (usize, usize) {
    let m = fixture.measurement();
    let measurement = Measurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024).unwrap();
    let s = Supervisor::WORK
        + crate::program_v3::tests::revalidation_work(fixture)
        + Key::IO_WORK
        + Anchor::REVALIDATION_WORK;
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
    // Expand the native call graph independently of measured work.
    let check = s + h + 2 * k + p + r + pv;
    (
        Prepared::WORK + s + h + c + pc + MANIFEST_WORK + 2 * k + p + check,
        Prepared::WORK + check,
    )
}

fn preparation_boundaries(
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    outer: &mut Budget<'_>,
    exact_work: usize,
) {
    let mut peak = 0;
    for case in LIMITS {
        let descriptors = fd_inventory();
        let accepted = accept(supervisor, submitter, outer);
        let consumed = accepted.retained_storage();
        let floor = supervisor.retained_storage() + consumed;
        let (work_limit, storage_limit, prepaid) = case.inputs(floor, exact_work, peak);
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        let account = b.work_ledger_identity_v1();
        b.reserve_storage(prepaid).unwrap();
        let result = supervisor.prepare_launch(accepted, &mut b);
        assert_eq!(b.storage(), prepaid, "{case:?}");
        assert!(b.work_ledger_identity_v1() == account);
        if case.succeeds() {
            let (owner, delta) = result.unwrap();
            assert_eq!(
                owner.retained_storage(),
                consumed + delta.additional_storage()
            );
            assert_eq!(b.work(), exact_work);
            if matches!(case, Limit::Roomy) {
                peak = b.peak_storage();
            }
            assert_eq!(
                b.peak_storage(),
                peak - if matches!(case, Limit::ExactFloor) {
                    EXTRA
                } else {
                    0
                }
            );
            assert_eq!(b.failed_storage(), None);
            drop(owner);
        } else {
            case.check_error(&result.unwrap_err(), &b, peak);
        }
        assert_eq!(work.failed_work(), case.failed_work(exact_work));
        outer.release_storage(consumed).unwrap();
        assert_eq!(fd_inventory(), descriptors, "prepare {case:?}");
        supervisor.revalidate(outer).unwrap();
    }
}

fn mismatched_manifest(owner: &mut Prepared, supervisor: &Supervisor, b: &mut Budget<'_>) {
    use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1 as Identity;
    let client = owner.service_manifest().client();
    let (manifest, delta) = Manifest::new(
        Identity::new(client.pid(), client.uid() + 1, client.gid()).unwrap(),
        owner.service_manifest().external_anchor_service(),
        supervisor.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (capability, delta) = Capability::create(manifest, b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let retained = capability.retained_storage();
    let original = std::mem::replace(&mut owner.launch_capability, capability);
    assert!(matches!(
        refused(owner, supervisor, b),
        Error::LaunchManifestMismatch
    ));
    drop(std::mem::replace(&mut owner.launch_capability, original));
    b.release_storage(retained).unwrap();
    owner.revalidate(supervisor, b).unwrap();
}

fn preparation_refusal(
    fixture: &crate::tests::Fixture,
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    b: &mut Budget<'_>,
) {
    let descriptors = fd_inventory();
    let accepted = accept(supervisor, submitter, b);
    let consumed = accepted.retained_storage();
    let root = File::open(&fixture.root).unwrap();
    let mode = Mode::from_raw_mode(rustix::fs::fstat(&root).unwrap().st_mode);
    rustix::fs::fchmod(&root, mode | Mode::WGRP).unwrap();
    let floor = b.storage();
    let result = supervisor.prepare_launch(accepted, b);
    rustix::fs::fchmod(&root, mode).unwrap();
    drop(root);
    assert!(matches!(result, Err(Error::Supervisor(_))));
    assert_eq!(b.storage(), floor);
    b.release_storage(consumed).unwrap();
    assert_eq!(fd_inventory(), descriptors);
    supervisor.revalidate(b).unwrap();
}

#[test]
#[ignore = "private distinct-UID V3 preparation helper, run only by the root coordinator"]
fn supervisor_process_helper() {
    require_child_credentials("handoff-supervisor-v3", 65_533);
    let control = inherited_control();
    let (payload, [peer, pidfd, submitter]) =
        receive_packet::<3>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"ANC2");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let account = b.work_ledger_identity_v1();
    b.reserve_storage(EXTRA).unwrap();
    let (fixture, supervisor) = fixture::supervisor(&peer, &pidfd, &mut b);
    let supervisor_storage = supervisor.retained_storage();
    let (prepare_work, revalidate_work) = expected_work(&fixture);
    let descriptors = fd_inventory();
    let accepted = accept(&supervisor, &submitter, &mut b);
    let consumed = accepted.retained_storage();
    let expected = *accepted.manifest().canonical_bytes();
    let floor = b.storage();
    let start = b.work();
    let (mut owner, delta) = supervisor.prepare_launch(accepted, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - start, prepare_work);
    assert_eq!(
        owner.retained_storage(),
        consumed + delta.additional_storage()
    );
    b.reserve_storage(delta.additional_storage()).unwrap();
    let _: &Manifest = owner.service_manifest();
    let _: &Capability = &owner.launch_capability;
    assert_eq!(owner.service_manifest().canonical_bytes(), &expected);
    assert!(
        owner
            .service_manifest()
            .matches_policy(supervisor.policy(), &mut b)
            .unwrap()
    );
    let staged = owner.staged_input();
    assert!(std::ptr::eq(staged.launcher, &owner.launcher));
    assert!(std::ptr::eq(staged.issuer, &owner.issuer));
    assert!(std::ptr::eq(staged.manifest, &owner.static_manifest_file));
    assert!(std::ptr::eq(staged.sources, &owner.sources));
    let start = b.work();
    owner.revalidate(&supervisor, &mut b).unwrap();
    assert_eq!(b.work() - start, revalidate_work);
    revalidation_boundaries(&owner, &supervisor, revalidate_work);
    mutations(&mut owner, &supervisor, &mut b);
    mismatched_manifest(&mut owner, &supervisor, &mut b);

    let retained = owner.retained_storage();
    let mut launched: LaunchedInputsV3 = owner.into_launched();
    let _: &fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV3 =
        &launched.capability;
    assert!(launched.control.is_some());
    assert!(launched.readiness_reader.is_some());
    // No child was created: consuming custody closes every parent pipe writer.
    let readiness = launched.readiness_reader.take().unwrap();
    assert_eq!(rustix::io::read(&readiness, &mut [0; 1]).unwrap(), 0);
    assert_eq!(
        rustix::io::read(&launched._stdout_reader, &mut [0; 1]).unwrap(),
        0
    );
    assert_eq!(
        rustix::io::read(&launched._stderr_reader, &mut [0; 1]).unwrap(),
        0
    );
    launched.capability.revalidate(&mut b).unwrap();
    let control_fd = launched.control.take().unwrap();
    drop((control_fd, readiness, launched));
    b.release_storage(retained).unwrap();
    assert_eq!(fd_inventory(), descriptors);
    assert_eq!(b.storage(), EXTRA + supervisor_storage);

    preparation_boundaries(&supervisor, &submitter, &mut b, prepare_work);
    preparation_refusal(&fixture, &supervisor, &submitter, &mut b);

    // Prior refusals stay on the exact account supplied to both operations.
    assert!(b.charge_work(WORK_LIMIT + 1).is_err());
    let denied_work = b.failed_work();
    assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
    let denied_storage = b.failed_storage();
    let accepted = accept(&supervisor, &submitter, &mut b);
    let (owner, delta) = supervisor.prepare_launch(accepted, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    owner.revalidate(&supervisor, &mut b).unwrap();
    let retained = owner.retained_storage();
    drop(owner);
    b.release_storage(retained).unwrap();
    assert_eq!(b.failed_work(), denied_work);
    assert_eq!(b.failed_storage(), denied_storage);
    assert_eq!(fd_inventory(), descriptors);
    assert!(b.work_ledger_identity_v1() == account);
    drop(supervisor);
    b.release_storage(supervisor_storage).unwrap();
    assert_eq!(b.storage(), EXTRA);
    drop(fixture);
    send_packet(&submitter, &frame(b"STOP", 0), &[]).unwrap();
    let (done, []) = receive_packet::<0>(&submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&done[..4], b"DONE");
    send_packet(&control, &payload, &[]).unwrap();
}

#[test]
#[ignore = "requires explicit disposable-container root opt-in; preparation custody only"]
fn native_distinct_uid_prepare_v3_fixture() {
    fixture::run_fixture(
        "FE2O3_RUN_PRIVILEGED_PREPARE_V3_TEST",
        "launch_v3::tests::supervisor_process_helper",
    );
}
