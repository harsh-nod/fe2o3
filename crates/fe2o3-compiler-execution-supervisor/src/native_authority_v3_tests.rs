use super::*;
use crate::program_v3::tests::{
    SEED, STORAGE_LIMIT, WORK_LIMIT, new_program, policy, revalidation_work,
};
use crate::{authority_v2_test_process as process_fixture, tests::Fixture};
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV3 as Cap;
use fe2o3_compiler_execution_protocol::sealed_static_issuer_runtime_measurement_v1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::ProtectedStaticExecutableV2 as Image;
use std::{
    os::fd::{AsFd, OwnedFd},
    time::{Duration, Instant},
};

type Supervisor = ProtectedIssuerSupervisorV3;
type Failure = ProtectedIssuerSupervisorErrorV3;
const OPT_IN: &str = "FE2O3_RUN_PRIVILEGED_SUPERVISOR_V3_TEST";
const ANCHOR_UID: u32 = 65_534;
const SUPERVISOR_UID: u32 = 65_533;
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD: &str = "authority_v3::tests::supervisor_process_helper";

pub(crate) fn measured_policy(
    issuer: fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1,
    generation: u64,
    budget: &mut Budget<'_>,
) -> Policy {
    policy(
        issuer,
        sealed_static_issuer_runtime_measurement_v1(),
        generation,
        budget,
    )
}

include!("authority_native_consuming_tests.rs");

fn credentials() -> Credentials {
    Credentials::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap()
}

fn inputs(
    f: &Fixture,
    peer: &OwnedFd,
    pidfd: &OwnedFd,
    generation: u64,
    b: &mut Budget<'_>,
) -> (Program, Key, Anchor, File) {
    let program = new_program(f, b);
    let other = (generation != 7).then(|| {
        policy(
            f.issuer_measurement(),
            sealed_static_issuer_runtime_measurement_v1(),
            generation,
            b,
        )
    });
    let temporary = SEED.len() + other.as_ref().map_or(0, Policy::retained_storage);
    b.reserve_storage(SEED.len()).unwrap();
    let mut seed = SEED;
    let (key, delta) = Key::create_and_zeroize(
        &mut seed,
        other.as_ref().unwrap_or_else(|| program.policy()),
        b,
    )
    .unwrap();
    assert_eq!(seed, [0; 32]);
    b.reserve_storage(delta.additional_storage()).unwrap();
    drop(other);
    b.release_storage(temporary).unwrap();
    b.reserve_storage(Anchor::PAIR_STORAGE).unwrap();
    let (anchor, delta) = Anchor::admit(
        rustix::io::fcntl_dupfd_cloexec(peer, 3).unwrap(),
        rustix::io::fcntl_dupfd_cloexec(pidfd, 3).unwrap(),
        AnchorIdentity::new(ANCHOR_UID, ANCHOR_UID).unwrap(),
        b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.reserve_storage(Supervisor::ROOT_FILE_STORAGE).unwrap();
    (program, key, anchor, File::open(&f.root).unwrap())
}

fn references(file: &File) -> usize {
    use std::os::unix::fs::MetadataExt;
    let object = file.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (object.dev(), object.ino()))
        .count()
}

