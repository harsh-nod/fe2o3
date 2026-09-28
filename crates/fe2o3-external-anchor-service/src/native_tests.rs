use crate::{DurableExternalAnchorV1, NEXT_STATE_FILE, PersistenceBoundaryV1, STATE_FILE};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_external_anchor_protocol::{
    AnchorDecisionV1, AnchoredStateV1, CallerNonceV1, PendingAnchorTransitionV1,
    TransactionDigestV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::{self, File},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

const LIMIT: usize = 1_000_000_000;
pub(crate) fn root() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}
fn public(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn measure(seed: u8) -> Measurement {
    Measurement::new([seed; 32], 4096).unwrap()
}
pub(crate) fn deployment(axis: u8, b: &mut Budget<'_>) -> Deployment {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    assert!(
        uid != 0 && gid != 0,
        "mechanical native fixtures require nonroot credentials"
    );
    let (p, c) = Policy::new(
        7 + u64::from(axis == 1),
        measure(1),
        measure(2),
        public(3),
        public(if axis == 2 { 8 } else { 7 }),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = Supervisor::new(
        uid + 10,
        gid + 10,
        Service::new(uid + u32::from(axis == 3), gid + u32::from(axis == 4)).unwrap(),
        measure(if axis == 5 { 6 } else { 5 }),
        measure(4),
        &p,
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = Deployment::new(&s, &p, measure(if axis == 6 { 9 } else { 8 }), b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let retired = p.retained_storage() + s.retained_storage();
    drop((p, s));
    b.release_storage(retired).unwrap();
    d
}
fn key(d: &Deployment, seed: u8, b: &mut Budget<'_>) -> Key {
    b.reserve_storage(32).unwrap();
    let mut bytes = [seed; 32];
    let (key, c) = Key::create_and_zeroize(&mut bytes, d, b).unwrap();
    assert_eq!(bytes, [0; 32]);
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(32).unwrap();
    key
}
pub(crate) fn admit(
    path: &Path,
    d: &Deployment,
    b: &mut Budget<'_>,
    mode: OpenMode,
) -> (Anchor, Disposition) {
    let k = key(d, 7, b);
    b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
    let ((a, disposition), c) =
        Anchor::admit(File::open(path).unwrap().into(), k, d, b, mode).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    (a, disposition)
}
pub(crate) fn retire(a: Anchor, b: &mut Budget<'_>) {
    let n = a.retained_storage();
    drop(a);
    b.release_storage(n).unwrap();
}
fn pending(
    sequence: u64,
    head: HashChainHeadV1,
    tx: u8,
    recover: bool,
    seed: u8,
) -> PendingAnchorTransitionV1 {
    let pinned = PinnedAnchorKeyV1::from_bytes(public(seed)).unwrap();
    let prepared = AnchoredStateV1::from_local_state(sequence, head)
        .prepare(TransactionDigestV1::from_bytes([tx; 32]), &pinned)
        .unwrap();
    let nonce = CallerNonceV1::from_bytes([tx.wrapping_add(1).max(1); 32]);
    if recover {
        prepared.begin_recovery(nonce, &pinned)
    } else {
        prepared.begin_advance(nonce, &pinned)
    }
    .unwrap()
}
pub(crate) fn initial(recover: bool) -> PendingAnchorTransitionV1 {
    pending(0, HashChainHeadV1::from_bytes([0; 32]), 9, recover, 7)
}
struct Observation {
    bytes: [u8; OBSERVATION_BYTES],
    charge: Storage,
}
impl Observation {
    fn retire(self, b: &mut Budget<'_>) {
        let charge = self.charge.additional_storage();
        drop(self);
        b.release_storage(charge).unwrap();
    }
    fn verify(self, p: PendingAnchorTransitionV1, b: &mut Budget<'_>) -> AnchorDecisionV1 {
        let result = p.verify(&self.bytes);
        self.retire(b);
        result.unwrap()
    }
}
fn exchange(
    a: &mut Anchor,
    d: &Deployment,
    p: &PendingAnchorTransitionV1,
    b: &mut Budget<'_>,
) -> Observation {
    b.reserve_storage(CHALLENGE_BYTES).unwrap();
    let (wire, c) = a.exchange(p.challenge().as_bytes(), d, b).unwrap();
    assert_eq!(
        c.additional_storage(),
        size_of::<([u8; OBSERVATION_BYTES], Storage)>()
    );
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(CHALLENGE_BYTES).unwrap();
    Observation {
        bytes: wire,
        charge: c,
    }
}
fn failure<T>(r: Result<T>) -> Error {
    match r {
        Err(e) => e,
        Ok(_) => panic!("unexpected acceptance"),
    }
}
fn state(path: &Path) -> Vec<u8> {
    fs::read(path.join(STATE_FILE)).unwrap()
}

#[test]
fn advance_retry_reopen_recovery_and_legacy_state_interoperate() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let p = initial(false);
    let expected_head = p.challenge().proposed_head();
    let first = exchange(&mut a, &d, &p, &mut b);
    let inode = fs::metadata(dir.path().join(STATE_FILE)).unwrap().ino();
    let retry = exchange(&mut a, &d, &p, &mut b);
    assert_eq!(retry.bytes, first.bytes);
    retry.retire(&mut b);
    assert_eq!(
        fs::metadata(dir.path().join(STATE_FILE)).unwrap().ino(),
        inode
    );
    assert!(matches!(
        first.verify(p, &mut b),
        AnchorDecisionV1::Commit(_)
    ));
    assert_eq!((a.sequence(), a.head()), (1, expected_head));
    retire(a, &mut b);
    let mut legacy = DurableExternalAnchorV1::open(
        File::open(dir.path()).unwrap().into(),
        SigningKey::from_bytes(&[7; 32]),
    )
    .unwrap();
    let p = initial(true);
    let legacy_wire = legacy.exchange(p.challenge().as_bytes()).unwrap();
    drop(legacy);
    let (mut a, disposition) = admit(dir.path(), &d, &mut b, OpenMode::Existing);
    assert_eq!(disposition, Disposition::Existing);
    let wire = exchange(&mut a, &d, &p, &mut b);
    assert_eq!(wire.bytes, legacy_wire);
    assert!(matches!(
        wire.verify(p, &mut b),
        AnchorDecisionV1::Commit(_)
    ));
    retire(a, &mut b);
}

#[test]
fn prior_recovery_is_read_only_and_atomic_bootstrap_never_resets() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, disposition) = admit(dir.path(), &d, &mut b, OpenMode::OpenOrInitialize);
    assert_eq!(disposition, Disposition::Initialized);
    let before = state(dir.path());
    let p = initial(true);
    let wire = exchange(&mut a, &d, &p, &mut b);
    assert!(matches!(wire.verify(p, &mut b), AnchorDecisionV1::Abort(_)));
    assert_eq!(state(dir.path()), before);
    assert_eq!(a.sequence(), 0);
    let p = initial(false);
    exchange(&mut a, &d, &p, &mut b).retire(&mut b);
    retire(a, &mut b);
    let committed = state(dir.path());
    fs::write(dir.path().join(NEXT_STATE_FILE), b"abandoned").unwrap();
    let (a, disposition) = admit(dir.path(), &d, &mut b, OpenMode::OpenOrInitialize);
    assert_eq!(disposition, Disposition::Existing);
    assert_eq!(a.sequence(), 1);
    assert_eq!(state(dir.path()), committed);
    assert!(!dir.path().join(NEXT_STATE_FILE).exists());
    retire(a, &mut b);
    for i in 0..STATE_BYTES {
        let mut corrupted = committed.clone();
        corrupted[i] ^= 1;
        fs::write(dir.path().join(STATE_FILE), &corrupted).unwrap();
        let k = key(&d, 7, &mut b);
        let consumed = k.retained_storage() + Anchor::ROOT_STORAGE;
        b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
        let floor = b.storage();
        assert!(
            Anchor::open_or_initialize(File::open(dir.path()).unwrap().into(), k, &d, &mut b)
                .is_err()
        );
        assert_eq!(b.storage(), floor);
        b.release_storage(consumed).unwrap();
        assert_eq!(state(dir.path()), corrupted);
    }
}

