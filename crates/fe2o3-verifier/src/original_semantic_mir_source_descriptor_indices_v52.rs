//! Original checked descriptor access, separate from physical Option locators.
use super::super::source_bytes::{descriptor_loans::Recipe, witness_events::original_value};
use super::*;
use fe2o3_kernel_ir::{ScalarType, Type as PhysicalType};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCompilerIntrinsicOperationV1 as Intrinsic, SemanticDirectCallV1 as Call,
    SemanticDisjointIndexSpaceV1 as IndexSpace, SemanticMutabilityV1 as Mutability,
    SemanticOperandV1 as Operand, SemanticPointerKindV1 as PointerKind,
    SemanticPointerMetadataV1 as Metadata,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

fn owned_index(operand: &Operand) -> Result<&fe2o3_mir_model::semantic_mir_v1::SemanticPlaceV1> {
    match operand {
        Operand::Move(place) if place.projections().is_empty() => Ok(place),
        _ => Err(unsupported()),
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DescriptorIndexCall {
    input: usize,
    index: usize,
    destination: usize,
    continuation: usize,
    recipe: Recipe,
    witness_type: u32,
    option_type: u32,
    moved_receiver: bool,
    moved_index: bool,
}

impl DescriptorIndexCall {
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        block: usize,
        call: &Call,
        callable: &Callable,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(4)?;
        let Callable::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            return Ok(None);
        };
        let (descriptor, witness, element, raw, disjoint) = match *operation {
            Intrinsic::DisjointSliceGetMut {
                disjoint_slice,
                index_witness,
                element,
                raw_index,
            } => (disjoint_slice, index_witness, element, raw_index, false),
            Intrinsic::DisjointSliceGetDisjointMut {
                disjoint_slice,
                index_witness,
                element,
                raw_index,
                index_space: IndexSpace::Index1d,
            } => (disjoint_slice, index_witness, element, raw_index, true),
            _ => return Ok(None),
        };
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(32)?;
        let [receiver, index] = call.arguments() else {
            return Err(mismatch());
        };
        let local_operand = |operand: &Operand| {
            let (place, moved) = match operand {
                Operand::Copy(place) => (place, false),
                Operand::Move(place) => (place, true),
                _ => return Err(unsupported()),
            };
            if !place.projections().is_empty() {
                return Err(unsupported());
            }
            Ok((place.local(), place.ty(), moved))
        };
        let (receiver_local, receiver_type, moved_receiver) = local_operand(receiver)?;
        let index_place = owned_index(index)?;
        let (index_local, index_type, moved_index) = (index_place.local(), index_place.ty(), true);
        let row = plan.instance(root, instance, out)?;
        let relation = slots.correspondence(out)?;
        let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let option_type = destination.place().ty();
        let destination_local = destination.place().local().index() as usize;
        let continuation = destination.edge().target().index() as usize;
        if call.unwind() != Unwind::Unreachable
            || !call.variadic_argument_abis().is_empty()
            || binding.abi().source_input_types() != [receiver_type, index_type]
            || binding.abi().return_type() != option_type
            || index_type != witness
            || !moved_index
            || !destination.place().projections().is_empty()
            || destination.edge().role() != EdgeRole::CallReturn
            || continuation >= row.blocks.len()
            || function
                .locals()
                .get(destination_local)
                .map(|local| local.ty())
                != Some(option_type)
            || slots.has_original_object(root, instance, destination_local as u32, out)?
        {
            return Err(mismatch());
        }
        let receiver_value = original_value(
            slots,
            row.function,
            block,
            None,
            Role::CallArgument(0),
            receiver_local.index(),
            false,
            out,
        )?;
        let receiver_endpoint =
            relation.ssa_typed_endpoint_v36(root, instance, receiver_value, out.budget)?;
        if receiver_endpoint.source_type(out.budget)? != receiver_type
            || receiver_endpoint.source_local(out.budget)? != receiver_local
            || receiver_endpoint.source_function(out.budget)? != row.function
        {
            return Err(mismatch());
        }
        let recipe = Recipe::derive(slots, plan, root, &receiver_endpoint, out)?;
        if recipe.source_type != descriptor.index() || !recipe.mutable || !moved_receiver {
            return Err(mismatch());
        }
        let Some((raw_type, raw_bits, actual_disjoint, IndexSpace::Index1d)) =
            slots.witness_class(witness, out)?
        else {
            return Err(unsupported());
        };
        let class = slots
            .descriptor_slice_class(descriptor, out)?
            .ok_or_else(unsupported)?;
        let scalar = ScalarV30::from_source(semantic.types(), element)?;
        let scalar_type = match scalar {
            ScalarV30::Float { width: 32 } => ScalarType::F32,
            ScalarV30::Float { width: 64 } => ScalarType::F64,
            ScalarV30::Bool => ScalarType::Bool,
            ScalarV30::Integer {
                width: 8,
                signed: false,
            } => ScalarType::U8,
            ScalarV30::Integer {
                width: 16,
                signed: false,
            } => ScalarType::U16,
            ScalarV30::Integer {
                width: 32,
                signed: false,
            } => ScalarType::U32,
            ScalarV30::Integer {
                width: 64,
                signed: false,
            } => ScalarType::U64,
            ScalarV30::Integer {
                width: 8,
                signed: true,
            } => ScalarType::I8,
            ScalarV30::Integer {
                width: 16,
                signed: true,
            } => ScalarType::I16,
            ScalarV30::Integer {
                width: 32,
                signed: true,
            } => ScalarType::I32,
            ScalarV30::Integer {
                width: 64,
                signed: true,
            } => ScalarType::I64,
            _ => return Err(unsupported()),
        };
        if raw_type != raw
            || u32::from(raw_bits) != recipe.metadata_bits
            || actual_disjoint != disjoint
            || class.element != scalar_type
            || !class.readable
            || !class.writable
        {
            return Err(mismatch());
        }
        let index_value = original_value(
            slots,
            row.function,
            block,
            None,
            Role::CallArgument(1),
            index_local.index(),
            false,
            out,
        )?;
        let index_endpoint =
            relation.ssa_typed_endpoint_v36(root, instance, index_value, out.budget)?;
        if index_endpoint.source_type(out.budget)? != witness
            || index_endpoint.source_local(out.budget)? != index_local
            || index_endpoint.source_function(out.budget)? != row.function
            || index_endpoint.physical_type(out.budget)? != Some(&PhysicalType::INDEX)
            || index_endpoint.reference(out.budget)?.is_some()
        {
            return Err(mismatch());
        }
        let Shape::Enum { variants, .. } = semantic
            .types()
            .get(option_type.index() as usize)
            .ok_or_else(mismatch)?
            .shape()
        else {
            return Err(mismatch());
        };
        let [none, some] = &**variants else {
            return Err(mismatch());
        };
        let [payload] = some.fields().fields() else {
            return Err(mismatch());
        };
        let Shape::Pointer(pointer) = semantic
            .types()
            .get(payload.index() as usize)
            .ok_or_else(mismatch)?
            .shape()
        else {
            return Err(mismatch());
        };
        if none.is_uninhabited()
            || some.is_uninhabited()
            || none.discriminant() != 0
            || some.discriminant() != 1
            || !none.fields().fields().is_empty()
            || pointer.kind() != PointerKind::Reference
            || pointer.metadata() != Metadata::None
            || pointer.mutability() != Mutability::Mutable
            || pointer.pointee() != element
            || slots.logical_enum_variant_v47(option_type, 0, out)? != Some((0, 0))
            || slots.logical_enum_variant_v47(option_type, 1, out)? != Some((1, 1))
        {
            return Err(mismatch());
        }
        match slots.logical_enum_field_v47(option_type, 1, 0, out)? {
            Some((
                ty,
                super::super::slots::EnumFieldV47::Reference {
                    scalar: actual,
                    mutable: true,
                    ..
                },
            )) if ty == *payload && actual == scalar => (),
            _ => return Err(mismatch()),
        }
        let flat = |local: usize| {
            row.locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)
        };
        Ok(Some(Self {
            input: flat(receiver_local.index() as usize)?,
            index: flat(index_local.index() as usize)?,
            destination: flat(destination_local)?,
            continuation: row
                .blocks
                .start
                .checked_add(continuation)
                .ok_or(Resource::Arithmetic)?,
            recipe,
            witness_type: witness.index(),
            option_type: option_type.index(),
            moved_receiver,
            moved_index,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, " let source = invocation_source_byte_pc_v36(invocation_source_descriptor_index_v52(cursor.source, {}, {}, {}, ",
            self.destination, self.input, self.index).map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(out, ", {}, {}, {}, {}, little_endian), {});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n",
            self.witness_type, self.option_type, self.moved_receiver, self.moved_index, self.continuation).map_err(|_| out.error())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<DescriptorIndexCall>()
        + h::<Option<DescriptorIndexCall>>()
        + h::<Recipe>()
        + 2 * h::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + h::<
            Option<(
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
                u16,
                bool,
                IndexSpace,
            )>,
        >()
        + h::<(
            fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
            bool,
        )>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use std::mem::align_of;

    #[test]
    fn descriptor_index_owned_operand_rejects_copy_independently_of_nominal_type() {
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticLocalIdV1, SemanticPlaceV1, SemanticTypeIdV1,
        };
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(7),
            vec![],
            SemanticTypeIdV1::from_index(3),
        )
        .unwrap();
        let moved = Operand::Move(place.clone());
        let moved_place = owned_index(&moved).unwrap();
        assert!(matches!(&moved, Operand::Move(original) if std::ptr::eq(original, moved_place)));
        let copied = Operand::Copy(place);
        assert!(
            matches!(owned_index(&copied), Err(error) if format!("{error:?}") == format!("{:?}", unsupported()))
        );
        // This checks the ownership grammar only. The genuine source fixtures
        // independently supply and authenticate both nominal witness classes.
    }

    #[allow(dead_code)]
    struct RecipeFields {
        origin: usize,
        source_type: u32,
        reference_type: u32,
        generation: u32,
        instance: usize,
        block: u32,
        statement: usize,
        mutable: bool,
        metadata_bits: u32,
        width: u16,
    }

    #[allow(dead_code)]
    struct CallFields {
        input: usize,
        index: usize,
        destination: usize,
        continuation: usize,
        recipe: RecipeFields,
        witness_type: u32,
        option_type: u32,
        moved_receiver: bool,
        moved_index: bool,
    }

    #[test]
    fn descriptor_index_headers_match_independent_call_recipe_and_query_fields() {
        fn envelope<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(size_of::<Recipe>(), size_of::<RecipeFields>());
        assert_eq!(align_of::<Recipe>(), align_of::<RecipeFields>());
        assert_eq!(size_of::<DescriptorIndexCall>(), size_of::<CallFields>());
        assert_eq!(align_of::<DescriptorIndexCall>(), align_of::<CallFields>());
        let expected = envelope::<CallFields>()
            + envelope::<Option<CallFields>>()
            + envelope::<RecipeFields>()
            + 2 * envelope::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
            + envelope::<
                Option<(
                    fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
                    u16,
                    bool,
                    IndexSpace,
                )>,
            >()
            + envelope::<(
                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1,
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
                bool,
            )>()
            + 32 * size_of::<usize>()
            + 24 * size_of::<&()>();
        assert_eq!(headers(), expected);
    }
}
