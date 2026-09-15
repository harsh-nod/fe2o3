//! Independent source exclusivity for one retained physical-root argument.
//!
//! This does not change rustc's optional noalias/Unpin metadata or authenticate
//! source safety, host arguments, or launch authority. The production caller
//! supplies its replay-checked SSA owner after authenticating the root roster.

use super::*;
use fe2o3_mir_model::SemanticExpandedRootV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticBackendScalarV1, SemanticCanonAbiV1, SemanticExternAbiV1,
    SemanticFieldsShapeV1, SemanticRustTypeKindV1, SemanticRustcVariantsV1,
    SemanticScalarValidityRangeV1, SemanticTypeLayoutDetailsV1,
};

pub(super) const MAX_ARGUMENT_DIAGNOSTICS_V1: usize = 4_096;
const MAX_ARGUMENTS: usize = MAX_ARGUMENT_DIAGNOSTICS_V1;
const MAX_LOCALS: usize = MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1;

pub(super) struct UniqueSliceSourceV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    source: &'a SemanticFunctionDeclV1,
    view: &'a SemanticExpandedRootV1,
    arguments: Vec<Option<usize>>,
}

impl<'a> UniqueSliceSourceV1<'a> {
    pub(super) fn for_root(
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        binding: SemanticKernelBindingIdentityV1,
    ) -> Option<Self> {
        let semantic = owner.source_semantic();
        let view = owner.execution_view_for_root(root)?;
        if semantic.roots().binary_search(&root).is_err()
            || semantic.select_kernel_body_for_root_v1(root)?.body() != root
            || view.root() != root
            || view.source_body() != root
            || owner.execution_plan_for_root(root)?.function_identity() != view.body().identity()
        {
            return None;
        }
        Self::from_view(semantic, view, binding)
    }

    fn from_view(
        semantic: &'a AdmittedInertSemanticMirV1,
        view: &'a SemanticExpandedRootV1,
        binding: SemanticKernelBindingIdentityV1,
    ) -> Option<Self> {
        let root = view.root();
        let source = semantic.functions().get(root.index() as usize)?;
        let function = view.body();
        let count = source.abi().source_input_types().len();
        if semantic.roots().binary_search(&root).is_err()
            || view.source_body() != root
            || count > MAX_ARGUMENTS
            || source.locals().len() > MAX_LOCALS
            || function.locals().len() > MAX_LOCALS
            || source.role() != SemanticFunctionRoleV1::KernelRoot
            || function.role() != SemanticFunctionRoleV1::KernelRoot
            || source.kernel_entry()?.kernel_binding_identity() != binding
            || function.kernel_entry() != source.kernel_entry()
            || function.abi() != source.abi()
            || function.item_definition_identity() != source.item_definition_identity()
            || function.monomorphization_identity() != source.monomorphization_identity()
            || function.generic_type_arguments_identity()
                != source.generic_type_arguments_identity()
            || function.const_generic_arguments_identity()
                != source.const_generic_arguments_identity()
            || source.abi().extern_abi() != SemanticExternAbiV1::GpuKernel
            || source.abi().canon_abi() != SemanticCanonAbiV1::GpuKernel
            || source.abi().c_variadic()
            || source.abi().arguments().len() != count
            || source.abi().fixed_count() as usize != count
            || source.abi().source_argument_ownership().len() != count
            || view.local_origins().len() != function.locals().len()
        {
            return None;
        }
        let frame = view.instances().first()?;
        if frame.function() != root
            || frame.function_identity() != source.identity()
            || frame.parent().is_some()
            || frame.call_block().is_some()
            || frame.local_start() != 0
            || frame.local_count() as usize != source.locals().len()
            || frame.depth() != 0
        {
            return None;
        }
        let mut arguments = Vec::new();
        arguments.try_reserve_exact(count).ok()?;
        arguments.resize(count, None);
        for (local, declaration) in source.locals().iter().enumerate() {
            let SemanticLocalRoleV1::Argument(ordinal) = declaration.role() else {
                continue;
            };
            let ordinal = ordinal as usize;
            let slot = arguments.get_mut(ordinal)?;
            if slot.replace(local).is_some()
                || !exact_argument_mapping(source, view, ordinal, local)
            {
                return None;
            }
        }
        if arguments.iter().any(Option::is_none) {
            return None;
        }
        // Expanded callees may not retain Argument roles, even for identical types.
        for (local, declaration) in function.locals().iter().enumerate() {
            if let SemanticLocalRoleV1::Argument(ordinal) = declaration.role()
                && arguments.get(ordinal as usize) != Some(&Some(local))
            {
                return None;
            }
        }
        Some(Self {
            types: semantic.types(),
            source,
            view,
            arguments,
        })
    }