#[test]
fn wrong_context_credentials_and_challenges_never_advance() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let before = state(dir.path());
    let p = initial(false);
    b.reserve_storage(CHALLENGE_BYTES).unwrap();
    for axis in 1..=6 {
        let other = deployment(axis, &mut b);
        let e = failure(a.exchange(p.challenge().as_bytes(), &other, &mut b));
        assert!(matches!(
            e,
            Error::ServiceCredentials | Error::Capability(_)
        ));
        let n = other.retained_storage();
        drop(other);
        b.release_storage(n).unwrap();
    }
    for p in [
        pending(5, a.head(), 9, false, 7),
        pending(0, a.head(), 9, false, 8),
    ] {
        assert!(matches!(
            a.exchange(p.challenge().as_bytes(), &d, &mut b),
            Err(Error::State(_))
        ));
    }
    for n in 0..CHALLENGE_BYTES {
        assert!(matches!(
            a.exchange(&p.challenge().as_bytes()[..n], &d, &mut b),
            Err(Error::State(_))
        ));
    }
    let mut extended = p.challenge().as_bytes().to_vec();
    extended.push(0);
    assert!(a.exchange(&extended, &d, &mut b).is_err());
    assert_eq!(state(dir.path()), before);
    assert_eq!(a.sequence(), 0);
    b.release_storage(CHALLENGE_BYTES).unwrap();
    retire(a, &mut b);
}

