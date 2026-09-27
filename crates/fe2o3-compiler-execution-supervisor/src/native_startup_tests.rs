use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorService,
    CompilerExecutionIssuerMeasurementV1 as Digest,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    os::unix::fs::MetadataExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 1 << 30;
const INPUT: usize = Policy::FILE_STORAGE + Deployment::FILE_STORAGE;
const CONTEXT_WORK: usize = Policy::IO_WORK + POLICY_WORK + Deployment::ADMISSION_WORK;
fn policy(axis: usize, b: &mut Budget<'_>) -> Policy {
    let key = |seed: u8| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (record, charge) = PolicyRecord::new(
        7 + u64::from(axis == 1),
        Digest::new([1 + u8::from(axis == 2); 32], 11 + u64::from(axis == 3)).unwrap(),
        Digest::new([2 + u8::from(axis == 4); 32], 12 + u64::from(axis == 5)).unwrap(),
        key(7 + u8::from(axis == 6)),
        key(9 + u8::from(axis == 7)),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (policy, charge) = Policy::create(record, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    policy
}
fn deployment(policy: &Policy, b: &mut Budget<'_>) -> Deployment {
    let (record, charge) = DeploymentRecord::new(
        1000,
        1000,
        AnchorService::new(2000, 2000).unwrap(),
        Digest::new([3; 32], 13).unwrap(),
        Digest::new([4; 32], 14).unwrap(),
        policy.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (deployment, charge) = Deployment::create(record, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    deployment
}
fn files(p: &Policy, d: &Deployment, b: &mut Budget<'_>) -> (File, File) {
    let (policy, charge) = p.try_clone_for_transfer(b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (deployment, charge) = d.try_clone_for_transfer(b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    (policy, deployment)
}
fn refs(file: &File) -> usize {
    let stat = file.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (stat.dev(), stat.ino()))
        .count()
}

#[test]
fn startup_context_owns_genuine_same_family_capabilities_on_original_ledger() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let p = policy(0, &mut b);
    let d = deployment(&p, &mut b);
    let (pf, df) = files(&p, &d, &mut b);
    let before = b.work();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    b.with_prepaid_scope(INPUT, 0, 0, 0, |b| -> Result<()> {
        let context = admit_context(pf, df, b)?;
        assert_eq!(context.policy.policy().identity(), p.policy().identity());
        assert_eq!(
            context.deployment.deployment().identity(),
            d.deployment().identity()
        );
        assert_eq!(
            b.storage(),
            floor + context.policy.retained_storage() + context.deployment.retained_storage()
                - INPUT
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - before, CONTEXT_WORK);
    assert!(b.work_ledger_identity_v1() == ledger);
    b.release_storage(INPUT).unwrap();
}

#[test]
fn every_policy_axis_and_swapped_roles_refuse_before_protected_profile_or_key() {
    for axis in 1..=8 {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let p = policy(0, &mut b);
        let d = deployment(&p, &mut b);
        let wrong = policy(axis, &mut b);
        let (pf, df) = files(&wrong, &d, &mut b);
        let floor = b.storage();
        let result = b.with_prepaid_scope(INPUT, 0, 0, 0, |b| {
            if axis == 8 {
                admit_context(df, pf, b)
            } else {
                admit_context(pf, df, b)
            }
        });
        assert!(matches!(result, Err(Error::Capability(_))));
        assert_eq!(b.storage(), floor);
        b.release_storage(INPUT).unwrap();
    }
}

#[test]
fn context_refusal_and_unwind_close_owned_aliases_not_the_original_witnesses() {
    for unwind in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let p = policy(0, &mut b);
        let d = deployment(&p, &mut b);
        let (pw, dw) = files(&p, &d, &mut b);
        let (pf, df) = files(&p, &d, &mut b);
        let before = (refs(&pw), refs(&dw));
        let floor = b.storage();
        let work_before = b.work();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = b.with_prepaid_scope(INPUT, 0, 0, 0, |b| {
                let _context = admit_context(pf, df, b)?;
                if unwind {
                    panic!("after native startup context admission");
                }
                Err(Error::Invalid("injected before profile and key"))
            });
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!((refs(&pw), refs(&dw)), (before.0 - 1, before.1 - 1));
        assert_eq!(b.storage(), floor);
        assert!(b.work() > work_before);
        b.release_storage(INPUT).unwrap();
    }
}

#[test]
fn context_exact_and_short_work_preserve_storage_and_prior_denials() {
    for short in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let p = policy(0, &mut b);
        let d = deployment(&p, &mut b);
        let (pf, df) = files(&p, &d, &mut b);
        let floor = b.storage();
        assert!(b.reserve_storage(LIMIT).is_err());
        let denied = b.failed_storage();
        b.charge_work(LIMIT - b.work() - CONTEXT_WORK + usize::from(short))
            .unwrap();
        let result = b.with_prepaid_scope(INPUT, 0, 0, 0, |b| admit_context(pf, df, b).map(drop));
        if short {
            assert!(result.is_err());
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
        } else {
            result.unwrap();
            assert_eq!(b.work(), LIMIT);
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.failed_storage(), denied);
        b.release_storage(INPUT).unwrap();
    }
}

#[test]
fn full_input_floor_includes_every_descriptor_and_both_maximum_images() {
    assert_eq!(
        INPUT_STORAGE,
        6 * FILE_STORAGE
            + 2 * (IMAGE_MAX + FILE_STORAGE)
            + Policy::FILE_STORAGE
            + Deployment::FILE_STORAGE
            + Key::FILE_STORAGE
    );
    assert!(NATIVE_ISSUER_PROCESS_STORAGE_V2 > INPUT_STORAGE + 3 * IMAGE_MAX);
}
