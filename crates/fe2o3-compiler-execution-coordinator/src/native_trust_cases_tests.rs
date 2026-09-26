use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 1 << 30;
const EXTRA: usize = 19;
fn public(seed: u8) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn policy(axis: usize, b: &mut Budget<'_>) -> PolicyCap {
    let (record, c) = Policy::new(
        7 + u64::from(axis == 1),
        Measurement::new([1 + u8::from(axis == 2); 32], 11 + u64::from(axis == 3)).unwrap(),
        Measurement::new([2 + u8::from(axis == 4); 32], 12 + u64::from(axis == 5)).unwrap(),
        public(7 + u8::from(axis == 6)),
        public(9 + u8::from(axis == 7)),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (owner, c) = PolicyCap::create(record, b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    owner
}
fn deployment(policy: &PolicyCap, b: &mut Budget<'_>) -> DeploymentCap {
    let (record, c) = Deployment::new(
        1000,
        1000,
        Service::new(2000, 2000).unwrap(),
        Measurement::new([3; 32], 13).unwrap(),
        Measurement::new([4; 32], 14).unwrap(),
        policy.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (owner, c) = DeploymentCap::create(record, b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    owner
}
fn key(policy: &PolicyCap, seed: u8, b: &mut Budget<'_>) -> Key {
    b.reserve_storage(32).unwrap();
    let (owner, c) = {
        let mut seed = [seed; 32];
        let result = Key::create_and_zeroize(&mut seed, policy.policy(), b).unwrap();
        assert_eq!(seed, [0; 32]);
        result
    };
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(32).unwrap();
    owner
}

struct Inputs {
    deployment: DeploymentCap,
    policy: PolicyCap,
    key: Key,
}
impl Inputs {
    fn fresh() -> Self {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let policy = policy(0, &mut b);
        let deployment = deployment(&policy, &mut b);
        let key = key(&policy, 7, &mut b);
        Self {
            deployment,
            policy,
            key,
        }
    }
    fn floor(&self) -> usize {
        input_storage(
            self.deployment.retained_storage(),
            self.policy.retained_storage(),
            self.key.retained_storage(),
        )
        .unwrap()
    }
    fn bind(self, b: &mut Budget<'_>) -> Result<(Trust, Storage)> {
        Trust::new(self.deployment, self.policy, self.key, b)
    }
    fn witnesses(&self, b: &mut Budget<'_>) -> Witnesses {
        let (deployment, c) = self.deployment.try_clone_for_transfer(b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (policy, c) = self.policy.try_clone_for_transfer(b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (key, c) = self.key.try_clone_for_transfer(b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        Witnesses([deployment, policy, key])
    }
}
struct Witnesses([File; 3]);
impl Witnesses {
    const WORK: usize = DeploymentCap::IO_WORK + PolicyCap::IO_WORK + Key::IO_WORK;
    const STORAGE: usize =
        DeploymentCap::FILE_STORAGE + PolicyCap::FILE_STORAGE + Key::FILE_STORAGE;
    fn assert_references(&self, expected: usize) {
        // Live unique-inode witnesses prevent FD-number and inode-reuse races.
        for file in &self.0 {
            let m = file.metadata().unwrap();
            let count = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
                .filter(|other| (m.dev(), m.ino()) == (other.dev(), other.ino()))
                .count();
            assert_eq!(count, expected);
        }
    }
    fn retire(self, b: &mut Budget<'_>) {
        drop(self);
        b.release_storage(Self::STORAGE).unwrap();
    }
}
fn bound_fixture() -> Trust {
    let inputs = Inputs::fresh();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(inputs.floor()).unwrap();
    let (trust, c) = inputs.bind(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    trust
}

#[test]
fn exact_bind_quota_returns_only_growth_and_actual_capabilities() {
    let inputs = Inputs::fresh();
    let floor = inputs.floor();
    let deployment = inputs.deployment.deployment().identity();
    let policy = inputs.policy.policy().identity();
    let mut w = Work::new(Trust::BIND_WORK);
    let mut b = Budget::new(&mut w, EXTRA + floor + Trust::SCRATCH);
    b.reserve_storage(EXTRA + floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (trust, c) = inputs.bind(&mut b).unwrap();
    assert_eq!(b.storage(), EXTRA + floor);
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(c.additional_storage(), Trust::GROWTH_STORAGE);
    assert_eq!(trust.retained_storage(), floor + Trust::GROWTH_STORAGE);
    assert_eq!(trust.deployment().deployment().identity(), deployment);
    assert_eq!(trust.policy().policy().identity(), policy);
    assert_eq!(trust.key_template().policy_identity(), policy);
    assert_eq!(b.work(), Trust::BIND_WORK);
    assert_eq!(b.peak_storage(), EXTRA + floor + Trust::SCRATCH);
    assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = trust.retained_storage();
    drop(trust);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn bind_one_short_floor_outer_and_nested_work_storage_refuse() {
    for mode in 0..6 {
        let inputs = Inputs::fresh();
        let floor = inputs.floor() - usize::from(mode == 0);
        let work = match mode {
            1 => ENTRY_WORK - 1,
            2 => LOCAL_WORK - 1,
            3 => Trust::BIND_WORK - 1,
            _ => Trust::BIND_WORK,
        };
        let scratch = if mode == 4 {
            Trust::OUTER_STORAGE - 1
        } else {
            Trust::SCRATCH - usize::from(mode == 5)
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let error = inputs.bind(&mut b).unwrap_err();
        match mode {
            0 => assert!(matches!(error, Error::Resource(Resource::Accounting))),
            1 | 2 => assert!(matches!(error, Error::Resource(Resource::Work(_)))),
            3 => assert!(matches!(
                error,
                Error::Capability(CapabilityError::Resource(Resource::Work(_)))
            )),
            4 => assert!(matches!(error, Error::Resource(Resource::Storage(_)))),
            _ => assert!(b.failed_storage().is_some()),
        }
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 | 2 => assert_eq!(b.work(), ENTRY_WORK),
            1 => assert_eq!(b.work(), 0),
            3 => assert_eq!(b.failed_work(), Some(Trust::BIND_WORK)),
            4 => assert_eq!(b.failed_storage(), Some(floor + Trust::OUTER_STORAGE)),
            _ => assert_eq!(b.failed_storage(), Some(floor + Trust::SCRATCH)),
        }
    }
}

#[test]
fn revalidation_exact_and_one_short_preserve_the_borrowed_owner() {
    let trust = bound_fixture();
    let retained = trust.retained_storage();
    for mode in 0..6 {
        let paid = retained - usize::from(mode == 1);
        let work = match mode {
            2 => ENTRY_WORK - 1,
            3 => LOCAL_WORK - 1,
            4 => Trust::REVALIDATION_WORK - 1,
            _ => Trust::REVALIDATION_WORK,
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, paid + Trust::SCRATCH - usize::from(mode == 5));
        b.reserve_storage(paid).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = trust.revalidate(&mut b);
        assert_eq!(result.is_ok(), mode == 0);
        assert_eq!(b.storage(), paid);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => {
                assert_eq!(b.work(), Trust::REVALIDATION_WORK);
                assert_eq!(b.peak_storage(), retained + Trust::SCRATCH);
            }
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            2 | 3 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            4 => assert_eq!(b.failed_work(), Some(Trust::REVALIDATION_WORK)),
            _ => assert_eq!(b.failed_storage(), Some(paid + Trust::SCRATCH)),
        }
    }
    let mut w = Work::new(Trust::REVALIDATION_WORK);
    let mut b = Budget::new(&mut w, retained + Trust::SCRATCH);
    b.reserve_storage(retained).unwrap();
    trust.revalidate(&mut b).unwrap();
    drop(trust);
    b.release_storage(retained).unwrap();
}

#[test]
fn full_policy_axes_refuse_before_key_validation() {
    for axis in 1..=7 {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let original = policy(0, &mut b);
        let deployment = deployment(&original, &mut b);
        let actual = policy(axis, &mut b);
        let key = key(&actual, 7 + u8::from(axis == 6), &mut b);
        let retired = original.retained_storage();
        drop(original);
        b.release_storage(retired).unwrap();
        let floor = b.storage();
        let before = b.work();
        assert!(matches!(
            Trust::new(deployment, actual, key, &mut b),
            Err(Error::ContextMismatch)
        ));
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work() - before, Trust::BIND_WORK - Key::IO_WORK);
        b.release_storage(floor).unwrap();
    }
}

#[test]
fn identical_verifying_key_with_another_generation_keeps_native_key_error() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let old = policy(0, &mut b);
    let actual = policy(1, &mut b);
    assert_eq!(
        old.policy().verifying_key(),
        actual.policy().verifying_key()
    );
    assert_ne!(old.policy().identity(), actual.policy().identity());
    let deployment = deployment(&actual, &mut b);
    let key = key(&old, 7, &mut b);
    let retired = old.retained_storage();
    drop(old);
    b.release_storage(retired).unwrap();
    let floor = b.storage();
    let before = b.work();
    assert!(matches!(
        Trust::new(deployment, actual, key, &mut b),
        Err(Error::Capability(CapabilityError::Rejected(
            "signing key is pinned to another native policy"
        )))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - before, Trust::BIND_WORK);
    b.release_storage(floor).unwrap();
}

#[test]
fn consumed_resource_refusal_closes_all_three_owners_but_not_witnesses() {
    for allowance in [ENTRY_WORK - 1, Trust::BIND_WORK - 1] {
        let inputs = Inputs::fresh();
        let owner_floor = inputs.floor();
        let mut w = Work::new(Witnesses::WORK + allowance);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(EXTRA + owner_floor).unwrap();
        let witnesses = inputs.witnesses(&mut b);
        witnesses.assert_references(2);
        let floor = b.storage();
        assert_eq!(floor, EXTRA + owner_floor + Witnesses::STORAGE);
        assert!(inputs.bind(&mut b).is_err());
        assert_eq!(b.storage(), floor);
        witnesses.assert_references(1);
        b.release_storage(owner_floor).unwrap();
        witnesses.retire(&mut b);
        assert_eq!(b.storage(), EXTRA);
    }
}

#[test]
fn all_retained_sealed_objects_are_rechecked_without_losing_ownership() {
    for changed in 0..3 {
        let inputs = Inputs::fresh();
        let floor = inputs.floor();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(floor).unwrap();
        let witnesses = inputs.witnesses(&mut b);
        let (trust, c) = inputs.bind(&mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let retained = trust.retained_storage();
        let old_mode = witnesses.0[changed].metadata().unwrap().mode() & 0o7777;
        rustix::fs::fchmod(
            &witnesses.0[changed],
            rustix::fs::Mode::from_raw_mode(0o666),
        )
        .unwrap();
        let entry = b.storage();
        assert!(matches!(
            trust.revalidate(&mut b),
            Err(Error::Capability(_))
        ));
        assert_eq!(b.storage(), entry);
        witnesses.assert_references(2);
        rustix::fs::fchmod(
            &witnesses.0[changed],
            rustix::fs::Mode::from_raw_mode(old_mode),
        )
        .unwrap();
        trust.revalidate(&mut b).unwrap();
        drop(trust);
        b.release_storage(retained).unwrap();
        witnesses.assert_references(1);
        witnesses.retire(&mut b);
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn successful_bind_and_revalidation_preserve_prior_denials_peak_and_work() {
    let inputs = Inputs::fresh();
    let floor = EXTRA + inputs.floor();
    let work = Trust::BIND_WORK + Trust::REVALIDATION_WORK;
    let peak = floor + Trust::GROWTH_STORAGE + 2 * Trust::SCRATCH;
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, peak);
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(work + 1).is_err());
    assert!(b.reserve_storage(peak - floor + 1).is_err());
    b.reserve_storage(peak - floor).unwrap();
    b.release_storage(peak - floor).unwrap();
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let ledger = b.work_ledger_identity_v1();
    let (trust, c) = inputs.bind(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    trust.revalidate(&mut b).unwrap();
    assert_eq!(b.work(), work);
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = trust.retained_storage();
    drop(trust);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn caller_unwind_drops_genuine_trust_before_restoring_the_outer_scope() {
    let inputs = Inputs::fresh();
    let owner_floor = inputs.floor();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(owner_floor).unwrap();
    let witnesses = inputs.witnesses(&mut b);
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = b.with_prepaid_scope(floor, 0, 0, 64, |b| {
            let (trust, c) = inputs.bind(b)?;
            b.reserve_storage(c.additional_storage())?;
            assert_eq!(
                trust.retained_storage(),
                owner_floor + Trust::GROWTH_STORAGE
            );
            panic!("caller after native trust binding");
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), Witnesses::WORK + Trust::BIND_WORK);
    assert_eq!(b.peak_storage(), floor + 64 + Trust::SCRATCH);
    assert!(b.work_ledger_identity_v1() == ledger);
    witnesses.assert_references(1);
    b.release_storage(owner_floor).unwrap();
    witnesses.retire(&mut b);
    assert_eq!(b.storage(), 0);
}

#[test]
fn checked_input_sum_and_original_ledger_overflow_refuse() {
    assert_eq!(input_storage(1, 2, 3).unwrap(), 6);
    assert_eq!(input_storage(usize::MAX - 2, 1, 1).unwrap(), usize::MAX);
    for values in [[usize::MAX, 1, 0], [usize::MAX - 1, 1, 1]] {
        assert!(matches!(
            input_storage(values[0], values[1], values[2]),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }
    for storage_overflow in [false, true] {
        let inputs = Inputs::fresh();
        let floor = if storage_overflow {
            usize::MAX
        } else {
            inputs.floor()
        };
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        b.reserve_storage(floor).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX).unwrap();
        }
        let error = inputs.bind(&mut b).unwrap_err();
        if storage_overflow {
            assert!(matches!(error, Error::Resource(Resource::Storage(_))));
            assert_eq!(b.work(), LOCAL_WORK);
            assert_eq!(b.failed_storage(), Some(usize::MAX));
        } else {
            assert!(matches!(error, Error::Resource(Resource::Work(_))));
            assert_eq!(b.work(), usize::MAX);
            assert_eq!(b.failed_work(), Some(usize::MAX));
        }
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn debug_and_errors_expose_no_key_seed_or_descriptor() {
    use std::error::Error as _;
    let trust = bound_fixture();
    let mut w = Work::new(0);
    let mut b = Budget::new(&mut w, trust.retained_storage());
    b.reserve_storage(trust.retained_storage()).unwrap();
    let text = format!("{trust:?}");
    assert!(text.contains("native-trust-custody-only"));
    for forbidden in [
        "07".repeat(32),
        format!("{:?}", [7; 32]),
        "File {".to_owned(),
        "key_template".to_owned(),
        "/proc/self/fd/".to_owned(),
    ] {
        assert!(!text.contains(&forbidden));
    }
    assert!(Error::ContextMismatch.source().is_none());
    assert!(Error::from(Resource::Accounting).source().is_some());
    assert!(
        Error::from(CapabilityError::Rejected("fixture"))
            .source()
            .is_some()
    );
}