#[test]
fn stale_challenges_reject_after_two_exact_advances() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let first = initial(false);
    exchange(&mut a, &d, &first, &mut b).retire(&mut b);
    let second = pending(a.sequence(), a.head(), 10, false, 7);
    let wire = exchange(&mut a, &d, &second, &mut b);
    assert!(matches!(
        wire.verify(second, &mut b),
        AnchorDecisionV1::Commit(_)
    ));
    b.reserve_storage(CHALLENGE_BYTES).unwrap();
    assert!(matches!(
        a.exchange(first.challenge().as_bytes(), &d, &mut b),
        Err(Error::State(StateError::ChallengeStateMismatch))
    ));
    assert_eq!(a.sequence(), 2);
}

struct Crash {
    boundary: PersistenceBoundaryV1,
    unwind: bool,
    fired: bool,
}
impl PersistenceHooksV1 for Crash {
    fn checkpoint(&mut self, at: PersistenceBoundaryV1) -> io::Result<()> {
        if at != self.boundary {
            return Ok(());
        }
        self.fired = true;
        assert!(!self.unwind, "injected persistence unwind");
        Err(io::Error::from(io::ErrorKind::Other))
    }
}
#[test]
fn every_persistence_error_and_unwind_poisons_then_recovers_exact_state() {
    for boundary in PersistenceBoundaryV1::ALL {
        for unwind in [false, true] {
            let mut w = Work::new(LIMIT);
            let mut b = Budget::new(&mut w, LIMIT);
            let d = deployment(0, &mut b);
            let dir = root();
            let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
            let p = initial(false);
            b.reserve_storage(CHALLENGE_BYTES).unwrap();
            let floor = b.storage();
            let before_work = b.work();
            let ledger = b.work_ledger_identity_v1();
            let mut crash = Crash {
                boundary,
                unwind,
                fired: false,
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                a.exchange_with_hooks(p.challenge().as_bytes(), &d, &mut b, &mut crash, || {})
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(Error::State(StateError::Io { .. }))
                ));
            }
            assert!(crash.fired);
            assert_eq!(b.storage(), floor);
            assert!(ledger == b.work_ledger_identity_v1());
            assert_eq!(b.work() - before_work, Anchor::STATE_WORK + Key::IO_WORK);
            assert!(matches!(
                a.exchange(p.challenge().as_bytes(), &d, &mut b),
                Err(Error::State(StateError::Poisoned))
            ));
            retire(a, &mut b);
            let (mut reopened, _) = admit(dir.path(), &d, &mut b, OpenMode::Existing);
            let proposed = matches!(
                boundary,
                PersistenceBoundaryV1::AfterRename
                    | PersistenceBoundaryV1::BeforeDirectorySync
                    | PersistenceBoundaryV1::AfterDirectorySync
            );
            assert_eq!(reopened.sequence(), u64::from(proposed));
            let recover = initial(true);
            let wire = exchange(&mut reopened, &d, &recover, &mut b);
            assert_eq!(
                matches!(wire.verify(recover, &mut b), AnchorDecisionV1::Commit(_)),
                proposed
            );
            let wire = exchange(&mut reopened, &d, &p, &mut b);
            assert!(matches!(
                wire.verify(p, &mut b),
                AnchorDecisionV1::Commit(_)
            ));
            assert_eq!(reopened.sequence(), 1);
            assert!(!dir.path().join(NEXT_STATE_FILE).exists());
            retire(reopened, &mut b);
        }
    }
}

