//! Portable custody/receipt checks only; fixtures do not authenticate a service boot.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionClientProcessIdentityV1 as Process,
    CompilerExecutionClientProfileV3 as ProfileRecord,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerPolicyV3 as PolicyRecord,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

const WORK: usize = 30_000_000;
const LIMIT: usize = 2_000_000;
const CHILD: u32 = 101;

use super::super::tests::run_in_isolated_boundary_test_process as isolated;
// These fixtures exercise inert transport, not fixed-root policy approval.
use super::preparation::PreparedCompilerExecutionTransportV3 as Prepared;
use crate::build_config::tests::native_recipe_for_test as recipe;
use fe2o3_compiler_closure_capability::COMPILER_EXECUTION_POLICY_CHILD_FD_V1 as POLICY_FD;
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 as SERVICE_FD,
    CompilerExecutionChildChannelErrorV1 as ChildError,
    PendingCompilerExecutionChildChannelV1 as Pending,
};
use std::{fs::File, os::fd::AsRawFd, process::Command};

#[test]
fn native_runtime_enforcement_is_unavailable_even_with_valid_configuration() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut budget);
    profile.revalidate(&mut budget).unwrap();
    let storage = budget.storage();
    let work = budget.work();
    let error = require_runtime_enforcement(&mut budget).unwrap_err();
    assert!(matches!(error, Failure::RuntimeEnforcementUnavailable));
    assert_eq!(
        error.to_string(),
        "native compiler runtime enforcement is not yet available"
    );
    assert!(error.source().is_none());
    assert_eq!(budget.storage(), storage);
    assert_eq!(budget.work(), work + 8);
    profile.revalidate(&mut budget).unwrap();
}