fn exercise(peer: OwnedFd, pidfd: OwnedFd) {
    let f = Fixture::new("v3-supervisor-account");
    let root_witness = File::open(&f.root).unwrap();
    let baseline = references(&root_witness);
    let nested = revalidation_work(&f) + Key::IO_WORK + Anchor::REVALIDATION_WORK;
    let full_work = Supervisor::WORK + 2 * nested;
    for case in 0..7 {
        // Every native input is constructed, admitted and consumed on this ledger.
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let account = b.work_ledger_identity_v1();
        b.reserve_storage(19).unwrap();
        let (program, key, anchor, root) =
            inputs(&f, &peer, &pidfd, if case == 3 { 8 } else { 7 }, &mut b);
        let input_charge = program.retained_storage()
            + key.retained_storage()
            + anchor.retained_storage()
            + Supervisor::ROOT_FILE_STORAGE;
        assert_eq!(b.storage(), input_charge + 19);
        let credentials = if case == 4 {
            Credentials::new(65_532, SUPERVISOR_UID).unwrap()
        } else {
            credentials()
        };
        if case == 1 {
            b.charge_work(WORK_LIMIT - b.work() - (ENTRY - 1)).unwrap();
        } else if case == 2 {
            b.release_storage(20).unwrap();
        } else if case == 5 {
            b.charge_work(WORK_LIMIT - b.work() - (full_work - 1))
                .unwrap();
        } else if case == 6 {
            b.reserve_storage(STORAGE_LIMIT - b.storage() - Supervisor::SCRATCH + 1)
                .unwrap();
        }
        let floor = b.storage();
        let start = b.work();
        let result = Supervisor::bind(program, credentials, root, key, anchor, &mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == account);
        match case {
            0 => {
                let (mut owner, delta) = result.unwrap();
                assert_eq!(b.work() - start, full_work);
                assert_eq!(delta.additional_storage(), Supervisor::OWNER_GROWTH);
                assert_eq!(
                    owner.retained_storage(),
                    input_charge + delta.additional_storage()
                );
                b.reserve_storage(delta.additional_storage()).unwrap();
                let _: &Policy = owner.policy();
                assert_eq!(owner.policy().generation(), 7);
                assert_eq!(owner.credentials(), credentials);
                assert_eq!(
                    owner.external_anchor_service(),
                    AnchorIdentity::new(ANCHOR_UID, ANCHOR_UID).unwrap()
                );
                let start = b.work();
                owner.revalidate(&mut b).unwrap();
                assert_eq!(b.work() - start, Supervisor::WORK + nested);
                let saved = owner.root_snapshot;
                let other = Fixture::new("v3-supervisor-other-root");
                let original = std::mem::replace(&mut owner.root, File::open(&other.root).unwrap());
                assert!(matches!(
                    owner.revalidate(&mut b),
                    Err(Failure::RootChanged)
                ));
                owner.root = original;
                assert_eq!(owner.root_snapshot, saved);
                rustix::fs::fchmod(&owner.root, rustix::fs::Mode::from_raw_mode(0o750)).unwrap();
                assert!(matches!(
                    owner.revalidate(&mut b),
                    Err(Failure::InvalidRoot("mode is not exactly 0700"))
                ));
                rustix::fs::fchmod(&owner.root, rustix::fs::Mode::from_raw_mode(0o700)).unwrap();
                owner.revalidate(&mut b).unwrap();
                assert_eq!(b.storage(), 19 + owner.retained_storage());
                let retained = owner.retained_storage();
                drop(owner);
                b.release_storage(retained).unwrap();
                assert_eq!(b.storage(), 19);
            }
            1 => {
                assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
                assert_eq!(b.work(), start);
            }
            2 => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Accounting))
                ));
                assert_eq!(b.work() - start, ENTRY);
            }
            3 => {
                assert!(matches!(
                    result,
                    Err(Failure::SigningKey(KeyError::Rejected(
                        "signing key is pinned to another native policy"
                    )))
                ));
                assert_eq!(
                    b.work() - start,
                    Supervisor::WORK + revalidation_work(&f) + Key::IO_WORK
                );
            }
            4 => {
                assert!(matches!(result, Err(Failure::ServiceIdentityMismatch)));
                assert_eq!(b.work() - start, Supervisor::WORK);
            }
            5 => {
                let failure = result.unwrap_err();
                let Failure::ExternalAnchor(error) = failure else {
                    panic!("expected late anchor quota refusal")
                };
                assert!(matches!(error.resource(), Some(Resource::Work(_))));
                assert_eq!(
                    b.work() - start,
                    full_work - Anchor::REVALIDATION_WORK + ENTRY
                );
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                ));
                assert_eq!(b.work() - start, Supervisor::WORK);
                assert_eq!(b.failed_storage(), Some(STORAGE_LIMIT + 1));
            }
        }
        assert_eq!(references(&root_witness), baseline);
    }
}

