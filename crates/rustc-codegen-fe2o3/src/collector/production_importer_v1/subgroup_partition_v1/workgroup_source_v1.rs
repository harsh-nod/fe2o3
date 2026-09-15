//! Source facts for transporting the same Workgroup through subgroup and epoch borrows.
//! These records do not issue authority. Lowering must bind argument zero to its
//! existing Workgroup SSA issuer; nominal identities alone are insufficient.

use super::*;
use rustc_middle::mir::{
    Body, BorrowKind, Local, MirPhase, ProjectionElem, RETURN_PLACE, RuntimePhase, Rvalue,
    START_BLOCK, StatementKind, TerminatorKind,
};
use rustc_middle::ty::{EarlyBinder, FnSig};

mod canonical_transport_v1;
mod context_entry_source_v1;
mod context_entry_transport_v1;
pub(in crate::collector::production_importer_v1) use context_entry_transport_v1::attach_context_entry_transfers_v1;
pub(in crate::collector::production_importer_v1) use canonical_transport_v1::{
    attach_epoch_projection_v1, epoch_source_for_function_v1,
};

const EPOCH_FIELD: usize = 2;
const WORKGROUP_FIELDS: usize = 4;
const EPOCH_METHOD: &str = "fe2o3_device::execution::WorkgroupCapability::epoch";

#[allow(clippy::too_many_arguments)]
pub(in crate::collector::production_importer_v1) fn subgroup_workgroup_reference_operation_v1<
    'tcx,
>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let source = subgroup_workgroup_reference_source_v1(tcx, instance, abi, types, root, contexts)?;
    canonical_transport_v1::subgroup_operation_v1(source, abi, source_identity)
}

/// Both identities are retained: a source reference is not an owned authority.
#[derive(Debug)]
pub(in crate::collector::production_importer_v1) struct WorkgroupReferenceSourceV1 {
    reference: SemanticTypeIdV1,
    workgroup: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    source_identity: SemanticFunctionIdentityV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
}

impl WorkgroupReferenceSourceV1 {
    pub(in crate::collector::production_importer_v1) fn reference(&self) -> SemanticTypeIdV1 {
        self.reference
    }

    pub(in crate::collector::production_importer_v1) fn workgroup(&self) -> SemanticTypeIdV1 {
        self.workgroup
    }

    pub(in crate::collector::production_importer_v1) fn output(&self) -> SemanticTypeIdV1 {
        self.output
    }

    pub(in crate::collector::production_importer_v1) fn source_identity(
        &self,
    ) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }

    pub(in crate::collector::production_importer_v1) fn provenance(
        &self,
    ) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }

    pub(in crate::collector::production_importer_v1) fn brand(&self) -> SemanticTypeIdentityV1 {
        self.brand
    }

    pub(in crate::collector::production_importer_v1) fn epoch(&self) -> SemanticTypeIdentityV1 {
        self.epoch
    }

    pub(in crate::collector::production_importer_v1) fn receiver_argument(&self) -> usize {
        0
    }
}

#[derive(Debug)]
pub(in crate::collector::production_importer_v1) struct WorkgroupEpochProjectionSourceV1 {
    receiver: WorkgroupReferenceSourceV1,
    epoch_type: SemanticTypeIdV1,
}

impl WorkgroupEpochProjectionSourceV1 {
    pub(in crate::collector::production_importer_v1) fn receiver(
        &self,
    ) -> &WorkgroupReferenceSourceV1 {
        &self.receiver
    }

    pub(in crate::collector::production_importer_v1) fn epoch_type(&self) -> SemanticTypeIdV1 {
        self.epoch_type
    }

    /// Source field ordinal, never a byte offset or an independently issued token.
    pub(in crate::collector::production_importer_v1) fn source_field(&self) -> usize {
        EPOCH_FIELD
    }
}

/// Validates the Wave64 source form without rewriting the legacy SubgroupDerive schema.
pub(in crate::collector::production_importer_v1) fn subgroup_workgroup_reference_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<WorkgroupReferenceSourceV1, ProductionSemanticImportErrorV1> {
    let signature = subgroup_signature_v1(tcx, instance)?;
    if !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
        || !exact_subgroup_source_layout_v1(types, abi.source_output_type())
    {
        return Err(rejected("subgroup workgroup physical lane ABI"));
    }
    workgroup_reference_source_v1(tcx, instance, signature, abi, types, root, contexts)
}

fn subgroup_signature_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if trusted_device_items::classify(tcx, instance.def_id())
        != Some(TrustedDeviceItem::ExecutionSubgroupCurrent)
    {
        return Err(rejected("subgroup workgroup source provider"));
    }
    let signature = workgroup_signature_v1(tcx, instance)?;
    let receiver_ty = rust_shared_reference_v1(signature.inputs()[0])
        .ok_or_else(|| rejected("subgroup workgroup shared reference"))?;
    let receiver = rust_workgroup_capability_v1(tcx, receiver_ty)
        .ok_or_else(|| rejected("subgroup workgroup exact receiver"))?;
    let subgroup = rust_subgroup_v1(tcx, signature.output())
        .ok_or_else(|| rejected("subgroup workgroup exact output"))?;
    let arguments = instance.args.types().collect::<Vec<_>>();
    if subgroup.width != 64
        || rust_execution_generic_width_v1(tcx, instance) != Some(64)
        || !rust_same_kernel_brand_v1(receiver.kernel_brand, subgroup.kernel_brand)
        || receiver.epoch != subgroup.epoch
        || !matches!(arguments.as_slice(), [brand, epoch, width]
            if *brand == receiver.kernel_brand.ty && *epoch == receiver.epoch
                && rust_subgroup_width_v1(tcx, *width) == Some(64))
    {
        return Err(rejected("subgroup workgroup width, brand, or epoch"));
    }
    Ok(signature)
}

