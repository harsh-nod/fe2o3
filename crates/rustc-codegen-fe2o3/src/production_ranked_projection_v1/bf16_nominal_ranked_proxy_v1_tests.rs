//! Synthetic row-mapping/custody tests only. No fake C1/C3/source owner and no
//! final-pass, genuine-source, ranked, normal, formal or LLVM success claim.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 37;

fn roots() -> [DigestV1; 6] {
    std::array::from_fn(|index| DigestV1::from_untrusted_bytes([index as u8 + 1; 32]))
}
fn required() -> TensorLayoutContractV1 {
    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        .with_zero_filled_predicate_inputs()
}

#[test]
fn synthetic_row_keeps_source_roots_and_qualified_result_identity() {
    let inputs = roots();
    let row = operation_from_parts(required(), 64, inputs).unwrap();
    let ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence,
        active_lanes,
        binding: Some(binding),
    } = row
    else {
        panic!("exact proxy row");
    };
    assert_eq!(contract, required());
    assert_eq!(convergence, TensorConvergenceAttr::UniformSubgroup);
    assert_eq!(active_lanes, 64);
    assert_eq!(binding.context_root(), inputs[0]);
    assert_eq!(binding.lane_root(), inputs[1]);
    assert_eq!(binding.lhs_root(), inputs[2]);
    assert_eq!(binding.rhs_root(), inputs[3]);
    assert_eq!(binding.accumulator_root(), inputs[4]);
    assert_eq!(binding.result_root(), inputs[5]);
    assert_ne!(binding.accumulator_root(), binding.result_root());
    assert_eq!(
        binding.argument_count(),
        4,
        "logical source arguments, not twelve scalar formals"
    );
}

#[test]
fn synthetic_nonzero_root_requirement_covers_every_binding_position() {
    for index in 0..6 {
        let mut inputs = roots();
        inputs[index] = DigestV1::from_untrusted_bytes([0; 32]);
        assert!(matches!(
            operation_from_parts(required(), 64, inputs),
            Err(Error::Unavailable(
                "nominal ranked proxy capability roots differ"
            ))
        ));
    }
}

#[test]
fn synthetic_wrong_profile_or_active_lane_count_refuses() {
    assert!(matches!(
        operation_from_parts(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            64,
            roots(),
        ),
        Err(Error::Unavailable(
            "nominal ranked proxy tensor profile differs"
        ))
    ));
    for lanes in [0, 32, 63, 65] {
        assert!(matches!(
            operation_from_parts(required(), lanes, roots()),
            Err(Error::Unavailable(
                "nominal ranked proxy tensor profile differs"
            ))
        ));
    }
}

#[test]
fn synthetic_scope_exact_and_one_short_boundaries_preserve_prefix() {
    for kind in 0..3 {
        let entered = Cell::new(false);
        let body = |_budget: &mut Budget<'_>| {
            entered.set(true);
            Ok(())
        };
        let bytes = scope_storage::<()>(std::mem::size_of_val(&body)).unwrap();
        let mut work = Work::new(7 + SCOPE_WORK - usize::from(kind == 1));
        let mut budget = Budget::new(&mut work, FLOOR + bytes - usize::from(kind == 2));
        budget.charge_work(7).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = with_owned_scope(&mut budget, body);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR);
        match kind {
            0 => {
                assert_eq!(result, Ok(()));
                assert!(entered.get());
                assert_eq!(budget.work(), 7 + SCOPE_WORK);
                assert_eq!(budget.peak_storage(), FLOOR + bytes);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert!(!entered.get());
                assert_eq!(budget.work(), 7);
                assert_eq!(budget.failed_work(), Some(7 + SCOPE_WORK));
                assert_eq!(budget.failed_storage(), None);
            }
            _ => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert!(!entered.get());
                assert_eq!(budget.work(), 7);
                assert_eq!(budget.failed_work(), None);
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}

