//! Direct source writes retain the original receiver, witness, and scalar value.

use super::super::source_bytes::{descriptor_loans::Recipe, witness_events::original_value};
use super::*;
use fe2o3_kernel_ir::{ScalarType, Type as PhysicalType};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDisjointIndexSpaceV1 as IndexSpace, SemanticOperandV1 as Operand,
    SemanticWriteOnlyDisjointWriteKindV1 as WriteKind,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

#[derive(Clone, Copy, Debug)]
pub(super) struct ThreadWriteCall {
    input: usize,
    index: usize,
    destination: usize,
    continuation: usize,
    recipe: Recipe,
    witness_type: u32,
    moved_index: bool,
    value: TypedOperand,
}

impl ThreadWriteCall {
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        body: &SourceByteBody<'_, '_, '_>,
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
        let Intrinsic::WriteOnlyDisjointSliceWrite {
            disjoint_slice: descriptor,
            witness,
            element,
            raw_index,
            index_space,
            kind,
        } = *operation
        else {
            return Ok(None);
        };
        let (IndexSpace::Index1d, WriteKind::Thread { disjoint }) = (index_space, kind) else {
            return Err(unsupported());
        };
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(40)?;
        let [Operand::Move(receiver), index_operand, scalar_operand] = call.arguments() else {
            return Err(unsupported());
        };
        let (index, moved_index) = index_argument(index_operand, disjoint)?;
        let row = plan.instance(root, instance, out)?;
        let relation = slots.correspondence(out)?;
        let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let local = destination.place().local().index() as usize;
        let continuation = destination.edge().target().index() as usize;
        if !receiver.projections().is_empty()
            || !index.projections().is_empty()
            || !destination.place().projections().is_empty()
            || call.unwind() != Unwind::Unreachable
            || !call.variadic_argument_abis().is_empty()
            || binding.abi().source_input_types() != [receiver.ty(), witness, element]
            || binding.abi().return_type() != destination.place().ty()
            || index.ty() != witness
            || scalar_operand.ty() != element
            || ScalarV30::from_source(semantic.types(), destination.place().ty())?
                != ScalarV30::Bool
            || destination.edge().role() != EdgeRole::CallReturn
            || continuation >= row.blocks.len()
            || function.locals().get(local).map(|local| local.ty())
                != Some(destination.place().ty())
            || slots.has_original_object(root, instance, local as u32, out)?
        {
            return Err(mismatch());
        }
        let receiver_value = original_value(
            slots,
            row.function,
            block,
            None,
            Role::CallArgument(0),
            receiver.local().index(),
            false,
            out,
        )?;
        let receiver_endpoint =
            relation.ssa_typed_endpoint_v36(root, instance, receiver_value, out.budget)?;
        if receiver_endpoint.source_type(out.budget)? != receiver.ty()
            || receiver_endpoint.source_local(out.budget)? != receiver.local()
            || receiver_endpoint.source_function(out.budget)? != row.function
        {
            return Err(mismatch());
        }
        let recipe = Recipe::derive(slots, plan, root, &receiver_endpoint, out)?;
        let Some((raw, bits, actual_disjoint, IndexSpace::Index1d)) =
            slots.witness_class(witness, out)?
        else {
            return Err(unsupported());
        };
        let class = slots
            .descriptor_slice_class(descriptor, out)?
            .ok_or_else(unsupported)?;
        let scalar = ScalarV30::from_source(semantic.types(), element)?;
        let expected = match scalar {
            ScalarV30::Float { width: 32 } => ScalarType::F32,
            ScalarV30::Float { width: 64 } => ScalarType::F64,
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
        if recipe.source_type != descriptor.index()
            || !recipe.mutable
            || raw != raw_index
            || u32::from(bits) != recipe.metadata_bits
            || actual_disjoint != disjoint
            || class.element != expected
            || class.readable
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
            index.local().index(),
            false,
            out,
        )?;
        let index_endpoint =
            relation.ssa_typed_endpoint_v36(root, instance, index_value, out.budget)?;
        if index_endpoint.source_type(out.budget)? != witness
            || index_endpoint.source_local(out.budget)? != index.local()
            || index_endpoint.source_function(out.budget)? != row.function
            || index_endpoint.physical_type(out.budget)? != Some(&PhysicalType::INDEX)
            || index_endpoint.reference(out.budget)?.is_some()
        {
            return Err(mismatch());
        }
        let value = body.call_argument(block, 2, out)?;
        if value.ty() != element || value.scalar() != Some(scalar) {
            return Err(mismatch());
        }
        let flat = |local: usize| {
            row.locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)
        };
        Ok(Some(Self {
            input: flat(receiver.local().index() as usize)?,
            index: flat(index.local().index() as usize)?,
            destination: flat(local)?,
            continuation: row
                .blocks
                .start
                .checked_add(continuation)
                .ok_or(Resource::Arithmetic)?,
            recipe,
            witness_type: witness.index(),
            moved_index,
            value,
        }))
    }

    pub(super) fn emit(
        self,
        root: usize,
        instance: usize,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, " let event = InvocationSourceByteEventV36::ThreadWrite(InvocationSourceThreadWriteV88 {{ destination: {}, input: {}, index: {}, recipe: ", self.destination, self.input, self.index).map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(
            out,
            ", witness_type: {}, moved_receiver: true, moved_index: {}, value: ",
            self.witness_type, self.moved_index
        )
        .map_err(|_| out.error())?;
        self.value.emit(out)?;
        write!(out, " }});\n let after = invocation_source_byte_step_v36(cursor.source, event, {root}, {instance}, little_endian);\n let observations = cursor.observations.push(InvocationSourceStatementObservationV36 {{ root: {root}, instance: {instance}, block: {block}, statement: cursor.next_statement, event: Some(event), before: cursor.source, after }});\n let source = invocation_source_byte_pc_v36(after, {});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations, operands: seq![], returned: None }}\n", self.continuation).map_err(|_| out.error())
    }
}

fn index_argument(
    operand: &Operand,
    disjoint: bool,
) -> Result<(&fe2o3_mir_model::semantic_mir_v1::SemanticPlaceV1, bool)> {
    match operand {
        Operand::Move(index) => Ok((index, true)),
        Operand::Copy(index) if !disjoint => Ok((index, false)),
        _ => Err(unsupported()),
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<ThreadWriteCall>()
        + h::<Option<ThreadWriteCall>>()
        + h::<Recipe>()
        + h::<TypedOperand>()
        + 2 * h::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + h::<
            Option<(
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
                u16,
                bool,
                IndexSpace,
            )>,
        >()
        + 40 * size_of::<usize>()
        + 32 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticConstantV1, SemanticConstantValueV1, SemanticLocalIdV1, SemanticPlaceV1,
        SemanticTypeIdV1,
    };

    #[test]
    fn thread_write_witness_ownership_keeps_disjoint_moves_and_plain_copies_distinct() {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(7),
            vec![],
            SemanticTypeIdV1::from_index(13),
        )
        .unwrap();
        for disjoint in [false, true] {
            let moved = Operand::Move(place.clone());
            assert_eq!(index_argument(&moved, disjoint).unwrap(), (&place, true));
        }
        let copied = Operand::Copy(place.clone());
        assert_eq!(index_argument(&copied, false).unwrap(), (&place, false));
        assert!(index_argument(&copied, true).is_err());
        let constant = Operand::Constant(SemanticConstantV1::new(
            place.ty(),
            SemanticConstantValueV1::ZeroSized,
        ));
        for disjoint in [false, true] {
            assert!(index_argument(&constant, disjoint).is_err());
        }
    }
}
