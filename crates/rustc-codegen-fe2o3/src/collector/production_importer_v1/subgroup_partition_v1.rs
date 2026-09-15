//! Trusted target entry points for target-neutral, epoch-bound subgroup partitions.

use super::*;
use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSubgroupPartitionOperationV1 as Partition,
};
use rustc_middle::ty::{InstanceKind, TypeVisitableExt, TypingEnv};

pub(super) mod workgroup_source_v1;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct PartitionSourceV1 {
    terminal: TrustedDeviceItem,
    source_identity: SemanticFunctionIdentityV1,
    inputs: Vec<SemanticTypeIdentityV1>,
    output: SemanticTypeIdentityV1,
    receiver: SemanticTypeIdentityV1,
    epoch_borrow: Option<SemanticTypeIdentityV1>,
    brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
    kernel: SemanticTypeIdentityV1,
}

fn partition_terminal_v1(
    terminal: ProductionExecutionTerminalV1,
) -> Result<
    (
        TrustedDeviceItem,
        &'static [SemanticSourceArgumentOwnershipV1],
    ),
    ProductionSemanticImportErrorV1,
> {
    use ProductionExecutionTerminalV1 as T;
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
    Ok(match terminal {
        T::SubgroupPartitionDerive => (
            TrustedDeviceItem::Gfx950SubgroupWave16,
            &[SharedBorrow, SharedBorrow],
        ),
        T::SubgroupPartitionReduceSumF32 => (
            TrustedDeviceItem::Gfx950SubgroupReduceSumF32Wave16,
            &[SharedBorrow, ByValue],
        ),
        T::SubgroupPartitionReduceMaxF32 => (
            TrustedDeviceItem::Gfx950SubgroupReduceMaxF32Wave16,
            &[SharedBorrow, ByValue],
        ),
        T::SubgroupPartitionBroadcastF32 => (
            TrustedDeviceItem::Gfx950SubgroupBroadcastF32Wave16,
            &[SharedBorrow, ByValue, ByValue],
        ),
        _ => return Err(rejected("unsupported subgroup partition terminal")),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn subgroup_partition_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminal: ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use ProductionExecutionTerminalV1 as T;
    let (expected, ownership) = partition_terminal_v1(terminal)?;
    if trusted_device_items::classify(tcx, instance.def_id()) != Some(expected)
        || canonical_function_identities_v1(tcx, instance).function() != source_identity
        || instance.args.consts().next().is_some()
        || !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
    {
        return Err(rejected("subgroup partition trusted source identity"));
    }
    let root = root.ok_or_else(|| rejected("subgroup partition authenticated root"))?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let signature = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
        .map_err(|_| rejected("subgroup partition signature normalization"))?;
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || signature.has_non_region_param()
        || signature.has_infer()
        || signature.has_aliases()
        || signature.has_escaping_bound_vars()
    {
        return Err(rejected("subgroup partition source signature"));
    }
    let inputs = signature.inputs();
    let output = signature.output();
    if inputs.len() != ownership.len() || abi.source_input_types().len() != ownership.len() {
        return Err(rejected("subgroup partition source arity"));
    }
    require_capability_memory_terminal_abi_v1(tcx, abi, types, inputs, output, ownership)?;
    let receiver = rust_shared_reference_v1(inputs[0])
        .ok_or_else(|| rejected("subgroup partition shared receiver"))?;
    let partition_ty = if terminal == T::SubgroupPartitionDerive {
        output
    } else {
        receiver
    };
    let partition_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        partition_ty,
        TrustedDeviceItem::Gfx950SubgroupContext,
    )
    .ok_or_else(|| rejected("subgroup partition exact capability type"))?;
    let [brand_ty, epoch] = partition_arguments.as_slice() else {
        return Err(rejected("subgroup partition capability type arity"));
    };
    let brand = rust_execution_brand_v1(tcx, *brand_ty)
        .ok_or_else(|| rejected("subgroup partition kernel brand"))?;
    if instance.args.types().collect::<Vec<_>>().as_slice() != [*brand_ty, *epoch] {
        return Err(rejected("subgroup partition instance brand or epoch"));
    }
    let epoch_borrow_type = match terminal {
        T::SubgroupPartitionDerive => {
            let subgroup = rust_subgroup_v1(tcx, receiver)
                .ok_or_else(|| rejected("subgroup partition exact physical subgroup"))?;
            let epoch_type = rust_shared_reference_v1(inputs[1])
                .ok_or_else(|| rejected("subgroup partition epoch shared reference"))?;
            let epoch_borrow = rust_workgroup_epoch_v1(tcx, epoch_type)
                .ok_or_else(|| rejected("subgroup partition exact epoch borrow"))?;
            if subgroup.width != 64
                || !rust_same_kernel_brand_v1(subgroup.kernel_brand, brand)
                || !rust_same_kernel_brand_v1(epoch_borrow.kernel_brand, brand)
                || subgroup.epoch != *epoch
                || epoch_borrow.epoch != *epoch
            {
                return Err(rejected(
                    "subgroup partition substituted width, brand, or epoch",
                ));
            }
            Some(epoch_type)
        }
        T::SubgroupPartitionReduceSumF32 | T::SubgroupPartitionReduceMaxF32 | T::SubgroupPartitionBroadcastF32 => {
            if !matches!(inputs[1].kind(), TyKind::Float(FloatTy::F32)) || output != inputs[1] {
                return Err(rejected("subgroup partition exact f32 operands"));
            }
            if terminal == T::SubgroupPartitionBroadcastF32
                && !matches!(inputs[2].kind(), TyKind::Uint(UintTy::U32))
            {
                return Err(rejected("subgroup partition exact u32 lane"));
            }
            None
        }
        _ => return Err(rejected("unsupported subgroup partition operation")),
    };
    let source = PartitionSourceV1 {
        terminal: expected,
        source_identity,
        inputs: inputs
            .iter()
            .map(|ty| rustc_type_identity_v1(tcx, *ty))
            .collect(),
        output: rustc_type_identity_v1(tcx, output),
        receiver: rustc_type_identity_v1(tcx, receiver),
        epoch_borrow: epoch_borrow_type.map(|ty| rustc_type_identity_v1(tcx, ty)),
        brand: rustc_type_identity_v1(tcx, brand.ty),
        epoch: rustc_type_identity_v1(tcx, *epoch),
        kernel: rustc_type_identity_v1(tcx, brand.kernel),
    };
    expand_partition_source_v1(
        terminal,
        abi,
        types,
        &source,
        capability_memory_provenance_v1(root, contexts)?,
        source_identity,
    )
}

fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

fn exact_source_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    identity: SemanticTypeIdentityV1,
) -> bool {
    identity.as_bytes() != &[0; 32]
        && types
            .get(ty.index() as usize)
            .is_some_and(|decl| decl.identity() == identity)
        && types
            .iter()
            .filter(|decl| decl.identity() == identity)
            .count()
            == 1
}

