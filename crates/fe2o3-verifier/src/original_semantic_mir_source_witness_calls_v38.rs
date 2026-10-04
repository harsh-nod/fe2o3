//! Compiler-intrinsic witness semantics over independent original source state.

use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceReferenceCarrierV38 as Carrier;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1 as Block, SemanticDisjointIndexSpaceV1 as IndexSpace,
    SemanticOperandV1 as Operand, SemanticTypeIdV1 as TypeId,
};
use fe2o3_pliron::{
    ProductionSemanticExpressionV2 as Expression, ProductionSemanticScalarTypeV2 as Scalar,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WitnessCall {
    Issue {
        ty: u32,
        bits: u16,
    },
    Convert {
        input: usize,
        input_type: u32,
        output_type: u32,
        moved: bool,
    },
    Read {
        input: usize,
        source_type: u32,
        origin: usize,
        generation: u32,
        instance: usize,
        block: u32,
        statement: usize,
        moved: bool,
    },
}

impl WitnessCall {
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        block: usize,
        call: &Call,
        operation: Intrinsic,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(7)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let output = destination.place().ty();
        let mut class = |ty: TypeId, disjoint: bool, raw: TypeId| -> Result<u16> {
            match slots.witness_class(ty, out)? {
                Some((actual, bits, exclusive, IndexSpace::Index1d))
                    if actual == raw && exclusive == disjoint =>
                {
                    Ok(bits)
                }
                _ => Err(unsupported()),
            }
        };
        let (witness, raw, disjoint) = match operation {
            Intrinsic::ThreadIndex1d {
                index_witness,
                raw_index,
            } => {
                let bits = class(index_witness, false, raw_index)?;
                if !call.arguments().is_empty() || output != index_witness {
                    return Err(mismatch());
                }
                return Ok(Some(Self::Issue {
                    ty: index_witness.index(),
                    bits,
                }));
            }
            Intrinsic::ThreadIndexIntoDisjoint {
                input_witness,
                output_witness,
                raw_index,
                index_space: IndexSpace::Index1d,
            } => {
                let input_bits = class(input_witness, false, raw_index)?;
                if class(output_witness, true, raw_index)? != input_bits || output != output_witness
                {
                    return Err(mismatch());
                }
                let (input, moved, ty) = argument(slots, plan, root, instance, call, out)?;
                if ty != input_witness {
                    return Err(mismatch());
                }
                return Ok(Some(Self::Convert {
                    input,
                    input_type: input_witness.index(),
                    output_type: output_witness.index(),
                    moved,
                }));
            }
            Intrinsic::ThreadIndexGet {
                index_witness,
                raw_index,
            } => (index_witness, raw_index, false),
            Intrinsic::DisjointIndexGet {
                index_witness,
                raw_index,
                index_space: IndexSpace::Index1d,
            } => (index_witness, raw_index, true),
            _ => return Ok(None),
        };
        let bits = class(witness, disjoint, raw)?;
        if output != raw {
            return Err(mismatch());
        }
        let (input, moved, ty) = argument(slots, plan, root, instance, call, out)?;
        let relation = slots.correspondence(out)?;
        let block_id = Block::from_index(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
        let reader = relation
            .index_reader_computation_v35(root, instance, block_id, out.budget)?
            .ok_or_else(mismatch)?;
        if reader.expression(out.budget)?
            != (Expression::GlobalInvocation1d {
                scalar: Scalar::Integer {
                    signed: false,
                    bits,
                },
            })
        {
            return Err(mismatch());
        }
        let definition = reader.original_definition(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        let physical = inventory
            .definitions()
            .get(definition)
            .ok_or_else(mismatch)?;
        if !matches!(
            physical.ty,
            fe2o3_kernel_ir::Type::Scalar(
                fe2o3_kernel_ir::ScalarType::Index
                    | fe2o3_kernel_ir::ScalarType::U32
                    | fe2o3_kernel_ir::ScalarType::U64
            )
        ) {
            return Err(mismatch());
        }
        let row = plan.instance(root, instance, out)?;
        let local = u32::try_from(input.checked_sub(row.locals.start).ok_or_else(mismatch)?)
            .map_err(|_| Resource::Arithmetic)?;
        let value = super::super::super::source_bytes::witness_events::original_value(
            slots,
            row.function,
            block,
            None,
            fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::CallArgument(0),
            local,
            false,
            out,
        )?;
        let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
        let reference = endpoint.reference(out.budget)?.ok_or_else(mismatch)?;
        if endpoint.source_type(out.budget)? != ty
            || endpoint.source_local(out.budget)?.index() != local
            || endpoint.source_function(out.budget)? != row.function
            || reference.carrier(out.budget)? != Carrier::StableScalar
            || reference.origin_type(out.budget)? != witness
        {
            return Err(mismatch());
        }
        let origin_instance = reference.origin_instance(out.budget)?;
        let origin_row = plan.instance(root, origin_instance, out)?;
        if origin_row.function != reference.origin_function(out.budget)? {
            return Err(mismatch());
        }
        let origin_local = reference.origin_local(out.budget)?.index() as usize;
        if origin_local >= origin_row.locals.len() {
            return Err(mismatch());
        }
        let (borrow_instance, borrow_block, statement) = reference.borrow_site(out.budget)?;
        Ok(Some(Self::Read {
            input,
            source_type: witness.index(),
            origin: origin_row
                .locals
                .start
                .checked_add(origin_local)
                .ok_or(Resource::Arithmetic)?,
            generation: reference.origin_generation(out.budget)?,
            instance: borrow_instance,
            block: borrow_block.index(),
            statement: statement.ok_or_else(unsupported)?,
            moved,
        }))
    }

    pub(super) fn emit(self, destination: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        match self {
            Self::Issue { ty, bits } => write!(out,
                "invocation_source_issue_witness_v38(cursor.source, {destination}, {ty}, {bits})"),
            Self::Convert { input, input_type, output_type, moved } => write!(out,
                "invocation_source_convert_witness_v38(cursor.source, {destination}, {input}, {input_type}, {output_type}, {moved})"),
            Self::Read { input, source_type, origin, generation, instance, block, statement, moved } => write!(out,
                "invocation_source_read_witness_v38(cursor.source, {destination}, {input}, {source_type}, {origin}, {generation}, {instance}, {block}, {statement}, {moved})"),
        }.map_err(|_| out.error())
    }
}

fn argument(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    call: &Call,
    out: &mut Writer<'_, '_>,
) -> Result<(usize, bool, TypeId)> {
    out.budget.charge_work(5)?;
    let [operand] = call.arguments() else {
        return Err(mismatch());
    };
    let (place, moved) = match operand {
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return Err(unsupported()),
    };
    if !place.projections().is_empty()
        || slots
            .legacy_descriptor_by_source(root, instance, place.local().index(), out)?
            .is_some()
    {
        return Err(unsupported());
    }
    let row = plan.instance(root, instance, out)?;
    let local = place.local().index() as usize;
    if local >= row.locals.len() {
        return Err(mismatch());
    }
    Ok((
        row.locals
            .start
            .checked_add(local)
            .ok_or(Resource::Arithmetic)?,
        moved,
        place.ty(),
    ))
}

pub(super) fn headers() -> usize {
    size_of::<WitnessCall>()
        + 2 * size_of::<Result<Option<WitnessCall>>>()
        + size_of::<Option<WitnessCall>>()
        + size_of::<Expression>()
        + size_of::<Result<Expression>>()
        + size_of::<(usize, bool, TypeId)>()
        + size_of::<Result<(usize, bool, TypeId)>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceIndexReadV35<'_, '_>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
}
