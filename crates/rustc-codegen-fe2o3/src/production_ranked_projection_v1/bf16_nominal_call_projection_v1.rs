//! N2a only: same-owner physical-effect/source-MFMA contract join.
//! No ranked admission, generic helper summary, capability transfer or FnABI
//! interpretation. N2b/N2c must consume the outstanding nominal obligations.
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectErrorV1,
    CanonicalKirCallEffectsV1, CanonicalKirInventoryV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, MatrixOperationKind, OperationKind,
    TensorLayoutContractV1,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as Error, CheckedBf16NominalCallV1, ProductionPreRankedKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDirectCallV1, SemanticFunctionIdV1, SemanticMfmaAccumulatorDistributionV1,
    SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1, SemanticMfmaRegisterDistributionV1,
    SemanticTerminatorKindV1,
};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, Error>;

/// A required future join, not a scalar-empty decision or a completed proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequiredNominalProjectionV1 {
    CallerCapabilitiesTensorLayoutFullWaveAndExactResults,
}

/// Lexically borrowed candidate only. There is deliberately no conversion to
/// DefinedCallableEmptyEffectDecisionV1, UnitLocal, ranked or normal owners.
/// CompleteEmpty here describes physical-memory/compiler-order effects only;
/// tensor/convergence and exact component transport remain mandatory inputs to
/// a future source-ranked projection.
pub(crate) struct CheckedNominalCallProjectionV1<'a> {
    call: &'a CheckedBf16NominalCallV1<'a>,
    source_matrix: &'a SemanticDirectCallV1,
    source_intrinsic: &'a SemanticCompilerIntrinsicOperationV1,
    physical_effects: Decision,
    tensor_contract: TensorLayoutContractV1,
}
impl CheckedNominalCallProjectionV1<'_> {
    pub(crate) const fn call(&self) -> &CheckedBf16NominalCallV1<'_> {
        self.call
    }
    pub(crate) const fn source_matrix_call(&self) -> &SemanticDirectCallV1 {
        self.source_matrix
    }
    pub(crate) const fn source_matrix_intrinsic(&self) -> &SemanticCompilerIntrinsicOperationV1 {
        self.source_intrinsic
    }
    pub(crate) const fn physical_effect_decision(&self) -> Decision {
        self.physical_effects
    }
    pub(crate) const fn required_tensor_contract(&self) -> TensorLayoutContractV1 {
        self.tensor_contract
    }
    pub(crate) const fn required_projection(&self) -> RequiredNominalProjectionV1 {
        RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults
    }
    pub(crate) fn required_result_permutation(&self) -> [u8; 4] {
        self.call.emission().return_permutation()
    }
}