#[test]
fn key_refusal_and_unwind_after_persistence_return_no_response_but_preserve_commit() {
    for unwind in [false, true] {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let d = deployment(0, &mut b);
        let dir = root();
        let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
        let (image, c) = a.key.try_clone_for_transfer(&mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let p = initial(false);
        b.reserve_storage(CHALLENGE_BYTES).unwrap();
        let floor = b.storage();
        let result = catch_unwind(AssertUnwindSafe(|| {
            a.exchange_with_hooks(
                p.challenge().as_bytes(),
                &d,
                &mut b,
                &mut NoopPersistenceHooksV1,
                || {
                    assert!(!unwind, "injected post-persistence unwind");
                    rustix::fs::fchmod(&image, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR)
                        .unwrap();
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Err(Error::Capability(_))));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(a.sequence(), 1);
        rustix::fs::fchmod(&image, rustix::fs::Mode::RUSR).unwrap();
        let inode = fs::metadata(dir.path().join(STATE_FILE)).unwrap().ino();
        retire(a, &mut b);
        let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Existing);
        let recovery = initial(true);
        let wire = exchange(&mut a, &d, &recovery, &mut b);
        assert!(matches!(
            wire.verify(recovery, &mut b),
            AnchorDecisionV1::Commit(_)
        ));
        let wire = exchange(&mut a, &d, &p, &mut b);
        assert!(matches!(
            wire.verify(p, &mut b),
            AnchorDecisionV1::Commit(_)
        ));
        assert_eq!(
            fs::metadata(dir.path().join(STATE_FILE)).unwrap().ino(),
            inode
        );
        retire(a, &mut b);
    }
}

#[test]
fn exact_and_one_short_exchange_budgets_preserve_the_original_ledger() {
    for mode in 0..8 {
        let mut setup_w = Work::new(LIMIT);
        let mut setup = Budget::new(&mut setup_w, LIMIT);
        let d = deployment(0, &mut setup);
        let dir = root();
        let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Initialize);
        let p = initial(false);
        let input = a.retained_storage() + d.retained_storage() + CHALLENGE_BYTES;
        let floor = input - usize::from(mode == 1);
        let work = match mode {
            2 => 7,
            3 => Anchor::STATE_WORK - 1,
            5 => Anchor::STATE_WORK + Key::IO_WORK - 1,
            7 => Anchor::EXCHANGE_WORK - 1,
            _ => Anchor::EXCHANGE_WORK,
        };
        let scratch = match mode {
            4 => Anchor::STATE_STORAGE - 1,
            6 => Anchor::EXCHANGE_STORAGE - 1,
            _ => Anchor::EXCHANGE_STORAGE,
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = a.exchange(p.challenge().as_bytes(), &d, &mut b);
        assert_eq!(b.storage(), floor);
        assert!(ledger == b.work_ledger_identity_v1());
        let expected = match mode {
            1 | 3 => 8,
            2 => 0,
            4 => Anchor::STATE_WORK,
            5 => Anchor::STATE_WORK + 8,
            6 => Anchor::STATE_WORK + Key::IO_WORK,
            7 => Anchor::STATE_WORK + Key::IO_WORK + 8,
            _ => Anchor::EXCHANGE_WORK,
        };
        assert_eq!(b.work(), expected);
        if mode == 0 {
            let (wire, c) = result.unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            let wire = Observation {
                bytes: wire,
                charge: c,
            };
            assert!(matches!(
                wire.verify(p, &mut b),
                AnchorDecisionV1::Commit(_)
            ));
            assert_eq!(b.peak_storage(), floor + Anchor::EXCHANGE_STORAGE);
        } else {
            match failure(result) {
                Error::Resource(Resource::Accounting) => assert_eq!(mode, 1),
                Error::Resource(Resource::Work(_))
                | Error::Capability(
                    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Resource(
                        Resource::Work(_),
                    ),
                ) => assert!(b.failed_work().is_some()),
                Error::Resource(Resource::Storage(_))
                | Error::Capability(
                    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Resource(
                        Resource::Storage(_),
                    ),
                ) => assert!(b.failed_storage().is_some()),
                other => panic!("wrong refusal: {other}"),
            }
        }
        assert_eq!(a.sequence(), u64::from(mode == 0 || mode == 7));
        if mode == 7 {
            drop(b);
            retire(a, &mut setup);
            let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Existing);
            let p = initial(true);
            let wire = exchange(&mut a, &d, &p, &mut setup);
            assert!(matches!(
                wire.verify(p, &mut setup),
                AnchorDecisionV1::Commit(_)
            ));
            retire(a, &mut setup);
        }
    }
}

fn image_refs(file: &File) -> usize {
    let m = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|p| fs::metadata(p.ok()?.path()).ok())
        .filter(|other| other.ino() == m.ino() && other.dev() == m.dev())
        .count()
}
#[test]
fn all_constructor_boundaries_consume_inputs_without_resetting_accounting() {
    for operation in 0..3 {
        for mode in 0..8 {
            let mut setup_w = Work::new(LIMIT);
            let mut setup = Budget::new(&mut setup_w, LIMIT);
            let d = deployment(0, &mut setup);
            let dir = root();
            if operation == 1 {
                let (a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Initialize);
                retire(a, &mut setup);
            }
            let k = key(&d, 7, &mut setup);
            let consumed = Anchor::ROOT_STORAGE + k.retained_storage();
            let (witness, c) = k.try_clone_for_transfer(&mut setup).unwrap();
            setup.reserve_storage(c.additional_storage()).unwrap();
            let root = File::open(dir.path()).unwrap();
            let fd = root.as_raw_fd();
            let root_metadata = root.metadata().unwrap();
            let input = consumed + d.retained_storage();
            let floor = input - usize::from(mode == 1);
            let work = match mode {
                2 => 7,
                3 => Anchor::OPEN_WORK - 1,
                5 => Anchor::OPEN_WORK + Key::IO_WORK - 1,
                7 => Anchor::ADMISSION_WORK - 1,
                _ => Anchor::ADMISSION_WORK,
            };
            let scratch = match mode {
                4 => Anchor::STATE_STORAGE - 1,
                6 => Anchor::ADMISSION_STORAGE - 1,
                _ => Anchor::ADMISSION_STORAGE,
            };
            let mut w = Work::new(work);
            let mut b = Budget::new(&mut w, floor + scratch);
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let mode_arg = match operation {
                0 => OpenMode::Initialize,
                1 => OpenMode::Existing,
                _ => OpenMode::OpenOrInitialize,
            };
            let result = Anchor::admit(root.into(), k, &d, &mut b, mode_arg);
            assert_eq!(b.storage(), floor);
            assert!(ledger == b.work_ledger_identity_v1());
            if mode == 0 {
                let ((a, disposition), c) = result.unwrap();
                assert_eq!(
                    disposition,
                    if operation == 1 {
                        Disposition::Existing
                    } else {
                        Disposition::Initialized
                    }
                );
                assert_eq!(b.work(), Anchor::ADMISSION_WORK);
                assert_eq!(b.peak_storage(), floor + Anchor::ADMISSION_STORAGE);
                assert_eq!(c.additional_storage(), a.retained_storage() - consumed);
                b.reserve_storage(c.additional_storage()).unwrap();
                retire(a, &mut b);
                assert_eq!(b.storage(), d.retained_storage());
            } else {
                assert!(result.is_err());
                b.release_storage(consumed - usize::from(mode == 1))
                    .unwrap();
            }
            assert_eq!(image_refs(&witness), 1);
            // Parallel tests may reuse the number, but not this private root object.
            assert!(!fs::metadata(format!("/proc/self/fd/{fd}")).is_ok_and(|m|
                m.dev() == root_metadata.dev() && m.ino() == root_metadata.ino()));
            assert_eq!(
                dir.path().join(STATE_FILE).exists(),
                operation == 1 || mode == 0 || mode == 7
            );
        }
    }
}

#[test]
fn cumulative_work_peak_and_first_denials_survive_success_and_refusal() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    b.reserve_storage(500_000).unwrap();
    b.release_storage(500_000).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    let peak = b.peak_storage();
    let work = b.work();
    let p = initial(false);
    let wire = exchange(&mut a, &d, &p, &mut b);
    assert!(matches!(
        wire.verify(p, &mut b),
        AnchorDecisionV1::Commit(_)
    ));
    assert_eq!(b.work(), work + Anchor::EXCHANGE_WORK);
    assert_eq!(b.peak_storage(), peak);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert!(a.exchange(&[], &d, &mut b).is_err());
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert_eq!(b.peak_storage(), peak);
}

