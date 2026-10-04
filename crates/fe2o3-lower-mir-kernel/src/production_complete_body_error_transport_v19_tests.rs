//! Allocation-free error transport controls, not source or ranked-proof evidence.
use super::*;
use fe2o3_kernel_analysis::KernelCheckPassKindV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, FormalGuardedMemoryResourceErrorV1, FormalMemoryObligationError,
    KernelId, RegionValidationError,
};
use fe2o3_pliron::{
    ProductionAnalysisResourcePhaseV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelErrorV1, ProductionSemanticExpressionErrorV2, ProductionSessionErrorV1,
};

#[test]
fn every_resource_variant_keeps_payload_and_ledger_state() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
    let mut budget = ArgumentBudgetV1::new(&mut work, 7);
    budget.charge_work(3).unwrap();
    budget.reserve_storage(3).unwrap();
    let work_error = budget.charge_work(3).unwrap_err();
    let storage_error = budget.reserve_storage(5).unwrap_err();
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    for expected in [
        work_error,
        storage_error,
        ArgumentResourceV1::Allocation,
        ArgumentResourceV1::Accounting,
        ArgumentResourceV1::Arithmetic,
    ] {
        let auxiliary = CompleteBodyAuxErrorV19::from(expected);
        let ProductionCompleteBodyCheckErrorV19::Resource(actual) = auxiliary.into() else {
            panic!("resource error changed category");
        };
        assert_eq!(actual, expected);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before,
        );
    }
}

#[test]
fn relation_text_keeps_exact_existing_public_variant() {
    let detail = "V19 projection operand has no actual SSA definition";
    let ProductionCompleteBodyCheckErrorV19::Relation(actual) =
        CompleteBodyAuxErrorV19::Relation(detail).into()
    else {
        panic!("relation error changed category");
    };
    assert_eq!(actual, detail);
    assert_eq!(actual.as_ptr(), detail.as_ptr());
}

#[test]
fn ranked_recipe_keeps_typed_stage_reason_and_all_numeric_fields() {
    for expected in [
        ProductionRankedKernelErrorV1::ResourceLimit {
            resource: "projection operand rows",
            limit: 48,
            actual: 49,
        },
        ProductionRankedKernelErrorV1::InvalidSemanticExpression(
            ProductionSemanticExpressionErrorV2::UnboundLoad,
        ),
        ProductionRankedKernelErrorV1::NonCanonicalValueId {
            expected: u32::MAX,
            actual: 23,
        },
        ProductionRankedKernelErrorV1::CrossBlockDefinitionRequiresArgument {
            definition_block: 1,
            use_block: 3,
        },
    ] {
        let auxiliary = CompleteBodyAuxErrorV19::RankedRecipe(expected.clone());
        let ProductionCompleteBodyCheckErrorV19::RankedRecipe(actual) = auxiliary.into() else {
            panic!("ranked recipe error changed category");
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn formal_errors_keep_owned_diagnostics_and_all_typed_variants() {
    let kernel = KernelId::new("retained_missing_kernel");
    let allocation = kernel.as_str().as_ptr();
    let ProductionCompleteBodyCheckErrorV19::Formal(FormalMemoryObligationError::MissingKernel {
        kernel,
    }) = CompleteBodyAuxErrorV19::Formal(FormalMemoryObligationError::MissingKernel { kernel })
        .into()
    else {
        panic!("missing kernel changed category");
    };
    assert_eq!(kernel.as_str(), "retained_missing_kernel");
    assert_eq!(kernel.as_str().as_ptr(), allocation);

    let mut module = Module::new("missing_entry");
    module.kernels.push(Kernel::new(
        "kernel",
        FunctionId::new("absent"),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let diagnostics = verify_module(&module).unwrap_err();
    let rows = diagnostics.diagnostics().as_ptr();
    let count = diagnostics.diagnostics().len();
    let ProductionCompleteBodyCheckErrorV19::Formal(FormalMemoryObligationError::InvalidModule(
        actual,
    )) = CompleteBodyAuxErrorV19::Formal(FormalMemoryObligationError::InvalidModule(diagnostics))
        .into()
    else {
        panic!("verification diagnostics changed category");
    };
    assert_eq!(actual.diagnostics().as_ptr(), rows);
    assert_eq!(actual.diagnostics().len(), count);

    for expected in [
        FormalMemoryObligationError::InvalidInvocationRange(
            RegionValidationError::RegionEndOverflow {
                byte_offset: u64::MAX,
                byte_length: 4,
                invocation_index: 127,
            },
        ),
        FormalMemoryObligationError::GuardedResource(FormalGuardedMemoryResourceErrorV1::Storage {
            actual: 97,
            limit: 96,
        }),
    ] {
        let auxiliary = CompleteBodyAuxErrorV19::Formal(expected.clone());
        let ProductionCompleteBodyCheckErrorV19::Formal(actual) = auxiliary.into() else {
            panic!("formal error changed category");
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn public_ranked_error_keeps_complete_typed_session_payload() {
    // The public Ranked carrier is deliberately not replaced by the auxiliary
    // type. Its complete report-bearing error type and nested stage remain.
    let error = ProductionCompleteBodyCheckErrorV19::Ranked(
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::AnalysisResourceLimit {
            phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
            producing_pass: Some(KernelCheckPassKindV1::SemanticRefinement),
            resource: "retained semantic payload",
        }),
    );
    let ProductionCompleteBodyCheckErrorV19::Ranked(ProductionRankedCompileErrorV1::Session(
        ProductionSessionErrorV1::AnalysisResourceLimit {
            phase,
            producing_pass,
            resource,
        },
    )) = error
    else {
        panic!("public ranked/session error changed category");
    };
    assert_eq!(phase, ProductionAnalysisResourcePhaseV1::SemanticRefinement);
    assert_eq!(
        producing_pass,
        Some(KernelCheckPassKindV1::SemanticRefinement)
    );
    assert_eq!(resource, "retained semantic payload");
}
