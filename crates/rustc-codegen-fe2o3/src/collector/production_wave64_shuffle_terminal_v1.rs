//! Typed capture of an already authenticated exact primitive instance.

use super::*;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1,
    rustc_semantic_fn_abi_identity_v1, rustc_semantic_fn_abi_layout_identity_v1,
    rustc_type_identity_v1,
};
use crate::rustc_semantic_plan_v1::source_signature_v1;
use crate::trusted_device_items::Wave64ShuffleScalarV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentRoleV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticRustTypeKindV1, SemanticSourceArgumentOwnershipV1,
};
use rustc_middle::ty::{self, TypingEnv};

#[cfg(test)]
#[path = "production_wave64_shuffle_terminal_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::check_wave64_descriptor_mutations_v1;

fn scalar_type(
    types: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
    scalar: Wave64ShuffleScalarV1,
) -> bool {
    types.get(id.index() as usize).is_some_and(|ty| {
        ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
            && matches!(
                (scalar, ty.shape()),
                (
                    Wave64ShuffleScalarV1::U32,
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                ) | (
                    Wave64ShuffleScalarV1::I32,
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: true,
                        bits: 32
                    })
                ) | (
                    Wave64ShuffleScalarV1::F32,
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
                )
            )
    })
}

fn semantic_shape(
    types: &[SemanticTypeDeclV1],
    abi: &SemanticFunctionAbiV1,
    scalar: Wave64ShuffleScalarV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let inputs = abi.source_input_types();
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || inputs.len() != 3
        || inputs[1] != abi.source_output_type()
        || !scalar_type(types, inputs[1], scalar)
        || !scalar_type(types, inputs[2], Wave64ShuffleScalarV1::U32)
    {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(reference) = types.get(inputs[0].index() as usize)?.shape()
    else {
        return None;
    };
    if reference.kind() != SemanticPointerKindV1::Reference
        || reference.mutability() != SemanticMutabilityV1::Immutable
        || reference.metadata() != SemanticPointerMetadataV1::None
        || reference.address_space() != 0
        || reference.pointer_width_bits() != 64
    {
        return None;
    }
    Some((reference.pointee(), inputs[1]))
}

fn actual_abi_matches<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    actual: &rustc_target::callconv::FnAbi<'tcx, Ty<'tcx>>,
) -> Option<()> {
    use crate::production_semantic_fn_abi_v1::{
        ProductionSemanticFnAbiArgumentProducerV1 as Argument,
        ProductionSemanticFnAbiTypeProducerV1 as Type,
        ProductionSemanticFnAbiV1Producer as Producer,
        ProductionSemanticFnAbiValueProducerV1 as Value, construct_production_semantic_fn_abi_v1,
    };
    if actual.args.len() != 3 {
        return None;
    }
    let target = canonical_target_layout_v1(
        &crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx).ok()?,
    );
    let mut inputs = Vec::new();
    let mut arguments = Vec::new();
    inputs.try_reserve_exact(3).ok()?;
    arguments.try_reserve_exact(3).ok()?;
    for (ordinal, actual) in actual.args.iter().enumerate() {
        let source = Type::new(
            actual.layout,
            abi.source_input_types()[ordinal],
            if ordinal == 0 {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::ByValue
            },
        );
        inputs.push(source);
        arguments.push(Argument::source(Value::new(source)));
    }
    let output = Type::new(
        actual.ret.layout,
        abi.source_output_type(),
        SemanticSourceArgumentOwnershipV1::ByValue,
    );
    let producer = Producer::new(
        rustc_semantic_fn_abi_identity_v1(
            tcx,
            canonical_function_identities_v1(tcx, instance).function(),
            actual,
        ),
        rustc_semantic_fn_abi_layout_identity_v1(tcx, target, actual),
        rustc_abi::ExternAbi::Rust,
        actual,
        inputs,
        output,
        arguments,
        Value::new(output),
    )
    .ok()?;
    (construct_production_semantic_fn_abi_v1(producer)
        .ok()?
        .eq(abi))
    .then_some(())
}

pub(super) fn construct<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    scalar: Wave64ShuffleScalarV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Option<SemanticCompilerIntrinsicOperationV1> {
    if trusted_device_items::wave64_shuffle_scalar_for_instance_v1(tcx, instance) != Some(scalar) {
        return None;
    }
    let (context, element) = semantic_shape(types, abi, scalar)?;
    let signature = source_signature_v1(tcx, instance).ok()?;
    let actual = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
        )
        .ok()?;
    if abi.identity()
        != rustc_semantic_fn_abi_identity_v1(
            tcx,
            canonical_function_identities_v1(tcx, instance).function(),
            actual,
        )
        || abi.can_unwind() != actual.can_unwind
        || abi.fixed_count() != 3
        || abi.arguments().len() != 3
        || !abi.hidden_arguments().is_empty()
        || abi.return_value().ty() != element
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
        || abi.source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
    {
        return None;
    }
    let exact = |id: SemanticTypeIdV1, rust_ty| {
        types
            .get(id.index() as usize)
            .is_some_and(|record| record.identity() == rustc_type_identity_v1(tcx, rust_ty))
    };
    if !exact(element, signature.output()) {
        return None;
    }
    for (ordinal, (&id, &rust_ty)) in abi
        .source_input_types()
        .iter()
        .zip(signature.inputs())
        .enumerate()
    {
        let argument = &abi.arguments()[ordinal];
        if !exact(id, rust_ty)
            || argument.role() != SemanticAbiArgumentRoleV1::Source
            || argument.ty() != id
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
        {
            return None;
        }
    }
    let TyKind::Ref(_, rust_context, rustc_hir::Mutability::Not) = signature.inputs()[0].kind()
    else {
        return None;
    };
    if !exact(context, *rust_context) {
        return None;
    }
    actual_abi_matches(tcx, instance, abi, actual)?;
    // Keep the actual provider's unwind bit; capture grants no lane, convergence
    // or executable target authority. Parent occurrence/owner joins still apply.
    Some(SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex { context, element })
}