/// Layout check only, consumed after exact provider and nominal type validation.
/// Subgroup contains a live u32 lane; only the partition view is zero-sized.
pub(in crate::collector::production_importer_v1) fn exact_subgroup_source_layout_v1(
    types: &[SemanticTypeDeclV1],
    subgroup: SemanticTypeIdV1,
) -> bool {
    let aggregate = |ty: SemanticTypeIdV1| -> Option<&[SemanticTypeIdV1]> {
        let declaration = types.get(ty.index() as usize)?;
        let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
            return None;
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
            return None;
        };
        (declaration.layout().size_bytes() == Some(4)
            && declaration.layout().alignment_bytes() == 4
            && !declaration.layout().is_uninhabited()
            && layout.field_offsets().len() == fields.fields().len()
            && layout.field_offsets().first() == Some(&0)
            && layout
                .field_offsets()
                .iter()
                .skip(1)
                .all(|offset| *offset == 4))
        .then_some(fields.fields())
    };
    let Some([lane, workgroup_brand, not_send_sync]) = aggregate(subgroup) else {
        return false;
    };
    let Some([rank, width, subgroup_brand, lane_not_send_sync]) = aggregate(*lane) else {
        return false;
    };
    [
        workgroup_brand,
        not_send_sync,
        width,
        subgroup_brand,
        lane_not_send_sync,
    ]
    .into_iter()
    .all(|ty| semantic_exact_inhabited_aggregate_zst_v1(types, *ty))
        && types.get(rank.index() as usize).is_some_and(|declaration| {
            declaration.shape()
                == &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                })
                && declaration.layout().size_bytes() == Some(4)
                && declaration.layout().alignment_bytes() == 4
                && !declaration.layout().is_uninhabited()
        })
}

pub(in crate::collector::production_importer_v1) fn workgroup_epoch_projection_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<WorkgroupEpochProjectionSourceV1, ProductionSemanticImportErrorV1> {
    let signature = epoch_signature_v1(tcx, instance)?;
    if !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_)) {
        return Err(rejected("workgroup epoch shared result ABI"));
    }
    let epoch_ty = rust_shared_reference_v1(signature.output())
        .ok_or_else(|| rejected("workgroup epoch shared output"))?;
    let receiver =
        workgroup_reference_source_v1(tcx, instance, signature, abi, types, root, contexts)?;
    let epoch_type = semantic_type_for_rust_v1(tcx, types, epoch_ty)?;
    if exact_shared_pointee_v1(types, receiver.output())? != epoch_type
        || !semantic_exact_inhabited_aggregate_zst_v1(types, epoch_type)
        || !matches!(types[receiver.workgroup().index() as usize].shape(),
            SemanticTypeShapeV1::Aggregate(fields)
                if fields.fields().len() == WORKGROUP_FIELDS
                    && fields.fields()[EPOCH_FIELD] == epoch_type)
    {
        return Err(rejected("workgroup epoch semantic field identity"));
    }
    Ok(WorkgroupEpochProjectionSourceV1 {
        receiver,
        epoch_type,
    })
}

fn epoch_signature_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !epoch_provider_v1(tcx, instance) {
        return Err(rejected("workgroup epoch exact reviewed provider"));
    }
    let signature = workgroup_signature_v1(tcx, instance)?;
    let receiver_ty = rust_shared_reference_v1(signature.inputs()[0])
        .ok_or_else(|| rejected("workgroup epoch shared receiver"))?;
    let receiver = rust_workgroup_capability_v1(tcx, receiver_ty)
        .ok_or_else(|| rejected("workgroup epoch exact workgroup"))?;
    let epoch_ty = rust_shared_reference_v1(signature.output())
        .ok_or_else(|| rejected("workgroup epoch shared output"))?;
    let epoch = rust_workgroup_epoch_v1(tcx, epoch_ty)
        .ok_or_else(|| rejected("workgroup epoch exact output"))?;
    if instance.args.types().collect::<Vec<_>>() != [receiver.kernel_brand.ty, receiver.epoch]
        || !rust_same_kernel_brand_v1(receiver.kernel_brand, epoch.kernel_brand)
        || receiver.epoch != epoch.epoch
        || !workgroup_epoch_field_v1(tcx, receiver_ty, epoch_ty)
        || !reviewed_epoch_body_v1(tcx, instance, tcx.instance_mir(instance.def), signature)
    {
        return Err(rejected(
            "workgroup epoch substituted projection, brand, epoch, or ABI",
        ));
    }
    Ok(signature)
}

