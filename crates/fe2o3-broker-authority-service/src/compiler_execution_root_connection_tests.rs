//! Inert association/account checks and actual API signatures only. No fixture
//! constructs a RootSession, connection, task observation or child custody.
//! These successes are not issuer authentication or retirement evidence.
use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};

const WORK_LIMIT: usize = 200_000_000;
const STORAGE_LIMIT: usize = 2_000_000;
const EPOCH: [u8; 32] = [0x71; 32];

fn policy(measurement: u8, b: &mut Budget<'_>) -> Policy {
    let (policy, charge) = Policy::new(
        7,
        Measurement::new([measurement; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    policy
}

fn manifest(policy: &Policy, client: u32, b: &mut Budget<'_>) -> Manifest {
    let (manifest, charge) = Manifest::new(
        Client::new(client, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        policy,
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    manifest
}

fn request(policy: &Policy, manifest: &Manifest, b: &mut Budget<'_>) -> Record {
    let (binding, charge) = Binding::new(policy, manifest, EPOCH, [0x72; 32], b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (request, charge) = gate_request(&binding, b).unwrap();
    let released = binding.retained_storage();
    drop(binding);
    b.release_storage(released).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    request
}

fn association(
    epoch: [u8; 32],
    request: &Record,
    policy: &Policy,
    manifest: &Manifest,
    b: &mut Budget<'_>,
) -> Result<()> {
    let floor = sum(&[
        request.retained_storage(),
        policy.retained_storage(),
        manifest.retained_storage(),
    ])?;
    b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
        check_launch_association(EPOCH, epoch, request, policy, manifest, b)
    })
}

#[test]
fn retained_launch_association_refuses_foreign_epoch_policy_and_manifest() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let p = policy(0x61, &mut b);
    let m = manifest(&p, 1234, &mut b);
    let r = request(&p, &m, &mut b);
    let other_policy = policy(0x63, &mut b);
    let other_manifest = manifest(&other_policy, 1234, &mut b);
    let other_client = manifest(&p, 1235, &mut b);
    let bytes = *r.canonical_bytes();
    for (epoch, policy, manifest, accepted) in [
        (EPOCH, &p, &m, true),
        ([0x73; 32], &p, &m, false),
        ([0; 32], &p, &m, false),
        (EPOCH, &other_policy, &m, false),
        (EPOCH, &other_policy, &other_manifest, false),
        (EPOCH, &p, &other_manifest, false),
        (EPOCH, &p, &other_client, false),
    ] {
        let before = b.work();
        let floor = b.storage();
        let result = association(epoch, &r, policy, manifest, &mut b);
        if accepted {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(Error::Refused("root connection association changed"))
            ));
        }
        assert_eq!(
            b.work() - before,
            LOCAL_WORK + if epoch == EPOCH { CODEC_WORK } else { 0 }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(r.canonical_bytes(), &bytes);
    }
}

#[test]
fn association_exact_and_short_funding_preserve_storage_and_first_denials() {
    for short in 0..4 {
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let p = policy(0x61, &mut b);
        let m = manifest(&p, 1234, &mut b);
        let r = request(&p, &m, &mut b);
        assert!(b.charge_work(WORK_LIMIT + 1).is_err());
        assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
        let history = (b.failed_work(), b.failed_storage());
        if short == 3 {
            b.release_storage(1).unwrap();
        } else {
            b.reserve_storage(
                STORAGE_LIMIT - b.storage() - FRAME - CODEC_SCRATCH + usize::from(short == 2),
            )
            .unwrap();
        }
        b.charge_work(WORK_LIMIT - b.work() - LOCAL_WORK - CODEC_WORK + usize::from(short == 1))
            .unwrap();
        let floor = b.storage();
        let result = association(EPOCH, &r, &p, &m, &mut b);
        match short {
            0 => {
                result.unwrap();
                assert_eq!(b.work(), WORK_LIMIT);
                assert_eq!(b.peak_storage(), STORAGE_LIMIT);
            }
            1 => assert!(matches!(
                result,
                Err(Error::Protocol(ProtocolError::Resource(Resource::Work(_))))
            )),
            2 => assert!(matches!(
                result,
                Err(Error::Protocol(ProtocolError::Resource(Resource::Storage(
                    _
                ))))
            )),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
        }
        assert_eq!(b.storage(), floor);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
    }
}

#[test]
fn original_account_comparison_refuses_foreign_ledger_address_process_and_thread() {
    let mut work = Work::new(WORK_LIMIT);
    let mut other_work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut other = Budget::new(&mut other_work, STORAGE_LIMIT);
    b.reserve_storage(23).unwrap();
    other.reserve_storage(23).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let pid = process::getpid();
    let tid = rustix::thread::gettid();
    let changed_pid = process::Pid::from_raw(if pid.as_raw_pid() == 1 { 2 } else { 1 }).unwrap();
    let changed_tid = process::Pid::from_raw(if tid.as_raw_pid() == 1 { 2 } else { 1 }).unwrap();
    assert!(b.charge_work(WORK_LIMIT + 1).is_err());
    assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
    let history = (b.failed_work(), b.failed_storage());
    for (process, thread, accepted) in [
        (pid, tid, true),
        (changed_pid, tid, false),
        (pid, changed_tid, false),
    ] {
        let result = b.with_prepaid_scope(23, ENTRY, LOCAL_WORK, FRAME, |b| {
            check_account_binding(ledger, address, process, thread, b)
        });
        if accepted {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        }
        assert_eq!(b.storage(), 23);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
    }
    assert!(matches!(
        check_account_binding(ledger, address, pid, tid, &other),
        Err(Error::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut b, &mut other);
    for candidate in [&b, &other] {
        assert!(matches!(
            check_account_binding(ledger, address, pid, tid, candidate),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(candidate.storage(), 23);
    }
    std::mem::swap(&mut b, &mut other);
    assert_eq!((b.failed_work(), b.failed_storage()), history);
}

#[test]
fn actual_method_signatures_separate_connection_authentication_from_live_observation() {
    // Compile-pass checks of the actual methods, never fabricated successful custody.
    fn established<T: Send + 'static>(
        connection: &RootConnectionV3<'_>,
        root: &RootControlSessionV3<'_>,
        issuer: &Child<T>,
        policy: &Policy,
        manifest: &Manifest,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        connection.validate_established_issuer_authentication(root, issuer, policy, manifest, b)
    }
    fn live<T: Send + 'static>(
        connection: &RootConnectionV3<'_>,
        root: &RootControlSessionV3<'_>,
        original: &Original<'_, '_>,
        issuer: &Child<T>,
        policy: &Policy,
        manifest: &Manifest,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        connection.validate(root, original, issuer, policy, manifest, b)
    }
    let _ = established::<()>;
    let _ = live::<()>;
}