fn require(condition: bool, why: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Unavailable(why))
    }
}
fn effect_error(error: CanonicalKirCallEffectErrorV1) -> Error {
    match error {
        CanonicalKirCallEffectErrorV1::Resource(error) => Error::Resource(error),
        _ => Error::Unavailable("nominal independent physical-effect report differs"),
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or(Error::Resource(Resource::Arithmetic))
}
fn scratch<R>() -> Result<usize> {
    add(
        size_of::<CheckedNominalCallProjectionV1<'static>>() + 4096,
        size_of::<R>().checked_mul(2).ok_or(Resource::Arithmetic)?,
    )
}

fn require_effect_inventory(
    inventory: &CanonicalKirInventoryV1<'_>,
    effects: &CanonicalKirCallEffectsV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(1)?;
    require(
        effects.belongs_to(inventory),
        "nominal effects belong to another inventory",
    )
}

// CompleteNonempty is not silently supported; Incomplete is never a proof.
// Even CompleteEmpty returns a nominal outstanding-obligation category, not any
// existing generic scalar/helper admission category.
fn require_nominal_decision(
    decision: Decision,
    budget: &mut Budget<'_>,
) -> Result<RequiredNominalProjectionV1> {
    budget.charge_work(1)?;
    require(
        decision == Decision::CompleteEmpty,
        "nominal helper effects are not the closed empty physical roster",
    )?;
    Ok(RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults)
}

fn source_contract<'a>(
    call: &'a CheckedBf16NominalCallV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<(
    &'a SemanticDirectCallV1,
    &'a SemanticCompilerIntrinsicOperationV1,
    TensorLayoutContractV1,
)> {
    budget.charge_work(24)?;
    let emission = call.emission();
    let semantic = emission.owner().semantic_ssa().source_semantic();
    let helper = semantic
        .functions()
        .get(emission.helper().index() as usize)
        .ok_or(Error::Unavailable("nominal helper source function absent"))?;
    let block = helper
        .blocks()
        .get(emission.source_matrix_block().index() as usize)
        .ok_or(Error::Unavailable(
            "nominal helper source Matrix block absent",
        ))?;
    let SemanticTerminatorKindV1::Call(source_call) = block.terminator().kind() else {
        return Err(Error::Unavailable(
            "nominal source Matrix is not an actual call",
        ));
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) = semantic
        .callables()
        .get(source_call.callee().index() as usize)
    else {
        return Err(Error::Unavailable("nominal source Matrix intrinsic absent"));
    };
    let SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
        lhs_fragment,
        rhs_fragment,
        accumulator_fragment,
        lhs,
        rhs,
        accumulator,
        ..
    } = operation
    else {
        return Err(Error::Unavailable("nominal source intrinsic is not MFMA"));
    };
    require(
        source_call.arguments().len() == 4
            && source_call.arguments()[1].ty() == *lhs_fragment
            && source_call.arguments()[2].ty() == *rhs_fragment
            && source_call.arguments()[3].ty() == *accumulator_fragment
            && source_call
                .destination()
                .is_some_and(|d| d.place().ty() == *accumulator_fragment),
        "nominal source MFMA operand/result types differ",
    )?;
    require(
        lhs.role == SemanticMfmaOperandRoleV1::A
            && rhs.role == SemanticMfmaOperandRoleV1::B
            && lhs.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && rhs.profile == lhs.profile
            && accumulator.profile == lhs.profile
            && lhs.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
            && rhs.register_distribution == lhs.register_distribution
            && accumulator.distribution == SemanticMfmaAccumulatorDistributionV1::RowMajor
            && lhs.wave_width == 64
            && rhs.wave_width == 64
            && accumulator.wave_width == 64,
        "nominal source MFMA role/layout/Wave64 contracts differ",
    )?;
    let OperationKind::Matrix(matrix) = &call.matrix().operation.kind else {
        return Err(Error::Unavailable("nominal canonical Matrix absent"));
    };
    require(
        matches!(matrix.kind, MatrixOperationKind::MultiplyAccumulate { .. }),
        "nominal canonical Matrix operation differs",
    )?;
    let expected = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        .with_zero_filled_predicate_inputs();
    require(
        matrix.tensor_layout == Some(expected) && matrix.active_lanes == 64,
        "nominal canonical tensor contract differs",
    )?;
    // Actual source context shared-reference/FnABI remains authenticated and
    // retained by the semantic owner; no physical signature conversion occurs.
    Ok((source_call, operation, expected))
}

fn checked_candidate<'a>(
    call: &'a CheckedBf16NominalCallV1<'a>,
    inventory: &CanonicalKirInventoryV1<'_>,
    effects: &CanonicalKirCallEffectsV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<CheckedNominalCallProjectionV1<'a>> {
    require_effect_inventory(inventory, effects, budget)?;
    budget.charge_work(6)?;
    require(
        call.belongs_to(inventory),
        "nominal call belongs to another inventory",
    )?;
    let helper = call.helper();
    let decision = effects
        .decision(helper.coordinate, budget)
        .map_err(effect_error)?;
    require_nominal_decision(decision, budget)?;
    // The closed helper contains its one actual Matrix and literal CFG/return
    // transport. No extra call, physical effect or ordering site is waived.
    require(
        helper.operations.len() == 1 && helper.calls.is_empty() && helper.effects.is_empty(),
        "nominal helper operation/call/effect roster differs",
    )?;
    let row = inventory
        .operations()
        .get(helper.operations.start)
        .ok_or(Error::Unavailable("nominal helper operation absent"))?;
    require(
        row.coordinate == call.matrix().coordinate
            && std::ptr::eq(row.operation, call.matrix().operation)
            && row.effects.is_empty()
            && row.compiler_ordering().is_empty(),
        "nominal actual Matrix effect/coordinate differs",
    )?;
    let (source_matrix, source_intrinsic, tensor_contract) = source_contract(call, budget)?;
    Ok(CheckedNominalCallProjectionV1 {
        call,
        source_matrix,
        source_intrinsic,
        physical_effects: decision,
        tensor_contract,
    })
}

// Separate construction and callback phases allow precise refunds: a panic
// during report construction has no callback-owned storage, whereas callback
// charges must survive. All payloads are dropped before owned refunds.
fn with_effect_report<'w, R: Copy + 'static>(
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'w>,
    inspect: impl for<'a, 'i, 'g> FnOnce(
        &'a CanonicalKirCallEffectsV1<'i, 'g>,
        &mut Budget<'w>,
    ) -> Result<R>,
) -> Result<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let ledger = budget.work_ledger_identity_v1();
    let header = scratch::<R>()?;
    budget.reserve_storage(header)?;
    let construction_floor = budget.storage();
    let derived = catch_unwind(AssertUnwindSafe(|| {
        CanonicalKirCallEffectsV1::derive(inventory, budget)
    }));
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < construction_floor {
        drop(derived);
        return Err(Resource::Accounting.into());
    }
    let (effects, storage) = match derived {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            if budget.storage() != construction_floor {
                return Err(Resource::Accounting.into());
            }
            budget.release_storage(header)?;
            return Err(effect_error(error));
        }
        Err(payload) => {
            drop(payload);
            // No callback has run and no report escaped.
            budget.release_storage(budget.storage() - construction_floor)?;
            budget.release_storage(header)?;
            return Err(Error::CallbackPanicked);
        }
    };
    if budget.storage() != construction_floor {
        drop(effects);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.reserve_storage(storage.retained_storage()) {
        drop(effects);
        budget.release_storage(header)?;
        return Err(error.into());
    }
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| inspect(&effects, budget)));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    drop(effects);
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
        let _ = result;
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(storage.retained_storage())?;
    budget.release_storage(header)?;
    result
}

