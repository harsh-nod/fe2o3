//! Native scalar entry projections into the actual expanded target. This does
//! not execute a prologue or establish a complete frame relation.
use super::super::{paired::ExpandedScalarBindingsV196, tile_target::TileTargetV176};
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::block_index;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as TargetBlock, CanonicalKirDefinitionCoordinateV1 as Definition,
    ScalarType, Type,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalRoleV1 as LocalRole, SemanticScalarTypeV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unsupported {
    EntryStorageOrArgument,
    ScalarType,
    UnavailableEntry,
    ParameterCoverage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Argument {
    local: usize,
    bits: u32,
    target: Option<usize>,
}

struct RootSeed<'a, 'slots, 'view, 'source> {
    program: &'a SourceByteProgram<'slots, 'view, 'source>,
    target: &'a TileTargetV176<'slots, 'view, 'source>,
    root: usize,
    source_owner: u32,
    source_pc: usize,
    target_owner: u32,
    target_pc: usize,
    definitions: usize,
    arguments: Vec<Argument>,
    required: usize,
}

enum Selection<'a, 'slots, 'view, 'source> {
    Ready(RootSeed<'a, 'slots, 'view, 'source>),
    Unsupported(Unsupported),
}

fn mismatch() -> Error {
    Error::Statement("expanded scalar root seed differs from its source ABI or actual target")
}

fn headers() -> usize {
    2 * size_of::<RootSeed<'_, '_, '_, '_>>()
        + 2 * size_of::<Selection<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<Selection<'_, '_, '_, '_>>>()
        + size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>()
        + size_of::<Vec<(usize, u32)>>()
        + size_of::<Vec<Argument>>()
        + size_of::<Vec<bool>>()
        + 2 * size_of::<Option<usize>>()
        + 2 * size_of::<Result<Option<usize>>>()
        + 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 2 * size_of::<
            std::result::Result<
                fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, (usize, u32)>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Argument>>>()
        + size_of::<std::slice::Iter<'_, bool>>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + 2 * size_of::<Result<()>>()
        + size_of::<Definition>()
        + size_of::<TargetBlock>()
        + size_of::<Type>()
        + size_of::<Option<Type>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

impl<'a, 'slots, 'view, 'source> RootSeed<'a, 'slots, 'view, 'source> {
    fn derive(
        plan: &InvocationPlan<'view, 'source>,
        program: &'a SourceByteProgram<'slots, 'view, 'source>,
        target: &'a TileTargetV176<'slots, 'view, 'source>,
        bindings: &ExpandedScalarBindingsV196<'_, 'slots, 'view, 'source>,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Selection<'a, 'slots, 'view, 'source>> {
        let slots = program.source_slots(out)?;
        bindings.check_owner(slots, target, out)?;
        let relation = slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        out.budget.charge_work(8)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let scope = plan.root(root, out)?;
        let frame = plan.instance(root, 0, out)?;
        let (range, owner, source_pc) = program.roots.get(root).ok_or_else(mismatch)?;
        if !frame.active
            || frame.incoming.is_some()
            || frame.function != scope.function
            || range != &scope.instances
            || *owner != frame.function.index()
        {
            return Err(mismatch());
        }
        let function = program
            .functions
            .get(range.start)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        if function.root != root || function.instance != 0 {
            return Err(mismatch());
        }
        let Some(rows) = function.enter.scalar_root_arguments_v300(root, out)? else {
            return Ok(Selection::Unsupported(Unsupported::EntryStorageOrArgument));
        };
        let semantic = source.source_semantic(out.budget)?;
        let declaration = semantic
            .functions()
            .get(frame.function.index() as usize)
            .ok_or_else(mismatch)?;
        let archive = source.source_ssa(out.budget)?;
        let ssa = archive
            .plan_for_function(frame.function)
            .ok_or_else(mismatch)?
            .plan();
        let actual = target.inventory(out)?;
        let target_owner = target.root_function(root, out)?;
        let physical = actual
            .functions()
            .get(target_owner.0 as usize)
            .ok_or_else(mismatch)?;
        let target_pc = block_index(
            actual,
            TargetBlock {
                function: target_owner,
                block: 0,
            },
        )?;
        out.budget.charge_work(6)?;
        if rows.len() != declaration.abi().source_input_types().len()
            || !physical.blocks.contains(&target_pc)
            || actual.blocks()[target_pc].block.id
                != physical
                    .function
                    .body
                    .as_ref()
                    .and_then(|body| body.blocks.first())
                    .ok_or_else(mismatch)?
                    .id
        {
            return Err(mismatch());
        }
        let mut seen = vector(physical.function.signature.parameters.len(), out)?;
        out.budget
            .charge_work(physical.function.signature.parameters.len())?;
        seen.resize(physical.function.signature.parameters.len(), false);
        let mut arguments = vector(rows.len(), out)?;
        // Unsupported paths keep the bounded prepaid row capacity charged until
        // owner cleanup. Do not refund a whole query floor: endpoint lookups may
        // retain other owner-bound storage while this temporary roster is live.
        for (ordinal, &(local, bits)) in rows.iter().enumerate() {
            out.budget.charge_work(8)?;
            let relative = local.checked_sub(frame.locals.start).ok_or_else(mismatch)?;
            let declaration_local = declaration.locals().get(relative).ok_or_else(mismatch)?;
            if declaration_local.role()
                != LocalRole::Argument(u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?)
                || declaration.abi().source_input_types().get(ordinal)
                    != Some(&declaration_local.ty())
            {
                return Err(mismatch());
            }
            let ty = semantic
                .types()
                .get(declaration_local.ty().index() as usize)
                .ok_or_else(mismatch)?;
            let expected = match (bits, ty.shape()) {
                (0, TypeShape::Unit) => None,
                (
                    32,
                    TypeShape::Scalar(SemanticScalarTypeV1::Integer {
                        bits: 32,
                        signed: false,
                    }),
                ) => Some(Type::Scalar(ScalarType::U32)),
                _ => return Ok(Selection::Unsupported(Unsupported::ScalarType)),
            };
            let entries = ssa.entry_definitions();
            out.budget
                .charge_work((usize::BITS - entries.len().max(1).leading_zeros()) as usize + 2)?;
            let Ok(index) = entries.binary_search_by_key(
                &(u32::try_from(relative).map_err(|_| Resource::Arithmetic)?),
                |row| row.variable().get(),
            ) else {
                return Ok(Selection::Unsupported(Unsupported::UnavailableEntry));
            };
            let endpoint =
                relation.ssa_typed_endpoint_v36(root, 0, entries[index].value(), out.budget)?;
            if endpoint.source_function(out.budget)? != frame.function
                || endpoint.source_local(out.budget)?.index() as usize != relative
                || endpoint.source_type(out.budget)? != declaration_local.ty()
            {
                return Err(mismatch());
            }
            let mapped = match (expected, endpoint.original_definition(out.budget)?) {
                (None, None) => None,
                (Some(expected), Some(original)) => {
                    let definition = bindings.definition(original, out)?;
                    let row = actual.definitions().get(definition).ok_or_else(mismatch)?;
                    let Definition::FunctionArgument { function, argument } = row.coordinate else {
                        return Ok(Selection::Unsupported(Unsupported::ParameterCoverage));
                    };
                    let parameter = argument as usize;
                    if function != target_owner
                        || row.ty != &expected
                        || physical.function.signature.parameters.get(parameter) != Some(row.ty)
                        || seen.get(parameter) != Some(&false)
                    {
                        return Err(mismatch());
                    }
                    seen[parameter] = true;
                    Some(definition)
                }
                _ => return Err(mismatch()),
            };
            arguments.push(Argument {
                local,
                bits,
                target: mapped,
            });
        }
        out.budget.charge_work(seen.len())?;
        if seen.iter().any(|seen| !seen) {
            return Ok(Selection::Unsupported(Unsupported::ParameterCoverage));
        }
        let scratch = rows
            .capacity()
            .checked_mul(size_of::<(usize, u32)>())
            .and_then(|n| n.checked_add(seen.capacity().checked_mul(size_of::<bool>())?))
            .ok_or(Resource::Arithmetic)?;
        drop(rows);
        drop(seen);
        out.budget.release_storage(scratch)?;
        Ok(Selection::Ready(Self {
            program,
            target,
            root,
            source_owner: *owner,
            source_pc: *source_pc,
            target_owner: target_owner.0,
            target_pc,
            definitions: actual.definitions().len(),
            arguments,
            required: out.budget.storage(),
        }))
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.program
            .source_slots(out)?
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.charge_work(1)?;
        if !std::ptr::eq(self.program.slots, self.target.source_slots(out)?) {
            return Err(mismatch());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_expanded_root_seed_v300_tests.rs"]
mod tests;

#[path = "original_semantic_mir_expanded_root_seed_emit_v300.rs"]
mod emission;

impl<'slots, 'view, 'source> SourceByteProgram<'slots, 'view, 'source> {
    pub(in super::super) fn emit_expanded_root_seeds_v300(
        &self,
        plan: &InvocationPlan<'view, 'source>,
        target: &TileTargetV176<'slots, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let emit = |out: &mut Writer<'_, '_>| {
            self.source_slots(out)?;
            out.budget.reserve_storage(headers())?;
            let bindings = ExpandedScalarBindingsV196::derive(self.slots, target, out)?;
            let mut count = 0usize;
            for root in 0..self.roots.len() {
                out.budget.charge_work(1)?;
                match RootSeed::derive(plan, self, target, &bindings, root, out)? {
                    Selection::Ready(seed) => {
                        seed.emit(out)?;
                        count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                    Selection::Unsupported(reason) => {
                        writeln!(out, "// Expanded scalar root seed {root} unsupported: {reason:?}; no entry projection claim.")
                            .map_err(|_| out.error())?;
                    }
                }
            }
            Ok(count)
        };
        out.budget
            .reserve_storage(2 * std::mem::size_of_val(&emit) + std::mem::align_of_val(&emit))?;
        self.slots.with_source_query_v42(out, emit)
    }
}