fn epoch_provider_v1(tcx: TyCtxt<'_>, instance: Instance<'_>) -> bool {
    matches!(instance.def, InstanceKind::Item(_))
        && tcx.is_mir_available(instance.def_id())
        && trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            instance.def_id(),
            EPOCH_METHOD,
        )
        && trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            instance.def_id(),
        )
        .unwrap_or(false)
}

fn workgroup_signature_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<FnSig<'tcx>, ProductionSemanticImportErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.consts().next().is_some()
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
    {
        return Err(rejected("workgroup source instance arguments"));
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("workgroup source signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != 1
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
        || signature.references_error()
    {
        return Err(rejected("workgroup source signature"));
    }
    Ok(signature)
}

#[allow(clippy::too_many_arguments)]
fn workgroup_reference_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    signature: FnSig<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<WorkgroupReferenceSourceV1, ProductionSemanticImportErrorV1> {
    let ownership = [SemanticSourceArgumentOwnershipV1::SharedBorrow];
    if !exact_source_abi_v1(abi, &ownership) {
        return Err(rejected("workgroup source exact reference ABI"));
    }
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        signature.output(),
        &ownership,
    )?;
    let reference_ty = signature.inputs()[0];
    let workgroup_ty = rust_shared_reference_v1(reference_ty)
        .ok_or_else(|| rejected("workgroup source shared reference type"))?;
    let workgroup = rust_workgroup_capability_v1(tcx, workgroup_ty)
        .ok_or_else(|| rejected("workgroup source exact nominal type"))?;
    if !rust_kernel_brand_matches_root_v1(tcx, workgroup.kernel_brand, root) {
        return Err(rejected("workgroup source authenticated root mismatch"));
    }
    let reference = abi.source_input_types()[0];
    let owned = semantic_type_for_rust_v1(tcx, types, workgroup_ty)?;
    if reference == owned || exact_shared_pointee_v1(types, reference)? != owned {
        return Err(rejected("workgroup source reference pointee identity"));
    }
    Ok(WorkgroupReferenceSourceV1 {
        reference,
        workgroup: owned,
        output: abi.source_output_type(),
        source_identity: canonical_function_identities_v1(tcx, instance).function(),
        provenance: capability_memory_provenance_v1(root, contexts)?,
        brand: rustc_type_identity_v1(tcx, workgroup.kernel_brand.ty),
        epoch: rustc_type_identity_v1(tcx, workgroup.epoch),
    })
}

fn workgroup_epoch_field_v1<'tcx>(tcx: TyCtxt<'tcx>, workgroup: Ty<'tcx>, epoch: Ty<'tcx>) -> bool {
    let TyKind::Adt(definition, arguments) = *workgroup.kind() else {
        return false;
    };
    if !definition.is_struct() || definition.variants().len() != 1 {
        return false;
    }
    let fields = &definition.non_enum_variant().fields;
    fields.len() == WORKGROUP_FIELDS
        && fields.iter().nth(EPOCH_FIELD).is_some_and(|field| {
            field.name.as_str() == "epoch"
                && tcx
                    .try_normalize_erasing_regions(
                        TypingEnv::fully_monomorphized(),
                        field.ty(tcx, arguments),
                    )
                    .is_ok_and(|ty| ty == epoch)
        })
}

/// Complete bounded executable body check; provider and nominal brands are
/// authenticated separately before this structural predicate can be consumed.
fn reviewed_epoch_body_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    signature: FnSig<'tcx>,
) -> bool {
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
        || !body.user_type_annotations.is_empty()
        || signature.inputs().len() != 1
    {
        return false;
    }
    let Some(receiver) = rust_shared_reference_v1(signature.inputs()[0]) else {
        return false;
    };
    let Some(epoch) = rust_shared_reference_v1(signature.output()) else {
        return false;
    };
    if !workgroup_epoch_field_v1(tcx, receiver, epoch) {
        return false;
    }
    let normalized = |ty: Ty<'tcx>| {
        instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(ty),
            )
            .ok()
    };
    if normalized(body.local_decls[RETURN_PLACE].ty) != Some(signature.output())
        || normalized(body.local_decls[Local::from_usize(1)].ty) != Some(signature.inputs()[0])
    {
        return false;
    }
    let block = &body.basic_blocks[START_BLOCK];
    if block.is_cleanup
        || block.statements.len() != 1
        || !block
            .terminator
            .as_ref()
            .is_some_and(|terminator| matches!(terminator.kind, TerminatorKind::Return))
    {
        return false;
    }
    let StatementKind::Assign(assignment) = &block.statements[0].kind else {
        return false;
    };
    let Rvalue::Ref(_, BorrowKind::Shared, place) = &assignment.1 else {
        return false;
    };
    assignment.0 == RETURN_PLACE.into()
        && place.local == Local::from_usize(1)
        && matches!(place.projection.as_ref(), [ProjectionElem::Deref, ProjectionElem::Field(field, ty)]
            if field.as_usize() == EPOCH_FIELD && normalized(*ty) == Some(epoch))
}

#[cfg(test)]
mod tests;