#[test]
fn v3_key_same_public_key_different_policy_is_not_an_authority_join() {
    let f = Fixture::new("v3-supervisor-key-join");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let account = b.work_ledger_identity_v1();
    let program = new_program(&f, &mut b);
    let other = policy(
        f.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        8,
        &mut b,
    );
    b.reserve_storage(SEED.len()).unwrap();
    let mut seed = SEED;
    let (key, delta) = Key::create_and_zeroize(&mut seed, &other, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(seed, [0; 32]);
    let floor = b.storage();
    let start = b.work();
    let error = key.revalidate(program.policy(), &mut b).unwrap_err();
    assert!(matches!(
        Failure::from(error),
        Failure::SigningKey(KeyError::Rejected(
            "signing key is pinned to another native policy"
        ))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - start, Key::IO_WORK);
    assert!(b.work_ledger_identity_v1() == account);
    key.revalidate(&other, &mut b).unwrap();
}

#[test]
fn v3_native_authority_errors_and_credential_axes_remain_typed() {
    let error = Failure::from(RootCheckError::Invalid("mode is not exactly 0700"));
    assert_eq!(
        error.to_string(),
        "invalid protected issuer root: mode is not exactly 0700"
    );
    let error = Failure::from(RootCheckError::Io {
        operation: "inspect protected issuer root",
        errno: rustix::io::Errno::BADF,
    });
    assert_eq!(
        error.source().unwrap().downcast_ref::<rustix::io::Errno>(),
        Some(&rustix::io::Errno::BADF)
    );
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    let other = |id| if id == 1 { 2 } else { 1 };
    for (uid, gid) in [(other(uid), gid.max(1)), (uid.max(1), other(gid))] {
        assert!(matches!(
            require_credentials(Credentials::new(uid, gid).unwrap()),
            Err(Failure::ServiceIdentityMismatch)
        ));
    }
}

#[test]
#[ignore = "opt-in disposable-container root coordinator; custody fixtures, not protected activation"]
fn native_distinct_uid_v3_supervisor_fixture() {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
    assert!(
        std::path::Path::new("/.dockerenv").is_file(),
        "isolated Docker fixture only"
    );
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let caps = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:\t"))
        .unwrap();
    let caps = u64::from_str_radix(caps, 16).unwrap();
    let required = (1 << 5) | (1 << 6) | (1 << 7);
    assert_eq!(
        caps & required,
        required,
        "root fixture lacks cleanup/credential capabilities"
    );
    let (control, child) = process_fixture::pair();
    // The reused anchor only supplies a live policy-neutral socket/pidfd pair.
    let mut anchor = process_fixture::spawn_role(
        "authority_v2_test_process::anchor_process_helper",
        "anchor",
        ANCHOR_UID,
        child,
    );
    let pid = anchor.0.id();
    let process = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
    let pidfd = rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()).unwrap();
    let (payload, [peer]) = process_fixture::receive_packet::<1>(
        &control,
        Instant::now() + process_fixture::IO_TIMEOUT,
    )
    .unwrap();
    assert_eq!(payload, process_fixture::frame(b"ANC2", pid));
    let (supervisor_control, child) = process_fixture::pair();
    let mut supervisor = process_fixture::spawn_role(CHILD, "supervisor-v3", SUPERVISOR_UID, child);
    let deadline = Instant::now() + CHILD_TIMEOUT;
    process_fixture::send_packet(
        &supervisor_control,
        &payload,
        &[peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd));
    // An exact helper filter running zero tests cannot produce this acknowledgment.
    let (completed, []) =
        process_fixture::receive_packet::<0>(&supervisor_control, deadline).unwrap();
    assert_eq!(completed, process_fixture::frame(b"DONE", pid));
    let supervisor_status = supervisor.wait_until(deadline).unwrap();
    process_fixture::send_packet(&control, &process_fixture::frame(b"STOP", pid), &[]).unwrap();
    let anchor_status = anchor
        .wait_until(Instant::now() + process_fixture::IO_TIMEOUT)
        .unwrap();
    assert!(
        supervisor_status.success(),
        "V3 supervisor fixture failed: {supervisor_status}"
    );
    assert!(
        anchor_status.success(),
        "anchor fixture failed: {anchor_status}"
    );
}

#[test]
#[ignore = "private non-root V3 role, executed only by the root coordinator"]
fn supervisor_process_helper() {
    process_fixture::require_child_credentials("supervisor-v3", SUPERVISOR_UID);
    let control = process_fixture::inherited_control();
    let (payload, [peer, pidfd]) = process_fixture::receive_packet::<2>(
        &control,
        Instant::now() + process_fixture::IO_TIMEOUT,
    )
    .unwrap();
    assert_eq!(&payload[..4], b"ANC2");
    let pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
    assert_ne!(pid, 0);
    exercise(peer, pidfd);
    process_fixture::send_packet(&control, &process_fixture::frame(b"DONE", pid), &[]).unwrap();
}