#[test]
fn runtime_enforcement_refusal_prepays_work_without_retiring_storage() {
    for limit in [7, 8] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        let error = require_runtime_enforcement(&mut budget).unwrap_err();
        match limit {
            7 => assert!(matches!(error, Failure::Resource(Resource::Work(_)))),
            8 => assert!(matches!(error, Failure::RuntimeEnforcementUnavailable)),
            _ => unreachable!(),
        }
        assert_eq!(budget.work(), if limit == 7 { 0 } else { 8 });
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn native_preparation_retains_exact_policy_and_terminal_storage() {
    if isolated(
        "compiler_execution_boundary::native::tests::native_preparation_retains_exact_policy_and_terminal_storage",
    ) {
        return;
    }
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut b);
    let expected = *profile.profile().policy().canonical_bytes();
    let recipe = recipe(&mut b);
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let mut command = Command::new("/bin/true");
    let prepared = Prepared::prepare(profile, recipe, &mut command, &mut b).unwrap();
    assert_eq!(
        std::fs::read(format!("/proc/self/fd/{POLICY_FD}")).unwrap(),
        expected
    );
    // SAFETY: F_GETFD observes the slots owned by Command and the pending channel.
    for fd in [POLICY_FD, SERVICE_FD] {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }
    drop(prepared);
    assert!(b.storage() > floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert!(matches!(
        Pending::preflight_with_issuer_policy(),
        Err(ChildError::ReservedPolicyDescriptorInUse)
    ));
    let retained = b.storage();
    drop(command);
    Pending::preflight_with_issuer_policy().unwrap();
    assert_eq!(b.storage(), retained);
}

#[test]
fn native_preparation_exact_short_and_occupied_slot_checks() {
    if isolated(
        "compiler_execution_boundary::native::tests::native_preparation_exact_short_and_occupied_slot_checks",
    ) {
        return;
    }
    let mut measured = (WORK, LIMIT);
    for case in 0..6 {
        let mut work = Work::new(measured.0 - usize::from(case == 2));
        let mut b = Budget::new(&mut work, measured.1 - usize::from(case == 3));
        let profile = profile(7, &mut b);
        let recipe = recipe(&mut b);
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let occupied = if case >= 4 {
            let source = File::open("/dev/null").unwrap();
            let target = if case == 4 { SERVICE_FD } else { POLICY_FD };
            let owned = rustix::io::fcntl_dupfd_cloexec(&source, target).unwrap();
            assert_eq!(owned.as_raw_fd(), target);
            Some(owned)
        } else {
            None
        };
        let mut command = Command::new("/bin/true");
        let result = Prepared::prepare(profile, recipe, &mut command, &mut b);
        match case {
            0 | 1 => drop(result.unwrap()),
            2 | 3 => {
                let error = result.err().expect("short quota must reject");
                let resource = match error {
                    Failure::Resource(resource)
                    | Failure::Capability(CapabilityError::Resource(resource))
                    | Failure::Policy(fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV3::Resource(resource)) => resource,
                    other => panic!("case {case}: unexpected refusal {other:?}"),
                };
                assert!(
                    match resource {
                        Resource::Work(_) => case == 2,
                        Resource::Storage(_) => case == 3,
                        _ => false,
                    },
                    "case {case}: {resource:?}"
                );
            }
            4 => assert!(matches!(
                result,
                Err(Failure::Child(ChildError::ReservedDescriptorInUse))
            )),
            _ => assert!(matches!(
                result,
                Err(Failure::Child(ChildError::ReservedPolicyDescriptorInUse))
            )),
        }
        if case == 0 {
            measured = (b.work(), b.peak_storage());
        }
        if let Some(fd) = occupied.as_ref() {
            assert!(rustix::fs::fstat(fd).is_ok());
            assert!(
                command.status().unwrap().success(),
                "refused preparation changed Command"
            );
        }
        drop(command);
        drop(occupied);
        Pending::preflight_with_issuer_policy().unwrap();
        assert!(b.storage() >= floor);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn native_preparation_refuses_invalid_finish_without_detaching_the_account() {
    if isolated(
        "compiler_execution_boundary::native::tests::native_preparation_refuses_invalid_finish_without_detaching_the_account",
    ) {
        return;
    }
    for child in [0, std::process::id()] {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, LIMIT);
        let profile = profile(7, &mut b);
        let recipe = recipe(&mut b);
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let mut command = Command::new("/bin/true");
        let prepared = Prepared::prepare(profile, recipe, &mut command, &mut b).unwrap();
        let result = prepared.finish(child, Instant::now());
        if child == 0 {
            assert!(matches!(
                result,
                Err(Failure::Child(ChildError::InvalidChildPid))
            ));
        } else {
            assert!(matches!(result, Err(Failure::Child(ChildError::Timeout))));
        }
        drop(command);
        Pending::preflight_with_issuer_policy().unwrap();
        assert!(b.storage() > floor);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

fn policy(generation: u64, b: &mut Budget<'_>) -> PolicyRecord {
    let mut bytes = fixture::policy_wire(3);
    bytes[24..32].copy_from_slice(&generation.to_le_bytes());
    fixture::seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    b.reserve_storage(bytes.len()).unwrap();
    let (policy, storage) = PolicyRecord::decode(&bytes, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    b.release_storage(bytes.len()).unwrap();
    policy
}

fn profile(generation: u64, b: &mut Budget<'_>) -> Profile {
    let policy = policy(generation, b);
    let (profile, storage) =
        ProfileRecord::new(2000, 2001, Anchor::new(6000, 6001).unwrap(), policy, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (profile, storage) = Profile::create(profile, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    profile
}

fn sealed_policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    let policy = policy(generation, b);
    let (policy, storage) = Policy::create(policy, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    policy
}

fn records(
    policy: &Policy,
    pid: u32,
    uid: u32,
    anchor: u32,
    b: &mut Budget<'_>,
) -> (Manifest, Ready) {
    let (manifest, storage) = Manifest::new(
        Process::new(pid, uid, 1000).unwrap(),
        Anchor::new(anchor, 6001).unwrap(),
        policy.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (ready, storage) = Ready::new(200, &manifest, policy.policy(), b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    (manifest, ready)
}

fn subject(b: &mut Budget<'_>) -> Subject {
    let bytes = fixture::subject_wire(3);
    b.reserve_storage(bytes.len()).unwrap();
    let (subject, storage) = Subject::decode(&bytes, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    b.release_storage(bytes.len()).unwrap();
    subject
}

fn carriage(b: &mut Budget<'_>) -> Carriage {
    let p = policy(7, b);
    let q = fixture::request_wire(3);
    let r = receipt_fixture::receipt_wire(3);
    b.reserve_storage(q.len() + r.len()).unwrap();
    let (q, storage) = Request::decode(&q, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (r, storage) = Receipt::decode(&r, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (u, storage) = Publication::new([0x81; 32], [0x82; 32], r, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (a, storage) = Ack::new(&u, [0x83; 32], b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (carriage, storage) = Carriage::new(p, q, u, a, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    b.release_storage(fixture::REQUEST_BYTES + 400).unwrap();
    carriage
}

#[test]
fn exact_readiness_retains_seals_and_every_context_axis() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut b);
    let policy = sealed_policy(7, &mut b);
    let (manifest, ready) = records(&policy, CHILD, 1000, 6000, &mut b);
    let floor = b.storage();
    validate_readiness(&profile, &policy, CHILD, &manifest, &ready, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    for child in [0, CHILD + 1] {
        assert!(matches!(
            validate_readiness(&profile, &policy, child, &manifest, &ready, &mut b),
            Err(Failure::Mismatch(_))
        ));
    }
    for (pid, uid, anchor) in [
        (CHILD + 1, 1000, 6000),
        (CHILD, 2000, 6000),
        (CHILD, 1000, 6002),
    ] {
        let (other, other_ready) = records(&policy, pid, uid, anchor, &mut b);
        assert!(matches!(
            validate_readiness(&profile, &policy, CHILD, &other, &other_ready, &mut b),
            Err(Failure::Mismatch(_))
        ));
        assert!(
            validate_readiness(&profile, &policy, CHILD, &manifest, &other_ready, &mut b).is_err()
        );
    }
    let other_policy = sealed_policy(8, &mut b);
    assert!(matches!(
        validate_readiness(&profile, &other_policy, CHILD, &manifest, &ready, &mut b),
        Err(Failure::Mismatch(_))
    ));
    let (other, other_ready) = records(&other_policy, CHILD, 1000, 6000, &mut b);
    assert!(matches!(
        validate_readiness(&profile, &policy, CHILD, &other, &other_ready, &mut b),
        Err(Failure::Mismatch(_))
    ));
}

#[test]
fn exact_readiness_and_one_short_limits_preserve_original_ledger() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let profile = profile(7, &mut setup);
    let policy = sealed_policy(7, &mut setup);
    let (manifest, ready) = records(&policy, CHILD, 1000, 6000, &mut setup);
    let floor = profile.retained_storage()
        + policy.retained_storage()
        + manifest.retained_storage()
        + ready.retained_storage();
    let mut expected = (WORK, LIMIT);
    for case in 0..5 {
        let (quota, limit, input) = match case {
            2 => (expected.0 - 1, expected.1, floor),
            3 => (expected.0, expected.1 - 1, floor),
            4 => (expected.0, expected.1, floor - 1),
            _ => (expected.0, expected.1, floor),
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = validate_readiness(&profile, &policy, CHILD, &manifest, &ready, &mut b);
        assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
        assert_eq!(b.storage(), input);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            expected = (b.work(), b.peak_storage());
        }
        if case == 3 {
            assert!(b.failed_storage().is_some());
        }
        if case == 4 {
            assert_eq!(b.work(), 19);
        }
    }
}

#[test]
fn receipt_matches_full_policy_subject_and_signature_without_granting_authority() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut b);
    let subject = subject(&mut b);
    let carriage = carriage(&mut b);
    let floor = b.storage();
    validate_receipt(&profile, &subject, &carriage, &mut b).unwrap();
    let (decoded, storage) =
        decode_receipt(&profile, &subject, carriage.canonical_bytes(), &mut b).unwrap();
    assert_eq!(decoded, carriage);
    assert_eq!(storage.additional_storage(), decoded.retained_storage());
    assert_eq!(b.storage(), floor);
    assert!(!decoded.grants_compiler_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(decoded.requires_protected_policy_verification());
    drop(decoded);

    let other_profile = self::profile(8, &mut b);
    assert!(matches!(
        validate_receipt(&other_profile, &subject, &carriage, &mut b),
        Err(Failure::Mismatch(_))
    ));
    assert!(matches!(
        decode_receipt(&other_profile, &subject, carriage.canonical_bytes(), &mut b),
        Err(Failure::Mismatch(_))
    ));
    let mut other_subject = fixture::subject_wire(3);
    other_subject[24..32].copy_from_slice(&10u64.to_le_bytes());
    fixture::seal(&mut other_subject, "INERT-COMPILER-EXECUTION-SUBJECT", 3);
    b.reserve_storage(other_subject.len()).unwrap();
    let (other_subject, storage) = Subject::decode(&other_subject, &mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    assert!(matches!(
        validate_receipt(&profile, &other_subject, &carriage, &mut b),
        Err(Failure::Mismatch(_))
    ));
    assert!(matches!(
        decode_receipt(&profile, &other_subject, carriage.canonical_bytes(), &mut b),
        Err(Failure::Mismatch(_))
    ));

    let mut corrupt = *carriage.canonical_bytes();
    b.reserve_storage(corrupt.len()).unwrap();
    corrupt[30] ^= 1;
    fixture::seal(&mut corrupt, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", 3);
    assert!(matches!(
        decode_receipt(&profile, &subject, &corrupt, &mut b),
        Err(Failure::Receipt(_))
    ));
    for version in [1u16, 2] {
        let mut downgraded = *carriage.canonical_bytes();
        downgraded[7] = b'0' + version as u8;
        downgraded[8..10].copy_from_slice(&version.to_le_bytes());
        fixture::seal(
            &mut downgraded,
            "COMPILER-EXECUTION-RECEIPT-CARRIAGE",
            version,
        );
        assert!(matches!(
            decode_receipt(&profile, &subject, &downgraded, &mut b),
            Err(Failure::Receipt(_))
        ));
    }
    for bytes in [
        &[][..],
        &carriage.canonical_bytes()[..carriage.canonical_bytes().len() - 1],
    ] {
        assert!(matches!(
            decode_receipt(&profile, &subject, bytes, &mut b),
            Err(Failure::Receipt(_))
        ));
    }
}

#[test]
fn receipt_decode_limits_refund_only_scratch_and_return_full_owner_charge() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let profile = profile(7, &mut setup);
    let subject = subject(&mut setup);
    let carriage = carriage(&mut setup);
    let bytes = carriage.canonical_bytes();
    let floor = profile.retained_storage() + SUBJECT_STORAGE + bytes.len();
    let mut expected = (WORK, LIMIT);
    for case in 0..5 {
        let (quota, limit, input) = match case {
            2 => (expected.0 - 1, expected.1, floor),
            3 => (expected.0, expected.1 - 1, floor),
            4 => (expected.0, expected.1, floor - 1),
            _ => (expected.0, expected.1, floor),
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = decode_receipt(&profile, &subject, bytes, &mut b);
        assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
        assert_eq!(b.storage(), input);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            expected = (b.work(), b.peak_storage());
        }
        if case == 3 {
            assert!(b.failed_storage().is_some());
        }
        if case == 4 {
            assert_eq!(b.work(), 19);
        }
        if let Ok((owner, charge)) = result {
            assert_eq!(charge.additional_storage(), owner.retained_storage());
            b.reserve_storage(charge.additional_storage()).unwrap();
            drop(owner);
            b.release_storage(charge.additional_storage()).unwrap();
            assert_eq!(b.storage(), input);
        }
    }
}