#[test]
fn constructor_context_credentials_and_key_metadata_reject_before_root_mutation() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    for axis in 1..=7 {
        let dir = root();
        let next = dir.path().join(NEXT_STATE_FILE);
        fs::write(&next, b"must remain untouched").unwrap();
        let other = deployment(if axis == 7 { 0 } else { axis }, &mut b);
        let k = key(&d, 7, &mut b);
        let (image, c) = k.try_clone_for_transfer(&mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        if axis == 7 {
            rustix::fs::fchmod(&image, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR).unwrap();
        }
        let consumed = k.retained_storage() + Anchor::ROOT_STORAGE;
        b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
        let floor = b.storage();
        let e = failure(Anchor::open_or_initialize(
            File::open(dir.path()).unwrap().into(),
            k,
            &other,
            &mut b,
        ));
        assert!(matches!(
            e,
            Error::ServiceCredentials | Error::Capability(_)
        ));
        assert_eq!(b.storage(), floor);
        b.release_storage(consumed).unwrap();
        assert_eq!(image_refs(&image), 1);
        assert_eq!(fs::read(&next).unwrap(), b"must remain untouched");
        assert!(!dir.path().join(STATE_FILE).exists());
        let retired = other.retained_storage() + Key::FILE_STORAGE;
        drop((other, image));
        b.release_storage(retired).unwrap();
    }
}

