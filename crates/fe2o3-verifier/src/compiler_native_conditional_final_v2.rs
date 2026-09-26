//! Conditional source-through-F content only; no final/nominal/native authority.
#![allow(
    dead_code,
    reason = "private content-only composition awaiting outer integration"
)]
#![allow(
    clippy::result_large_err,
    clippy::large_enum_variant,
    reason = "typed terminal errors without an uncharged allocation"
)]
use super::{
    Budget, E as SourceError, NativeConditionalRootPolicyV2, NativeConditionalSourcePacketInputV2,
    NativeConditionalSourceStorageV2, ReplayedNativeConditionalSourceV2, ReplayedNativeSourceV1,
    ReplayedRoot, Resource, reconstruct, root, with_decoded_native_conditional_source_packet_v2,
};
use crate::conditional_contract_request_v2::check_conditional_contract_content_v2;
use crate::conditional_ranked_formulas_v1::import_and_check_conditional_ranked_formula_v2;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::{
    check_native_v12_text_descriptor_relation_v5,
    check_production_target_coordinate_preservation_v1,
};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_TABLE_VIEW_STORAGE_V5, DeviceDescriptorTableV5, KernelId,
};
use fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 as Catalog;
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits,
    CheckedCanonicalRefinedForwardingHistoryV1 as History,
    DecodedRefinedForwardingHistoryV1 as DecodedHistory, RefinedForwardingHistoryRoleV1 as Role,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use std::mem::size_of;

#[path = "compiler_native_conditional_final_v2/account.rs"]
mod account;
#[path = "compiler_native_conditional_final_v2/error.rs"]
mod error;
#[path = "compiler_native_conditional_final_v2/roster.rs"]
mod roster;
pub(crate) use error::NativeConditionalFinalErrorV2;
use error::{Cause, NativeConditionalFinalErrorV2 as Error};

/// All backing is caller-owned and prepaid on the original consumer ledger.
/// Limits and profile are independently accepted, never copied from transport.
/// Neither these inputs nor their content agreement authenticates source origin,
/// original nominal/context custody, registration or launch restrictions.
pub(crate) struct NativeConditionalFinalInputsV2<'a, 'frame, 'wire> {
    pub decoded_history: &'a DecodedHistory<'frame, 'wire>,
    pub expected_limits: Limits,
    pub final_catalog: &'a Catalog,
    pub published_output_bytes: &'a [u8],
    pub profile: Profile,
    pub descriptors: &'a DeviceDescriptorTableV5<'wire>,
    pub final_llvm: &'a str,
}
type Inputs<'a, 'frame, 'wire> = NativeConditionalFinalInputsV2<'a, 'frame, 'wire>;
type Output = (
    ReplayedNativeConditionalSourceV2,
    NativeConditionalSourceStorageV2,
);
const HEADER: usize = size_of::<Inputs<'static, 'static, 'static>>()
    + 4 * size_of::<account::Account>()
    + size_of::<Result<Output, Error>>();

