//! Same-import conditional contract comparison, not collector/native authority.
//!
//! Success compares supported content only. Original source/nominal collector
//! custody remains external; no helper here can mint a receipt or authority.
#![allow(
    dead_code,
    reason = "Private prerequisite for the separate C1 same-import integration."
)]
#![allow(
    clippy::result_large_err,
    clippy::large_enum_variant,
    reason = "Preserve typed errors without an uncharged box."
)]

use crate::ProductionConditionalFormulaExecutionV2 as Execution;
use fe2o3_kernel_descriptor::{
    ConditionalInvocationContractV2 as Contract, ConditionalInvocationWireErrorV1,
    DescriptorWireErrorV3, DescriptorWireErrorV5, DeviceDescriptorTableV5 as Table,
    KernelDescriptorRefV5 as Kernel, KernelId,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedContextRootV29, ProductionSemanticKirErrorV1,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use fe2o3_pliron::{ProductionConditionalAggregateErrorV1, ProductionSourceArgumentErrorV1};
use std::mem::size_of;

mod account;
mod arguments;
mod nominal;
mod rows;
mod source;

#[derive(Debug)]
pub(crate) enum ConditionalContractRequestErrorV2 {
    Resource(Resource),
    Descriptor(DescriptorWireErrorV5<Resource>),
    Contract(ConditionalInvocationWireErrorV1<Resource>),
    Source(ProductionSemanticKirErrorV1),
    Argument(ProductionSourceArgumentErrorV1),
    Aggregate(ProductionConditionalAggregateErrorV1),
    Mismatch(&'static str),
    MissingContextCollectorCustody,
}
type E = ConditionalContractRequestErrorV2;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<DescriptorWireErrorV5<Resource>> for E {
    fn from(value: DescriptorWireErrorV5<Resource>) -> Self {
        Self::Descriptor(value)
    }
}
impl From<DescriptorWireErrorV3<Resource>> for E {
    fn from(value: DescriptorWireErrorV3<Resource>) -> Self {
        Self::Descriptor(DescriptorWireErrorV5::Nominal(value))
    }
}
impl From<ConditionalInvocationWireErrorV1<Resource>> for E {
    fn from(value: ConditionalInvocationWireErrorV1<Resource>) -> Self {
        Self::Contract(value)
    }
}
impl std::fmt::Display for E {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "conditional contract/request V2: {self:?}")
    }
}
// Terminal leaf: callers must not classify a nested opaque failure as refundable.
impl std::error::Error for E {}

fn sum(parts: &[usize]) -> Result<usize, Resource> {
    parts
        .iter()
        .try_fold(0usize, |n, p| n.checked_add(*p).ok_or(Resource::Arithmetic))
}
fn product(a: usize, b: usize) -> Result<usize, Resource> {
    a.checked_mul(b).ok_or(Resource::Arithmetic)
}
fn require(ok: bool, field: &'static str) -> R<()> {
    if ok { Ok(()) } else { Err(E::Mismatch(field)) }
}

// Query constants pay their own nested decoder temporaries. The additional
// extents cover our simultaneously live bindings, maps and comparison rows.
const SCRATCH: usize = fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V5
    + size_of::<Kernel<'static, 'static>>()
    + size_of::<Contract<'static>>()
    + size_of::<fe2o3_pliron::ProductionSourceArgumentRelationV1<'static, 'static>>()
    + size_of::<fe2o3_pliron::ProductionSourceArgumentBindingV1<'static, 'static>>()
    + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
    + 2 * size_of::<usize>()
    + rows::STORAGE
    + nominal::STORAGE;

/// Call only inside the same strict-import callback as this genuine request.
/// All backing and existing source/proof owners remain on the caller's ledger.
/// No contract, execution, budget, graph or proof is reconstructed here.
///
/// V5 omits the original collector's semantic type identity. Success is only
/// content agreement, never that original nominal/source provenance. A checked
/// context-root view cannot authorize unsupported logical-helper elision.
pub(crate) fn check_conditional_contract_content_v2(
    request: &Request<'_>,
    execution: &Execution,
    source: (SemanticFunctionIdV1, SemanticFunctionIdV1),
    expected_kernel: KernelId,
    table: &Table<'_>,
    context: Option<&ProductionCheckedContextRootV29<'_>>,
    budget: &mut Budget<'_>,
) -> R<()> {
    let inherited = sum(&[
        request.source().retained_analysis_storage_v1(),
        table.canonical_bytes().len(),
        fe2o3_kernel_descriptor::DESCRIPTOR_TABLE_VIEW_STORAGE_V5,
    ])?;
    if budget.storage() < inherited {
        return Err(Resource::Accounting.into());
    }
    account::scope(budget, SCRATCH, |budget| {
        let input = request.pliron_input();
        input
            .require_current_graph_v1(budget)
            .map_err(E::Aggregate)?;
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let slot = budget as *const Budget<'_> as usize;
        let result = (|| {
            let descriptor = table.find_kernel(expected_kernel, &mut |w| budget.charge_work(w))?;
            let contract = descriptor.conditional_contract(&mut |w| budget.charge_work(w))?;
            source::check(request, source, expected_kernel, table, &descriptor, budget)?;
            rows::check(request, execution, expected_kernel, &contract, budget)?;
            if let Some(context) = context {
                budget.charge_work(4)?;
                require(
                    std::ptr::eq(context.semantic_ssa(), request.source().semantic_ssa())
                        && context.root_id() == source.0,
                    "context source/root",
                )?;
                if source.1 != source.0 {
                    require(context.helper_id() == source.1, "context helper")?;
                    return Err(E::MissingContextCollectorCustody);
                }
            }
            arguments::check(request, source, table, &descriptor, &contract, budget)?;
            Ok(())
        })();
        if ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
            || slot != budget as *const Budget<'_> as usize
        {
            return Err(Resource::Accounting.into());
        }
        // Check current graph even on a content/custody refusal. The account
        // scope retains terminal charges if an inherited checker loses a floor.
        input
            .require_current_graph_v1(budget)
            .map_err(E::Aggregate)?;
        result
    })
}

#[cfg(test)]
mod tests;