#[test]
fn work_and_storage_overflow_preserve_prefix_and_do_not_persist() {
    let mut setup_w = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_w, LIMIT);
    let d = deployment(0, &mut setup);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Initialize);
    let before = state(dir.path());
    let p = initial(false);
    for storage_overflow in [false, true] {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let floor = if storage_overflow {
            usize::MAX - Anchor::STATE_STORAGE + 1
        } else {
            a.retained_storage() + d.retained_storage() + CHALLENGE_BYTES
        };
        b.reserve_storage(floor).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX - 7).unwrap();
        }
        let ledger = b.work_ledger_identity_v1();
        let e = failure(a.exchange(p.challenge().as_bytes(), &d, &mut b));
        if storage_overflow {
            assert!(matches!(e, Error::Resource(Resource::Storage(_))));
            assert_eq!(b.work(), Anchor::STATE_WORK);
            assert_eq!(b.failed_storage(), Some(usize::MAX));
        } else {
            assert!(matches!(e, Error::Resource(Resource::Work(_))));
            assert_eq!(b.work(), usize::MAX - 7);
            assert_eq!(b.failed_work(), Some(usize::MAX));
        }
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(b.storage(), floor);
        assert_eq!(a.sequence(), 0);
        assert_eq!(state(dir.path()), before);
    }
}

#[test]
fn metadata_length_symlink_and_second_writer_refusals_preserve_existing_state() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let original = state(dir.path());
    let reject = |b: &mut Budget<'_>| {
        let k = key(&d, 7, b);
        let consumed = k.retained_storage() + Anchor::ROOT_STORAGE;
        b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
        let e = failure(Anchor::open_or_initialize(
            File::open(dir.path()).unwrap().into(),
            k,
            &d,
            b,
        ));
        b.release_storage(consumed).unwrap();
        e
    };
    assert!(matches!(
        reject(&mut b),
        Error::State(StateError::StoreBusy)
    ));
    retire(a, &mut b);
    let path = dir.path().join(STATE_FILE);
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        reject(&mut b),
        Error::State(StateError::RootModeMismatch { .. })
    ));
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(matches!(
        reject(&mut b),
        Error::State(StateError::InvalidStateFileMetadata)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    for bytes in [
        original[..STATE_BYTES - 1].to_vec(),
        [original.clone(), vec![0]].concat(),
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(reject(&mut b), Error::State(_)));
        assert_eq!(state(dir.path()), bytes);
    }
    fs::write(&path, &original).unwrap();
    let saved = dir.path().join("saved");
    fs::rename(&path, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &path).unwrap();
    assert!(matches!(reject(&mut b), Error::State(_)));
    fs::remove_file(&path).unwrap();
    fs::hard_link(&saved, &path).unwrap();
    assert!(matches!(
        reject(&mut b),
        Error::State(StateError::InvalidStateFileMetadata)
    ));
    fs::remove_file(&path).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &path,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(matches!(
        reject(&mut b),
        Error::State(StateError::InvalidStateFileMetadata)
    ));
    assert_eq!(fs::read(&saved).unwrap(), original);
}