    pub(super) fn allocation(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        ordinal: usize,
        argument: &SemanticAbiArgumentV1,
        abi: AllocationContractV1,
    ) -> Option<AllocationContractV1> {
        let local = *self.arguments.get(ordinal)?.as_ref()?;
        if !std::ptr::eq(types, self.types)
            || !std::ptr::eq(function, self.view.body())
            || !std::ptr::eq(argument, function.abi().arguments().get(ordinal)?)
            || !exact_argument_mapping(self.source, self.view, ordinal, local)
            || function.abi().source_argument_ownership().get(ordinal)
                != Some(&SemanticSourceArgumentOwnershipV1::UniqueBorrow)
            || !exact_primitive_slice(types, argument)
            || abi.allocation_origin != u64::try_from(ordinal).ok()?.checked_add(1)?
            || abi.noalias_class != 0
            || !abi.writable
            || abi.singleton_object
        {
            return None;
        }
        Some(AllocationContractV1 {
            noalias_class: abi.allocation_origin.checked_add(1)?,
            ..abi
        })
    }
}

fn exact_argument_mapping(
    source: &SemanticFunctionDeclV1,
    view: &SemanticExpandedRootV1,
    ordinal: usize,
    local: usize,
) -> bool {
    let Some(ty) = source.abi().source_input_types().get(ordinal) else {
        return false;
    };
    let Some(argument) = source.abi().arguments().get(ordinal) else {
        return false;
    };
    let Some(origin) = view.local_origins().get(local) else {
        return false;
    };
    argument.role() == SemanticAbiArgumentRoleV1::Source
        && argument.value().source_ty() == *ty
        && argument.value().adjusted().is_none()
        && origin.instance().index() == 0
        && origin.function() == view.root()
        && origin.local().index() as usize == local
        && [source, view.body()].into_iter().all(|function| {
            function.locals().get(local).is_some_and(|declaration| {
                declaration.role() == SemanticLocalRoleV1::Argument(ordinal as u32)
                    && declaration.ty() == *ty
            })
        })
}