fn exact_shared_pointee_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(reference.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(rejected("partition source shared reference"));
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
    {
        return Err(rejected("partition source shared reference shape"));
    }
    Ok(pointer.pointee())
}

fn exact_source_abi_v1(
    abi: &SemanticFunctionAbiV1,
    ownership: &[SemanticSourceArgumentOwnershipV1],
) -> bool {
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi.source_input_types().len() == ownership.len()
        && abi.arguments().len() == ownership.len()
        && abi.adjusted_arguments().len() == ownership.len()
        && usize::try_from(abi.fixed_count()).ok() == Some(ownership.len())
        && abi.source_argument_ownership() == ownership
        && abi.return_value().ty() == abi.source_output_type()
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .all(|(argument, ty)| {
                argument.role() == SemanticAbiArgumentRoleV1::Source
                    && argument.ty() == *ty
                    && matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
                    && argument.value().adjusted().is_none()
                    && argument.value().pointee_override().is_none()
            })
}

fn expand_partition_source_v1(
    terminal: ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    source: &PartitionSourceV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use ProductionExecutionTerminalV1 as T;
    let (expected, ownership) = partition_terminal_v1(terminal)?;
    if source.terminal != expected
        || source.source_identity != source_identity
        || source_identity.as_bytes() == &[0; 32]
        || source.kernel != provenance.kernel_marker()
        || source.brand.as_bytes() == &[0; 32]
        || source.epoch.as_bytes() == &[0; 32]
    {
        return Err(rejected("subgroup partition source identity or root"));
    }
    if !exact_source_abi_v1(abi, ownership) || source.inputs.len() != ownership.len() {
        return Err(rejected("subgroup partition exact source ABI"));
    }
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    if !inputs
        .iter()
        .zip(&source.inputs)
        .all(|(ty, identity)| exact_source_type_v1(types, *ty, *identity))
        || !exact_source_type_v1(types, output, source.output)
    {
        return Err(rejected("subgroup partition source type identity"));
    }
    let receiver = exact_shared_pointee_v1(types, inputs[0])?;
    if !exact_source_type_v1(types, receiver, source.receiver)
        || if terminal == T::SubgroupPartitionDerive {
            !workgroup_source_v1::exact_subgroup_source_layout_v1(types, receiver)
        } else {
            !semantic_exact_inhabited_aggregate_zst_v1(types, receiver)
        }
    {
        return Err(rejected("subgroup partition exact receiver type"));
    }
    let scalar = |ty: SemanticTypeIdV1, shape: SemanticScalarTypeV1| {
        types.get(ty.index() as usize).is_some_and(|decl| {
            decl.shape() == &SemanticTypeShapeV1::Scalar(shape)
                && decl.layout().size_bytes() == Some(4)
                && decl.layout().alignment_bytes() == 4
                && !decl.layout().is_uninhabited()
        })
    };
    // The trusted entry points select these widths; neither is inferred from a launch.
    let (width, partition_width) = (64, 16);
    let operation = match terminal {
        T::SubgroupPartitionDerive => {
            let epoch = exact_shared_pointee_v1(types, inputs[1])?;
            if !source
                .epoch_borrow
                .is_some_and(|identity| exact_source_type_v1(types, epoch, identity))
                || !semantic_exact_inhabited_aggregate_zst_v1(types, epoch)
                || !semantic_exact_inhabited_aggregate_zst_v1(types, output)
                || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
            {
                return Err(rejected("subgroup partition epoch borrow or result ABI"));
            }
            Partition::Derive {
                subgroup_reference: inputs[0],
                subgroup: receiver,
                epoch: inputs[1],
                partition: output,
                width,
                partition_width,
            }
        }
        T::SubgroupPartitionReduceSumF32 | T::SubgroupPartitionReduceMaxF32 | T::SubgroupPartitionBroadcastF32 => {
            if source.epoch_borrow.is_some()
                || output != inputs[1]
                || !scalar(inputs[1], SemanticScalarTypeV1::Float { bits: 32 })
                || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
            {
                return Err(rejected("subgroup partition exact f32 source transport"));
            }
            if terminal == T::SubgroupPartitionReduceSumF32 {
                Partition::ReduceSumF32 {
                    partition_reference: inputs[0],
                    partition: receiver,
                    element: inputs[1],
                    width,
                    partition_width,
                }
            } else if terminal == T::SubgroupPartitionReduceMaxF32 {
                Partition::ReduceMaxF32 {
                    partition_reference: inputs[0], partition: receiver, element: inputs[1],
                    width, partition_width,
                }
            } else {
                if !scalar(
                    inputs[2],
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    },
                ) {
                    return Err(rejected("subgroup partition exact u32 source lane"));
                }
                Partition::BroadcastF32 {
                    partition_reference: inputs[0],
                    partition: receiver,
                    element: inputs[1],
                    source_lane: inputs[2],
                    width,
                    partition_width,
                }
            }
        }
        _ => return Err(rejected("unsupported subgroup partition source operation")),
    };
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::SubgroupPartition(operation),
        SemanticExecutionCapabilitySignatureV1::new(inputs, output)
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?,
        provenance,
        source.brand,
        source.epoch,
        None,
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}
