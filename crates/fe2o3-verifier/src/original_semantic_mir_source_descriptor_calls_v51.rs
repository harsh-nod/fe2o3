//! Descriptor intrinsics consume exact original logical loans, never pointer casts.

use super::super::source_bytes::{descriptor_loans::Recipe, witness_events::original_value};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCompilerIntrinsicOperationV1 as Intrinsic, SemanticDirectCallV1 as Call,
    SemanticOperandV1 as Operand,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

#[derive(Clone, Copy, Debug)]
pub(super) struct DescriptorCall {
    input: usize,
    destination: usize,
    continuation: usize,
    recipe: Recipe,
    moved: bool,
}

impl DescriptorCall {
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
        let (descriptor, raw_index) = match operation {
            Intrinsic::DisjointSliceLen {
                disjoint_slice,
                raw_index,
                ..
            }
            | Intrinsic::WriteOnlyDisjointSliceLen {
                disjoint_slice,
                raw_index,
                ..
            } => (*disjoint_slice, *raw_index),
            _ => return Ok(None),
        };
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(16)?;
        let (place, moved) = match call.arguments() {
            [Operand::Copy(place)] => (place, false),
            [Operand::Move(place)] => (place, true),
            _ => return Err(unsupported()),
        };
        let row = plan.instance(root, instance, out)?;
        let semantic = slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let destination_local = destination.place().local().index() as usize;
        let continuation = destination.edge().target().index() as usize;
        if !place.projections().is_empty()
            || call.unwind() != Unwind::Unreachable
            || !call.variadic_argument_abis().is_empty()
            || binding.abi().source_input_types() != [place.ty()]
            || binding.abi().return_type() != raw_index
            || destination.place().ty() != raw_index
            || !destination.place().projections().is_empty()
            || destination.edge().role() != EdgeRole::CallReturn
            || continuation >= row.blocks.len()
            || function
                .locals()
                .get(destination_local)
                .map(|local| local.ty())
                != Some(raw_index)
            || slots.has_original_object(root, instance, destination_local as u32, out)?
        {
            return Err(mismatch());
        }
        let original = original_value(
            slots,
            row.function,
            block,
            None,
            Role::CallArgument(0),
            place.local().index(),
            false,
            out,
        )?;
        let endpoint = slots
            .correspondence(out)?
            .ssa_typed_endpoint_v36(root, instance, original, out.budget)?;
        if endpoint.source_type(out.budget)? != place.ty()
            || endpoint.source_local(out.budget)? != place.local()
            || endpoint.source_function(out.budget)? != row.function
        {
            return Err(mismatch());
        }
        let recipe = Recipe::derive(slots, plan, root, &endpoint, out)?;
        if recipe.source_type != descriptor.index()
            || recipe.mutable
            || ScalarV30::from_source(semantic.types(), raw_index)?
                != (ScalarV30::Integer {
                    width: recipe.metadata_bits,
                    signed: false,
                })
        {
            return Err(mismatch());
        }
        Ok(Some(Self {
            input: row
                .locals
                .start
                .checked_add(place.local().index() as usize)
                .ok_or(Resource::Arithmetic)?,
            destination: row
                .locals
                .start
                .checked_add(destination_local)
                .ok_or(Resource::Arithmetic)?,
            continuation: row
                .blocks
                .start
                .checked_add(continuation)
                .ok_or(Resource::Arithmetic)?,
            recipe,
            moved,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, " let source = invocation_source_byte_pc_v36(invocation_source_descriptor_length_v51(cursor.source, {}, {}, ",
            self.destination, self.input).map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(out, ", {}), {});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n",
            self.moved, self.continuation).map_err(|_| out.error())
    }

    pub(super) fn emit_frame_proof(self, cursor: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(
            out,
            " invocation_cut_frame_descriptor_length_v93(c{cursor}.source, {}, {}, ",
            self.destination, self.input
        )
        .map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(out, ", {});\n", self.moved).map_err(|_| out.error())
    }

    pub(super) fn emit_wf_proof(self, cursor: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.reserve_storage(
            size_of::<Self>() + 2 * size_of::<usize>() + size_of::<Result<()>>(),
        )?;
        out.budget.charge_work(2)?;
        write!(
            out,
            " invocation_source_descriptor_length_wf_v95(c{cursor}.source, {}, {}, ",
            self.destination, self.input
        )
        .map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(out, ", {});\n assert(invocation_source_byte_state_well_formed_v36(invocation_source_byte_pc_v36(invocation_source_descriptor_length_v51(c{cursor}.source, {}, {}, ", self.moved, self.destination, self.input).map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(
            out,
            ", {}), {}))) by {{ reveal(invocation_source_byte_state_well_formed_v36); }}\n",
            self.moved, self.continuation
        )
        .map_err(|_| out.error())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<DescriptorCall>()
        + h::<Option<DescriptorCall>>()
        + h::<Recipe>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 16 * size_of::<usize>()
        + 16 * size_of::<&()>()
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use std::mem::align_of;

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
        destination: usize,
        continuation: usize,
        recipe: RecipeFields,
        moved: bool,
    }

    #[test]
    fn descriptor_length_headers_match_independent_call_and_recipe_fields() {
        fn envelope<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(size_of::<Recipe>(), size_of::<RecipeFields>());
        assert_eq!(align_of::<Recipe>(), align_of::<RecipeFields>());
        assert_eq!(size_of::<DescriptorCall>(), size_of::<CallFields>());
        assert_eq!(align_of::<DescriptorCall>(), align_of::<CallFields>());
        let expected = envelope::<CallFields>()
            + envelope::<Option<CallFields>>()
            + envelope::<RecipeFields>()
            + envelope::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
            + 16 * size_of::<usize>()
            + 16 * size_of::<&()>();
        assert_eq!(headers(), expected);
    }
}