fn exact_primitive_slice(types: &[SemanticTypeDeclV1], argument: &SemanticAbiArgumentV1) -> bool {
    let value = argument.value();
    let Some(reference) = types.get(value.source_ty().index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Pointer(pointer) = reference.shape() else {
        return false;
    };
    let Some(slice) = types.get(pointer.pointee().index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Slice { element } = slice.shape() else {
        return false;
    };
    let Some(element) = types.get(element.index() as usize) else {
        return false;
    };
    let (primitive, maximum) = match element.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => {
            (SemanticBackendPrimitiveV1::integer(false, 8, 1), 1)
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed,
            bits: bits @ (8 | 16 | 32 | 64),
        }) => (
            SemanticBackendPrimitiveV1::integer(*signed, *bits, u64::from(*bits / 8)),
            (1_u128 << bits) - 1,
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
            bits: bits @ (32 | 64),
        }) => (
            SemanticBackendPrimitiveV1::float(*bits, u64::from(*bits / 8)),
            (1_u128 << bits) - 1,
        ),
        _ => return false,
    };
    let size = primitive.size_bytes().unwrap();
    let scalar = SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, maximum),
    );
    let pointer_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let Some(pointee) = reference.abi_properties().first_pointee() else {
        return false;
    };
    let SemanticAbiPassModeV1::Pair { first, .. } = value.mode() else {
        return false;
    };
    argument.role() == SemanticAbiArgumentRoleV1::Source
        && value.adjusted().is_none()
        && value.pointee_override().is_none()
        && !first.regular().no_alias()
        && pointer.kind() == SemanticPointerKindV1::Reference
        && pointer.mutability() == SemanticMutabilityV1::Mutable
        && pointer.address_space() == 0
        && pointer.pointer_width_bits() == 64
        && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
        && matches!(
            pointee.kind(),
            SemanticAbiPointeeKindV1::MutableReference { .. }
        )
        && pointee.guaranteed_size_bytes() == 0
        && pointee.reliable_alignment_bytes() <= size
        && reference.abi_properties().second_pointee().is_none()
        && [reference, slice, element].into_iter().all(|ty| {
            ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
                && !ty.layout().is_uninhabited()
                && matches!(
                    ty.layout().variants(),
                    SemanticRustcVariantsV1::Single { index: 0 }
                )
                && matches!(ty.layout().details(), SemanticTypeLayoutDetailsV1::None)
                && !ty.abi_properties().has_unsized_foreign_tail()
        })
        && !reference
            .abi_properties()
            .pass_indirectly_in_non_rustic_abis()
        && !element
            .abi_properties()
            .pass_indirectly_in_non_rustic_abis()
        && reference.layout().size_bytes() == Some(16)
        && reference.layout().alignment_bytes() == 8
        && reference.layout().fields().source_order_offsets_bytes() == Some(&[0, 8])
        && reference.layout().fields().memory_order_source_indices() == Some(&[0, 1])
        && *reference.layout().backend_repr()
            == SemanticBackendReprV1::scalar_pair(pointer_scalar, length_scalar)
        && slice.layout().size_bytes().is_none()
        && slice.layout().rustc_size_bytes() == 0
        && slice.layout().alignment_bytes() == size
        && *slice.layout().fields() == SemanticFieldsShapeV1::array(size, 0)
        && *slice.layout().backend_repr() == SemanticBackendReprV1::memory(false)
        && slice.abi_properties().first_pointee().is_none()
        && slice.abi_properties().second_pointee().is_none()
        && element.layout().size_bytes() == Some(size)
        && element.layout().alignment_bytes() == size
        && *element.layout().fields() == SemanticFieldsShapeV1::Primitive
        && *element.layout().backend_repr() == SemanticBackendReprV1::scalar(scalar)
        && element.abi_properties().first_pointee().is_none()
        && element.abi_properties().second_pointee().is_none()
}

pub(super) struct ArgumentDiagnosticV1<'a> {
    pub(super) function: &'a SemanticFunctionDeclV1,
    pub(super) ordinal: usize,
    pub(super) ty: &'a SemanticTypeDeclV1,
    pub(super) argument: &'a SemanticAbiArgumentV1,
}

impl fmt::Display for ArgumentDiagnosticV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fe2o3 unique-slice source check: function={:?} function_role={:?} canon_abi={:?} extern_abi={:?} root_binding={:?} arg={} type_id={} type_identity={:?} ownership={:?} pointee={:?} override={:?} adjusted_role={:?} passmode={:?}",
            self.function.identity(),
            self.function.role(),
            self.function.abi().canon_abi(),
            self.function.abi().extern_abi(),
            self.function
                .kernel_entry()
                .map(|entry| entry.kernel_binding_identity()),
            self.ordinal,
            self.argument.value().source_ty().index(),
            self.ty.identity(),
            self.function
                .abi()
                .source_argument_ownership()
                .get(self.ordinal),
            self.ty.abi_properties().first_pointee(),
            self.argument.value().pointee_override(),
            self.argument.role(),
            self.argument.mode(),
        )
    }
}

#[cfg(test)]
mod tests;