/// Closed effect/source-contract observation. N1 performs its exact owner,
/// inventory, source and retained-floor checks BEFORE effect allocation. All
/// additional report/query work uses that same supplied ledger. The borrowed
/// candidate and report cannot escape; no scalar/UnitLocal/normal routing changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_bf16_nominal_call_projection_v1<'w, R: Copy + 'static>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    source_call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: impl for<'a> FnOnce(&CheckedNominalCallProjectionV1<'a>, &mut Budget<'w>) -> Result<R>,
) -> Result<R> {
    owner.with_checked_bf16_nominal_call_v1(
        inventory,
        root,
        caller,
        block,
        source_call,
        budget,
        |call, budget| {
            with_effect_report(inventory, budget, |effects, budget| {
                let candidate = checked_candidate(call, inventory, effects, budget)?;
                inspect(&candidate, budget)
            })
        },
    )
}

#[cfg(test)]
pub(crate) fn inspect_genuine_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    // Runs only after genuine frontend nominal replay/emission (Identity and
    // Swap01 CPU ladder sessions). Fixed test headers/assertions are prepaid on
    // the original live phase account; no replacement analysis ledger exists.
    let ledger = budget.work_ledger_identity_v1();
    let before = budget.storage();
    let headers = 4096;
    budget.reserve_storage(headers)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
        budget.charge_work(64)?;
        let permutation = with_bf16_nominal_call_projection_v1(
            owner,
            inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
            |candidate, budget| {
                budget.charge_work(32)?;
                assert!(candidate.call().belongs_to(inventory));
                assert!(std::ptr::eq(
                    candidate.call().source_call(),
                    source.source_call()
                ));
                assert!(std::ptr::eq(candidate.call().emission().owner(), owner));
                assert_eq!(candidate.call().emission().root(), source.root());
                assert_eq!(candidate.call().emission().helper(), source.helper());
                assert_eq!(
                    candidate.physical_effect_decision(),
                    Decision::CompleteEmpty
                );
                assert_eq!(candidate.required_projection(),
                    RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults);
                assert_eq!(
                    candidate.required_tensor_contract(),
                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                        .with_zero_filled_predicate_inputs()
                );
                let semantic = owner.semantic_ssa().source_semantic();
                let actual_block = &semantic.functions()[source.helper().index() as usize].blocks()
                    [candidate.call().emission().source_matrix_block().index() as usize];
                let SemanticTerminatorKindV1::Call(actual_matrix) =
                    actual_block.terminator().kind()
                else {
                    panic!("genuine source Matrix terminator differs")
                };
                assert!(std::ptr::eq(candidate.source_matrix_call(), actual_matrix));
                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
                    &semantic.callables()[actual_matrix.callee().index() as usize]
                else {
                    panic!("genuine source Matrix callable differs")
                };
                assert!(std::ptr::eq(candidate.source_matrix_intrinsic(), operation));
                assert_eq!(candidate.call().call().operation.results.len(), 4);
                assert_eq!(candidate.call().matrix().operation.results.len(), 4);
                Ok(candidate.required_result_permutation())
            },
        )?;
        assert_eq!(permutation, source.return_permutation());
        assert_eq!(budget.storage(), protected);

        // This candidate deliberately does not authorize the existing generic
        // helper path, nor does N2a remove the materialized ranked refusal.
        let entered = std::cell::Cell::new(false);
        assert!(
            owner
                .with_checked_canonical_calls_v1(inventory, budget, |_, _| {
                    entered.set(true);
                    Ok(())
                })
                .is_err()
        );
        assert!(!entered.get());
        assert!(matches!(
            super::ranked_projection_source_v1::RankedProjectionSourceV1::from_materialized_checked(owner),
            Err(super::ProductionRankedProjectionErrorV1::StructuralValidation(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                    consumer: "BF16 nominal source-ranked projection"
                }
            ))
        ));
        Ok(())
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(headers)?;
    assert_eq!(budget.storage(), before);
    result
}

#[cfg(test)]
#[path = "bf16_nominal_call_projection_v1_tests.rs"]
mod tests;