#[test]
fn synthetic_callback_success_error_and_panic_keep_surplus_and_work() {
    for outcome in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = with_owned_scope(&mut budget, |budget| {
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match outcome {
                0 => Ok(7),
                1 => Err(Error::Unavailable("synthetic proxy callback")),
                _ => panic!("synthetic proxy unwind"),
            }
        });
        assert_eq!(
            result,
            match outcome {
                0 => Ok(7),
                1 => Err(Error::Unavailable("synthetic proxy callback")),
                _ => Err(Error::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + 23);
        assert_eq!(budget.work(), SCOPE_WORK + 17);
    }
}

#[test]
fn synthetic_ignored_work_or_storage_denial_stays_sticky() {
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_owned_scope(&mut budget, |budget| {
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), storage);
        assert_eq!(budget.failed_work().is_some(), !storage);
    }
}

#[test]
fn synthetic_floor_undercut_is_not_repaired() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let body = |budget: &mut Budget<'_>| {
        budget.release_storage(1)?;
        Ok(())
    };
    let bytes = scope_storage::<()>(std::mem::size_of_val(&body)).unwrap();
    assert_eq!(
        with_owned_scope(&mut budget, body),
        Err(Resource::Accounting.into())
    );
    assert_eq!(budget.storage(), FLOOR + bytes - 1);
}

#[test]
fn synthetic_replaced_budget_is_not_repaired_or_charged() {
    let mut original_work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut original_work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let replacement = Budget::new(&mut foreign_work, LIMIT);
    assert_eq!(
        with_owned_scope(&mut budget, |budget| {
            *budget = replacement;
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(budget.work_ledger_identity_v1() != identity);
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}

#[test]
fn synthetic_preexisting_denial_refuses_before_callback() {
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.charge_work(2).is_err());
    let result = with_owned_scope(&mut budget, |_| -> Result<()> {
        panic!("sticky failure must not enter proxy");
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!((budget.storage(), budget.work()), (0, 0));
    assert_eq!(budget.failed_work(), Some(2));
}

#[test]
fn synthetic_explicit_resource_errors_survive_and_panic_payload_drops() {
    for error in [
        Resource::Allocation,
        Resource::Arithmetic,
        Resource::Accounting,
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert_eq!(
            with_owned_scope(&mut budget, |_| -> Result<()> { Err(error.into()) }),
            Err(Error::Resource(error))
        );
        assert_eq!(budget.storage(), 0);
    }
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = with_owned_scope(&mut budget, |_| -> Result<()> {
        std::panic::panic_any(DropFlag(dropped.clone()));
    });
    assert_eq!(result, Err(Error::CallbackPanicked));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn synthetic_large_callback_and_result_headers_are_explicit_and_checked() {
    let big = [7u8; 8192];
    let body = move |_budget: &mut Budget<'_>| -> Result<[u8; 8192]> {
        assert_eq!(big[8191], 7);
        Ok(big)
    };
    let callback_bytes = std::mem::size_of_val(&body);
    assert_eq!(callback_bytes, 8192);
    let bytes = scope_storage::<[u8; 8192]>(callback_bytes).unwrap();
    assert!(bytes >= 4 * 8192);
    let mut work = Work::new(SCOPE_WORK);
    let mut budget = Budget::new(&mut work, bytes);
    let result = with_owned_scope(&mut budget, body).unwrap();
    assert_eq!(result, [7u8; 8192]);
    assert_eq!(budget.peak_storage(), bytes);
    assert_eq!(budget.storage(), 0);
    assert_eq!(
        scope_storage::<()>(usize::MAX),
        Err(Resource::Arithmetic.into())
    );
}

#[test]
fn synthetic_fixed_binding_hash_input_is_bounded() {
    // Five fixed-arity legacy input-root hashes; the qualified C3 result hash is
    // borrowed, not rehashed or recreated in a caller namespace. Two operand
    // hashes have five words, and three simple roots have one word each.
    let domain = super::super::TENSOR_CAPABILITY_ROOT_DOMAIN_V1.len();
    let bytes = 5 * (8 + domain + 1 + 8) + (5 + 5 + 1 + 1 + 1) * 8;
    assert!(bytes < BINDING_WORK);
    assert_eq!(SOURCE_ARGUMENT_COUNT, 4);
    assert_eq!(JOIN_WORK, 64);
}
