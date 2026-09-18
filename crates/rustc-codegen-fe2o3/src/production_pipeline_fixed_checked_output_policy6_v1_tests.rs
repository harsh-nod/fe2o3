//! Resource and genuine native-source accessor tests; no collector owner is fabricated.
use super::*;
use std::cell::Cell;

#[test]
fn direct_original_accessors_are_one_actual_v12_n_not_compatibility_bytes() {
    use fe2o3_kernel_ir::{
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1 as DOMAIN,
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1 as POLICY,
    };
    use sha2::{Digest, Sha256};
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        crate::production_ranked_projection_v1::with_backend_checked_output_policy6_owned_v1(
            profile,
            |admitted, _| {
                let actual = direct_native_source(&admitted);
                let source = admitted.source_semantic_kir();
                assert!(std::ptr::eq(
                    actual,
                    source.pre_ranked_executable().unwrap()
                ));
                assert!(std::ptr::eq(actual.module(), source.module()));
                let bytes = actual.canonical().canonical_bytes();
                let mut digest = Sha256::new();
                digest.update(u32::try_from(DOMAIN.len()).unwrap().to_le_bytes());
                digest.update(DOMAIN);
                digest.update(POLICY.to_le_bytes());
                digest.update(u64::try_from(bytes.len()).unwrap().to_le_bytes());
                digest.update(bytes);
                let digest: [u8; 32] = digest.finalize().into();
                assert_eq!(actual.canonical().identity().digest(), &digest);
                assert_ne!(bytes, source.canonical_kernel_ir_bytes());
                assert_ne!(
                    actual.canonical().identity().digest(),
                    source.canonical_kernel_ir_identity().digest()
                );
            },
        );
    }
}

#[test]
fn fixed_facade_accounts_only_additional_in_place_header_for_each_source_shape() {
    let total = std::mem::size_of::<FixedCheckedOutputProductionCompilationPolicy6V1>();
    assert_eq!(
        additional_header(SourcePolicy::RawEmpty).unwrap() + std::mem::size_of::<Direct>(),
        total,
    );
    assert_eq!(
        additional_header(SourcePolicy::UnitLocal).unwrap() + std::mem::size_of::<Erased>(),
        total,
    );
    assert!(additional_header(SourcePolicy::RawEmpty).unwrap() > 0);
    assert!(additional_header(SourcePolicy::UnitLocal).unwrap() > 0);
}

#[test]
fn fixed_facade_header_work_and_storage_are_prepaid_at_exact_limits() {
    for source in [SourcePolicy::RawEmpty, SourcePolicy::UnitLocal] {
        let additional = additional_header(source).unwrap();
        let inherited = 41;
        for (work_limit, storage_limit, expected) in [
            (HEADER_WORK, inherited + additional, true),
            (HEADER_WORK - 1, inherited + additional, false),
            (HEADER_WORK, inherited + additional - 1, false),
        ] {
            let entered = Cell::new(false);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(inherited).unwrap();
            let identity = budget.work_ledger_identity_v1();
            let result = transfer_header(source, &mut budget, || {
                entered.set(true);
                Ok(7_u8)
            });
            assert_eq!(result.is_ok(), expected);
            assert_eq!(entered.get(), expected);
            assert_eq!(budget.storage(), inherited);
            assert!(budget.work_ledger_identity_v1() == identity);
            if expected {
                assert_eq!(result.unwrap(), 7);
                assert_eq!(budget.work(), HEADER_WORK);
                assert_eq!(budget.peak_storage(), inherited + additional);
            } else if work_limit < HEADER_WORK {
                assert!(matches!(
                    result,
                    Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                        CheckedOutputPolicy6StageErrorV1::Resource(Resource::Work(_))
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                        CheckedOutputPolicy6StageErrorV1::Resource(Resource::Storage(_))
                    ))
                ));
            }
        }
    }
}

#[test]
fn fixed_facade_header_error_and_panic_cleanup_preserve_the_original_failure() {
    for source in [SourcePolicy::RawEmpty, SourcePolicy::UnitLocal] {
        let inherited = 73;
        let mut work = Work::new(HEADER_WORK * 2);
        let mut budget = Budget::new(&mut work, inherited + additional_header(source).unwrap());
        budget.reserve_storage(inherited).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result: Result<(), _> = transfer_header(source, &mut budget, || {
            Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::NativePublicationUnavailable,
            ))
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::NativePublicationUnavailable
            ))
        ));
        assert_eq!(budget.storage(), inherited);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), _> =
                transfer_header(source, &mut budget, || std::panic::panic_any(0x414_u32));
        }));
        assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 0x414);
        assert_eq!(budget.storage(), inherited);
        assert_eq!(budget.work(), HEADER_WORK * 2);
        assert!(budget.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn fixed_facade_combined_receipt_requires_the_real_inherited_plus_header_ceiling() {
    for source in [SourcePolicy::RawEmpty, SourcePolicy::UnitLocal] {
        let additional = additional_header(source).unwrap();
        let inherited = 97;
        for limit in [inherited + additional, inherited + additional - 1] {
            let mut work = Work::new(0);
            let mut budget = Budget::new(&mut work, limit);
            let identity = budget.work_ledger_identity_v1();
            let result = combined_storage_floor(inherited, additional, &mut budget);
            if limit == inherited + additional {
                assert_eq!(result.unwrap(), limit);
                assert_eq!(budget.peak_storage(), limit);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                        CheckedOutputPolicy6StageErrorV1::Resource(Resource::Storage(error))
                    )) if error.actual() == inherited + additional && error.limit() == limit
                ));
            }
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.work(), 0);
            assert!(budget.work_ledger_identity_v1() == identity);
        }
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(matches!(
            combined_storage_floor(usize::MAX, additional, &mut budget),
            Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
        assert_eq!(budget.storage(), 0);
        budget.reserve_storage(1).unwrap();
        assert!(matches!(
            combined_storage_floor(inherited, additional, &mut budget),
            Err(ProductionPipelineError::CheckedOutputPolicy6Stage(
                CheckedOutputPolicy6StageErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.storage(), 1);
    }
}
