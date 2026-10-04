//! Checked source calls through the production local-allowance adapter.
use super::super::resource_tests::argument_correspondence_tests::native_helper_argument_owner;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

struct Probe {
    result: Result<(), &'static str>,
    consumed: usize,
    local_work: usize,
    peak: usize,
    failed: bool,
}

fn run_calls(
    owner: &ProductionPreRankedKirOwnerV1,
    correspondence: &SemanticKirCorrespondenceV1,
    local_work: usize,
    local_storage: usize,
    expected_eligible: bool,
) -> Probe {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 128 << 20);
    budget.charge_work(23).unwrap();
    budget.reserve_storage(4096).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut allowance = TranslationAllowanceV1::new(&budget, local_work, local_storage);
    let module = owner.executable.module();
    let mut result = Ok(());
    let mut visited = 0;
    for root in owner.semantic_ssa.source_semantic().roots() {
        let association = owner
            .correspondence
            .lowered_functions
            .iter()
            .find(|row| row.correspondence_owner == *root && row.semantic_function == *root)
            .unwrap();
        let entry = module
            .functions
            .iter()
            .find(|function| function.id == association.kernel_ir_function)
            .unwrap();
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: Some(&mut allowance),
            failed: false,
            resource_error: None,
        };
        result = with_native_helper_values(
            &owner.semantic_ssa,
            module,
            correspondence,
            *root,
            entry,
            &mut meter,
            |context, meter| {
                let mut calls = 0;
                for block in &entry.body.as_ref().unwrap().blocks {
                    for (ordinal, operation) in block.operations.iter().enumerate() {
                        if !matches!(operation.kind, OperationKind::Call { .. }) {
                            continue;
                        }
                        calls += 1;
                        let call = context.root_call(
                            entry,
                            FunctionOperationLocation::new(block.id, ordinal),
                            operation,
                            meter,
                        );
                        if expected_eligible {
                            call?;
                        } else {
                            assert!(call.is_err());
                        }
                        assert!(!meter.exhausted());
                    }
                }
                assert_eq!(calls, 2);
                visited += 1;
                Ok(())
            },
        );
        if result.is_err() {
            assert!(meter.exhausted());
            assert_eq!(
                meter.resource_error, None,
                "only the local allowance is exhausted"
            );
            assert!(meter.work(0).is_err());
            break;
        }
    }
    if result.is_ok() {
        assert_eq!(visited, 2);
    }
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), 4096);
    Probe {
        result,
        consumed: budget.work() - 23,
        local_work: allowance.work,
        peak: budget.peak_storage() - 4096,
        failed: allowance.failed,
    }
}

#[test]
fn checked_calls_share_exact_work_and_storage_allowances_across_roots() {
    for shape in [None, Some(false), Some(true)] {
        let owner = native_helper_argument_owner(shape);
        let measured = run_calls(&owner, &owner.correspondence, 10_000_000, 32 << 20, true);
        measured.result.unwrap();
        assert_eq!(measured.consumed, measured.local_work);
        assert!(!measured.failed);
        assert!(measured.consumed > 0 && measured.peak > 0);
        let exact = run_calls(
            &owner,
            &owner.correspondence,
            measured.consumed,
            measured.peak,
            true,
        );
        exact.result.unwrap();
        assert_eq!(exact.consumed, measured.consumed);
        assert_eq!(exact.local_work, measured.consumed);
        assert_eq!(exact.peak, measured.peak);
        assert!(!exact.failed);
        for (work, storage) in [
            (measured.consumed - 1, measured.peak),
            (measured.consumed, measured.peak - 1),
        ] {
            let short = run_calls(&owner, &owner.correspondence, work, storage, true);
            assert!(short.result.is_err() && short.failed);
            assert_eq!(short.consumed, short.local_work);
            assert!(short.consumed <= work);
            assert!(short.peak <= storage);
        }
    }
}

#[test]
fn semantic_refusal_debits_query_work_without_exhausting_the_allowance() {
    let owner = native_helper_argument_owner(Some(true));
    let mut changed = owner.correspondence.clone();
    for row in &mut changed.parameter_component_bindings {
        if row.semantic_function == SemanticFunctionIdV1::from_index(0) {
            row.semantic_component_type = SemanticTypeIdV1::from_index(0);
        }
    }
    let probe = run_calls(&owner, &changed, 10_000_000, 32 << 20, false);
    probe.result.unwrap();
    assert_eq!(probe.consumed, probe.local_work);
    assert!(probe.consumed > 0);
    assert!(!probe.failed);
}
