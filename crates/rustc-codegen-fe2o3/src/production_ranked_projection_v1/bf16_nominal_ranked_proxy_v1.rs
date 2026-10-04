//! Borrowed N3 recipe-row candidate, not ranked/final/normal admission.
//! C3 alone does not prove C2 Final. The genuine hook selects the actual Final
//! visitor; a future positive continuation still needs a sealed postflight.
//! No candidate digest, copied operation, or pass enum is an authority token.
use super::bf16_nominal_layout_return_v1::NominalTensorOccurrenceV1;
use super::{
    DigestV1, ProductionCooperativeTensorBindingV1, ProductionRankedOperationV1,
    TensorConvergenceAttr, tensor_capability_root_v1, tensor_operand_root_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, TensorLayoutContractV1,
};
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as Error;
use sha2::Sha256;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, Error>;
const SCOPE_WORK: usize = 16;
const JOIN_WORK: usize = 64;
const BINDING_WORK: usize = 2048;
const SOURCE_ARGUMENT_COUNT: u16 = 4;

/// A compiler-built analysis row borrowing its actual C3 association. Neither
/// Clone nor Copy; the public-to-projector API only lends this lexical value.
/// An owned operation copied by a consumer remains an UNVALIDATED recipe row.
pub(super) struct NominalRankedTensorProxyV1<'occ, 'source> {
    occurrence: &'occ NominalTensorOccurrenceV1<'source>,
    operation: ProductionRankedOperationV1,
}
impl<'occ, 'source> NominalRankedTensorProxyV1<'occ, 'source> {
    pub(super) const fn occurrence(&self) -> &'occ NominalTensorOccurrenceV1<'source> {
        self.occurrence
    }
    pub(super) const fn operation(&self) -> &ProductionRankedOperationV1 {
        &self.operation
    }
}

// Private pure mapping accepts inert data and grants no source/final authority.
// Production reaches it only after joining the live occurrence below.
fn operation_from_parts(
    contract: TensorLayoutContractV1,
    active_lanes: u32,
    roots: [DigestV1; 6],
) -> Result<ProductionRankedOperationV1> {
    if contract
        != TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
            .with_zero_filled_predicate_inputs()
        || active_lanes != 64
    {
        return Err(Error::Unavailable(
            "nominal ranked proxy tensor profile differs",
        ));
    }
    let [context, lane, lhs, rhs, accumulator, qualified_result] = roots;
    let binding = ProductionCooperativeTensorBindingV1::new(
        context,
        lane,
        lhs,
        rhs,
        accumulator,
        qualified_result,
        SOURCE_ARGUMENT_COUNT,
    )
    .ok_or(Error::Unavailable(
        "nominal ranked proxy capability roots differ",
    ))?;
    Ok(ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence: TensorConvergenceAttr::UniformSubgroup,
        active_lanes,
        binding: Some(binding),
    })
}

