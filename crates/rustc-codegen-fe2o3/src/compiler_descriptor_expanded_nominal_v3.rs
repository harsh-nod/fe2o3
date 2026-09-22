//! Nominal V3 evidence for the actual expanded final graph, never relabeled U.
#![allow(
    clippy::drop_non_drop,
    reason = "End borrowed descriptor views before refunds."
)]
#![allow(
    clippy::result_large_err,
    reason = "Preserve typed child refusals inline."
)]
use super::*;
use crate::compiler_descriptor::nominal_v3::{self as codec, NominalDescriptorErrorV3};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    decode_device_descriptor_table_v3,
};
use fe2o3_lower_mir_kernel::{
    ProductionExpandedPolicyErrorV1, ProductionExpandedPrefixV1,
    ProductionOwnedExpandedContinuationV1 as Expanded,
};
use std::any::Any;

pub(crate) const FINAL_DOMAIN: &[u8] = b"FE2O3/EXPANDED-PRODUCTION-FINAL-EXECUTABLE-ABI/V3\0";

#[derive(Debug)]
pub(crate) enum ExpandedNominalErrorV3 {
    Resource(Resource),
    Nominal(NominalDescriptorErrorV3),
    Expanded(ProductionExpandedPolicyErrorV1),
    Mismatch(&'static str),
    Panicked,
}
pub(crate) type E = ExpandedNominalErrorV3;
pub(crate) type R<T> = std::result::Result<T, E>;
impl From<Resource> for E {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expanded final nominal evidence: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Nominal(e) => Some(e),
            Self::Expanded(e) => Some(e),
            Self::Mismatch(_) | Self::Panicked => None,
        }
    }
}
pub(crate) fn pipeline(e: crate::production_pipeline::ProductionPipelineError) -> E {
    E::Nominal(NominalDescriptorErrorV3::Pipeline(e))
}

pub(crate) const SCOPE: usize = 2 * size_of::<usize>()
    + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<[Option<Box<dyn Any + Send>>; 2]>();

/// One paid scope shared by the expanded owner and its final transport. Numeric
/// floors are necessary accounting, never source or execution authentication.
pub(crate) fn scoped<'w, T, X: From<Resource>>(
    budget: &mut Budget<'w>,
    panicked: impl FnOnce() -> X,
    run: impl FnOnce(&mut Budget<'w>) -> std::result::Result<T, X>,
) -> std::result::Result<T, X> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let paid = SCOPE
        .checked_add(size_of::<std::result::Result<T, X>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(panicked())
        }
    };
    let same =
        ledger == budget.work_ledger_identity_v1() && slot == budget as *const Budget<'w> as usize;
    if !same
        || floor
            .checked_add(paid)
            .is_none_or(|min| budget.storage() < min)
    {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if same && budget.storage() >= floor {
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
        }
    }
    drop(payloads);
    result
}

fn original_view(owner: &Expanded) -> R<CheckedDescriptorViewV1<'_>> {
    let original = match owner.prefix() {
        ProductionExpandedPrefixV1::Direct(value) => UnrolledOwnerV1::Direct(value),
        ProductionExpandedPrefixV1::Erased(value) => UnrolledOwnerV1::Erased(value),
    };
    let mut view = original
        .view()
        .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Unroll(e)))?;
    // Original source, pre-bind N/E and B stay inherited. Final evidence comes
    // only from the complete independently replayed expanded owner.
    view.output = owner.output();
    view.kernels = owner.kernels();
    Ok(view)
}

const VIEW: usize = size_of::<UnrolledOwnerV1<'static>>()
    + size_of::<CheckedDescriptorViewV1<'static>>()
    + size_of::<fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1<'static>>()
    + size_of::<ProductionGeometryV1>()
    + size_of::<fe2o3_artifacts::RustNominalScalarEvidenceV3>();

fn check_source_abi(owner: &Expanded, wire: &[u8], budget: &mut Budget<'_>) -> R<()> {
    budget.reserve_storage(
        DESCRIPTOR_TABLE_VIEW_STORAGE_V3
            .checked_add(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let table = decode_device_descriptor_table_v3(wire, &mut |n| budget.charge_work(n))
        .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Wire(e)))?;
    let (agreement, storage) =
        fe2o3_verifier::check_nominal_source_abi_v3(owner.source_anchor(), &table, budget)
            .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Agreement(e)))?;
    budget.reserve_storage(storage.retained_storage())?;
    agreement
        .verify_equivalence(budget)
        .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Agreement(e)))?;
    if agreement.grants_artifact_or_launch_authority() {
        return Err(E::Mismatch("inert expanded nominal agreement"));
    }
    drop(agreement);
    drop(table);
    Ok(())
}

/// Fresh final bytes from sealed genuine source/history custody. The returned
/// Vec's actual capacity and header must be paid before controlled use.
pub(crate) fn produce(
    owner: &Expanded,
    roots: &[TypedDescriptorRootV1],
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    produce_for_profile(
        owner,
        roots,
        target.profile(),
        target.rustc_layout().default_pointer_width_bits(),
        budget,
    )
}
fn produce_for_profile(
    owner: &Expanded,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    if budget.storage()
        < owner
            .retained_input_storage_floor_v1()
            .map_err(E::Expanded)?
    {
        return Err(Resource::Accounting.into());
    }
    scoped(
        budget,
        || E::Panicked,
        |budget| {
            budget.reserve_storage(VIEW)?;
            budget.charge_work(2)?;
            owner.verify_equivalence(budget).map_err(E::Expanded)?;
            let view = original_view(owner)?;
            let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                view.neutral,
                view.bound,
                profile,
                budget,
            )
            .map_err(|e| {
                E::Nominal(NominalDescriptorErrorV3::Descriptor(
                    CompilerDescriptorError::CheckedOutputTarget(e),
                ))
            })?;
            nominal_v3::validate_view(roots, &view, profile, budget).map_err(E::Nominal)?;
            let wire = codec::encode_subject(
                roots,
                view.semantic,
                view.output.module(),
                view.output.canonical().canonical_bytes(),
                view.kernels.len(),
                profile,
                pointer_width,
                FINAL_DOMAIN,
                "inert-expanded-production-final-native-v3",
                budget,
            )
            .map_err(E::Nominal)?;
            budget.reserve_storage(
                wire.capacity()
                    .checked_add(size_of::<Vec<u8>>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            check_source_abi(owner, &wire, budget)?;
            Ok(wire)
        },
    )
}

#[cfg(test)]
#[path = "compiler_descriptor_expanded_nominal_v3_tests.rs"]
pub(crate) mod tests;

#[path = "compiler_descriptor_expanded_decoded_v3.rs"]
pub(crate) mod decoded;