/// One checked history, one N/B relation and one B1/lower/V2 import per root.
/// Returns only existing C1 source content custody with UNRESERVED storage.
/// The caller still owns decoded actual F; no F/nominal/launch owner is minted.
/// All errors (including overriding postchecks) are terminal for refund purposes.
pub(crate) fn validate_native_conditional_source_through_f_v2(
    packet_bytes: &[u8],
    accepted: &[NativeConditionalRootPolicyV2<'_>],
    inputs: Inputs<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Output, Error> {
    account::transfer(budget, |budget| {
        require_backing(packet_bytes, &inputs, budget)?;
        account::temporary(budget, HEADER, |budget| {
            require_limits(
                inputs.decoded_history.frame().limits(),
                inputs.expected_limits,
                budget,
            )?;
            let history = inputs
                .decoded_history
                .check_semantics(budget)
                .map_err(|error| Error(Cause::History(error)))?;
            let storage = history.storage().retained_storage();
            account::temporary(budget, storage, move |budget| {
                let result = with_decoded_native_conditional_source_packet_v2(
                    packet_bytes,
                    budget,
                    |packet, budget| {
                        reconstruct::reconstruct_using(
                            packet,
                            accepted,
                            budget,
                            |source, packet, roots, retained, budget| {
                                check_source(
                                    source, packet, accepted, roots, retained, &inputs, &history,
                                    budget,
                                )
                            },
                        )
                    },
                )
                .map_err(|error| Error(Cause::Packet(error)));
                drop(history);
                result?
            })
        })
    })
}

fn require_limits(actual: Limits, expected: Limits, budget: &mut Budget<'_>) -> Result<(), Error> {
    budget.charge_work(size_of::<Limits>() + 1)?;
    if actual != expected {
        return Err(Error::mismatch(
            "independently accepted complete history limits",
        ));
    }
    Ok(())
}

fn require_backing(
    bytes: &[u8],
    inputs: &Inputs<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(10)?;
    // Visible minimum only. Caller retains actual capacities/receipts, including
    // separate published bytes when they do not borrow the already-paid F/wire.
    let lengths = [
        bytes.len(),
        inputs.decoded_history.storage().retained_storage(),
        inputs.decoded_history.frame().storage().retained_storage(),
        inputs.decoded_history.frame().canonical_bytes().len(),
        DESCRIPTOR_TABLE_VIEW_STORAGE_V5,
        inputs.descriptors.canonical_bytes().len(),
        size_of::<Catalog>(),
        inputs.final_catalog.canonical_bytes().len(),
        inputs.final_llvm.len(),
    ];
    let minimum = lengths
        .into_iter()
        .try_fold(0usize, |sum, n| sum.checked_add(n))
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

fn check_source(
    source: &ReplayedNativeSourceV1,
    packet: &NativeConditionalSourcePacketInputV2<'_>,
    accepted: &[NativeConditionalRootPolicyV2<'_>],
    roots: &mut Vec<ReplayedRoot>,
    retained: &mut usize,
    inputs: &Inputs<'_, '_, '_>,
    history: &History<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let (coordinates, storage) = check_production_target_coordinate_preservation_v1(
        source.source().executable(),
        inputs.decoded_history.graph(Role::B),
        inputs.profile,
        budget,
    )
    .map_err(|error| Error(Cause::Coordinates(error)))?;
    account::temporary(budget, storage.retained_storage(), move |budget| {
        roster::check(source, packet, history.output(), inputs.descriptors, budget)?;
        reconstruct::reconstruct_roots(
            source,
            packet,
            accepted,
            roots,
            retained,
            budget,
            |source, row, policy, body, budget| {
                root::reconstruct_root_using(
                    source,
                    row,
                    policy,
                    budget,
                    |request, decoded, budget| {
                        import_and_check_conditional_ranked_formula_v2(
                            request,
                            decoded,
                            row.formula_receipt,
                            policy.formula,
                            budget,
                            |execution, budget| {
                                request
                                    .check_replayed_refined_forwarding_output_v1(
                                        &coordinates,
                                        history,
                                        inputs.expected_limits,
                                        budget,
                                    )
                                    .map_err(|error| Error(Cause::ConditionalFinal(error)))?;
                                check_conditional_contract_content_v2(
                                    request,
                                    execution,
                                    (SemanticFunctionIdV1::from_index(row.semantic_root), body),
                                    KernelId::from_bytes(row.launch.kernel_binding()),
                                    inputs.descriptors,
                                    None,
                                    budget,
                                )
                                .map_err(|error| Error(Cause::Contract(error)))
                            },
                        )
                    },
                    |imported| imported.map_err(|error| Error(Cause::Import(error)))?,
                )
            },
        )?;
        let relation = check_native_v12_text_descriptor_relation_v5(
            history.output(),
            inputs.final_catalog,
            inputs.published_output_bytes,
            inputs.profile,
            inputs.descriptors,
            inputs.final_llvm,
            budget,
        )
        .map_err(|error| Error(Cause::Text(error)))?;
        account::temporary(budget, relation.storage().retained_storage(), move |_| {
            drop(relation);
            Ok(())
        })?;
        drop(coordinates);
        Ok(())
    })
}

#[cfg(test)]
#[path = "compiler_native_conditional_final_v2/tests.rs"]
mod tests;