fn construct<'occ, 'source>(
    occurrence: &'occ NominalTensorOccurrenceV1<'source>,
    budget: &mut Budget<'_>,
) -> Result<NominalRankedTensorProxyV1<'occ, 'source>> {
    budget.charge_work(JOIN_WORK)?;
    let authenticated = occurrence.authenticated();
    let candidate = authenticated.candidate();
    let checked = candidate.call();
    let site = authenticated.site();
    let emission = checked.emission();
    if !std::ptr::eq(site.owner(), emission.owner())
        || !std::ptr::eq(site.call(), checked.source_call())
        || !site.inventory().belongs_to(site.owner().executable())
        || !checked.belongs_to(site.inventory())
        || occurrence.call_coordinate() != checked.call().coordinate
        || occurrence.matrix_coordinate() != checked.matrix().coordinate
        || occurrence.call_coordinate().block.function
            == occurrence.matrix_coordinate().block.function
        || occurrence.return_coordinate().function != occurrence.matrix_coordinate().block.function
        || occurrence.permutation() != emission.return_permutation()
        || occurrence.permutation() != candidate.required_result_permutation()
    {
        return Err(Error::Unavailable(
            "nominal ranked proxy live occurrence differs",
        ));
    }
    // The qualified Return rows stay borrowed through occurrence. Do not replace
    // their N1 edge-argument ancestry with equality of unrelated numeric IDs.
    let tensor = occurrence.input_tensor();
    if tensor.contract != candidate.required_tensor_contract() {
        return Err(Error::Unavailable(
            "nominal ranked proxy caller tensor differs",
        ));
    }
    budget.charge_work(BINDING_WORK)?;
    let operation = operation_from_parts(
        tensor.contract,
        u32::from(tensor.contract.subgroup_width),
        [
            tensor_capability_root_v1(1, &[tensor.context_root]),
            tensor_capability_root_v1(2, &[tensor.accumulator.lane_root]),
            tensor_operand_root_v1(tensor.lhs),
            tensor_operand_root_v1(tensor.rhs),
            tensor_capability_root_v1(6, &[tensor.accumulator.flow_root]),
            // This is the C3 qualified helper/Call/Return association identity,
            // NOT the ordinary array destination's accumulator origin.
            occurrence.binding_digest(),
        ],
    )?;
    Ok(NominalRankedTensorProxyV1 {
        occurrence,
        operation,
    })
}

fn scope_storage<R>(callback_bytes: usize) -> Result<usize> {
    size_of::<NominalRankedTensorProxyV1<'static, 'static>>()
        .checked_add(size_of::<ProductionRankedOperationV1>())
        .and_then(|n| n.checked_add(size_of::<ProductionCooperativeTensorBindingV1>()))
        .and_then(|n| n.checked_add(size_of::<super::AuthenticatedTensorInstructionV1>()))
        .and_then(|n| n.checked_add(size_of::<Sha256>()))
        .and_then(|n| n.checked_add(size_of::<[DigestV1; 6]>()))
        .and_then(|n| n.checked_add(4096))
        .and_then(|n| callback_bytes.checked_mul(2).and_then(|f| n.checked_add(f)))
        .and_then(|n| {
            size_of::<Result<R>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or(Resource::Arithmetic.into())
}

// The original caller lends its actual Budget. There is no owned-account
// constructor, cloned meter, callback receipt, or detached identity input.
fn with_owned_scope<'w, R: Copy + 'static, F>(budget: &mut Budget<'w>, body: F) -> Result<R>
where
    F: FnOnce(&mut Budget<'w>) -> Result<R>,
{
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let slot = budget as *const Budget<'w>;
    let ledger = budget.work_ledger_identity_v1();
    let reserved = scope_storage::<R>(size_of::<F>())?;
    budget.reserve_storage(reserved)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(SCOPE_WORK)?;
        body(budget)
    }));
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
    if budget as *const Budget<'w> != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() < protected
    {
        return Err(Resource::Accounting.into());
    }
    // Candidate, callback captures, and panic payload have dropped first.
    // Only this reservation is released; callback surplus/work/denial survive.
    budget.release_storage(reserved)?;
    result
}

/// Lends a recipe CANDIDATE while retaining the live C3 occurrence. The caller
/// must pass the same original budget from its real source/facts/C2/C3 scope.
/// This API cannot establish Final or allow access/normal compilation. Positive
/// continuation requires a driver-sealed postflight, a complete root recipe and
/// qualified source/ranked correspondence before any retained attachment.
pub(super) fn with_nominal_ranked_tensor_proxy_v1<'occ, 'source, 'w, R, F>(
    occurrence: &'occ NominalTensorOccurrenceV1<'source>,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'scope> FnOnce(
        &'scope NominalRankedTensorProxyV1<'occ, 'source>,
        &mut Budget<'w>,
    ) -> Result<R>,
{
    with_owned_scope(budget, move |budget| {
        let proxy = construct(occurrence, budget)?;
        let result = inspect(&proxy, budget);
        drop(proxy);
        result
    })
}

#[cfg(test)]
#[path = "bf16_nominal_ranked_proxy_v1_tests.rs"]
mod tests;
