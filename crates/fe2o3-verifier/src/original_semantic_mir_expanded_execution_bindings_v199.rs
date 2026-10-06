//! Original execution provenance joined to actual authoritative target slots.
//! The emitted predicate is one relation conjunct, not a refinement theorem.
use super::super::invocations::InvocationPlan;
use super::{
    Error, Resource, Result, Writer,
    slots::SourceSlots,
    source_bytes::execution_loans::{self, Recipe, Role},
    tile_target::TileTargetV176,
};
use crate::mixed_optimizer_refinement_v26::semantics::{definition_index, operation_index};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as KirRole, OperationKind, Type,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceExecutionIdentityV199 as Identity, ProductionSourceExecutionOwnerV199 as Owner,
};
use fe2o3_mir_model::{
    SsaValueV1 as Value,
    semantic_mir_v1::{
        SemanticExecutionRoleV29 as SourceRole, SemanticRustTypeKindV1 as RustType,
        SemanticTerminatorKindV1 as Terminator,
    },
};
use std::{fmt::Write as _, mem::size_of};

pub(super) const SHARED: &str = concat!(
    include_str!("original_semantic_mir_expanded_execution_bindings_v199.vrs"),
    include_str!("original_semantic_mir_execution_correspondence_v205.vrs"),
    include_str!("original_semantic_mir_expanded_payload_lease_v209.vrs"),
    include_str!("original_semantic_mir_context_issue_coupling_v211.vrs"),
);

#[path = "original_semantic_mir_context_issue_coupling_v211.rs"]
mod context_issue;
#[path = "original_semantic_mir_context_issue_segment_v222.rs"]
mod context_issue_segment;
#[path = "original_semantic_mir_expanded_payload_lease_v209.rs"]
mod payload_lease;

pub(super) struct ExpandedExecutionBindingsV199<'plan, 'target, 'slots, 'view, 'source> {
    plan: &'plan InvocationPlan<'view, 'source>,
    slots: &'slots SourceSlots<'view, 'source>,
    target: &'target TileTargetV176<'slots, 'view, 'source>,
    required: usize,
}

#[cfg(test)]
#[path = "original_semantic_mir_expanded_execution_bindings_v199_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
struct Site {
    definition: usize,
    operation: Operation,
}

fn mismatch() -> Error {
    Error::Statement("execution relation differs from its source producer or actual scope slot")
}

impl<'plan, 'target, 'slots, 'view, 'source>
    ExpandedExecutionBindingsV199<'plan, 'target, 'slots, 'view, 'source>
{
    fn headers() -> usize {
        size_of::<Self>()
            + 2 * size_of::<Result<Self>>()
            + 2 * size_of::<Owner>()
            + 4 * size_of::<Site>()
            + 2 * size_of::<Option<Recipe>>()
            + size_of::<Option<execution_loans::ExecutionOperand>>()
            + 2 * size_of::<Definition>()
            + size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
            + 32 * size_of::<usize>()
            + size_of::<bool>()
            + size_of::<&str>()
            + size_of::<&Self>()
            + 2 * size_of::<usize>()
            + size_of::<Value>()
            + size_of::<&mut Writer<'_, '_>>()
            + size_of::<Result<()>>()
            + payload_lease::headers()
            + context_issue::headers()
            + context_issue_segment::headers()
    }

    pub(super) fn derive(
        plan: &'plan InvocationPlan<'view, 'source>,
        slots: &'slots SourceSlots<'view, 'source>,
        target: &'target TileTargetV176<'slots, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(Self::headers())?;
            out.budget.charge_work(3)?;
            if !std::ptr::eq(
                plan.source(out)?,
                slots.correspondence(out)?.source(out.budget)?,
            ) || !std::ptr::eq(slots, target.source_slots(out)?)
            {
                return Err(mismatch());
            }
            Ok(Self {
                plan,
                slots,
                target,
                required: out.budget.storage(),
            })
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.charge_work(2)?;
        if !std::ptr::eq(
            self.plan.source(out)?,
            self.slots.correspondence(out)?.source(out.budget)?,
        ) || !std::ptr::eq(self.slots, self.target.source_slots(out)?)
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(super) fn check_owner(
        &self,
        plan: &InvocationPlan<'_, '_>,
        slots: &SourceSlots<'_, '_>,
        target: &TileTargetV176<'_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            out.budget.charge_work(3)?;
            if !std::ptr::eq(self.plan, plan)
                || !std::ptr::eq(self.slots, slots)
                || !std::ptr::eq(self.target, target)
            {
                return Err(mismatch());
            }
            Ok(())
        })
    }

    fn frame(&self, root: usize, mut instance: usize, out: &mut Writer<'_, '_>) -> Result<usize> {
        let mut depth = 0usize;
        loop {
            out.budget.charge_work(2)?;
            let row = self.plan.instance(root, instance, out)?;
            if !row.active {
                return Err(mismatch());
            }
            match row.incoming {
                Some((parent, _)) if parent < instance => {
                    instance = parent;
                    depth = depth.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
                None if instance == 0 => return Ok(depth),
                _ => return Err(mismatch()),
            }
        }
    }

    fn site(
        &self,
        root: usize,
        identity: Identity,
        role: KirRole,
        out: &mut Writer<'_, '_>,
    ) -> Result<Site> {
        let source = self.plan.source(out)?;
        let semantic = source.source_semantic(out.budget)?;
        let producer = self.plan.instance(root, identity.instance, out)?;
        out.budget.charge_work(10)?;
        let expected = match role {
            KirRole::Context => SourceRole::KernelContext,
            KirRole::Workgroup => SourceRole::Workgroup,
            _ => return Err(mismatch()),
        };
        let function = semantic
            .functions()
            .get(producer.function.index() as usize)
            .ok_or_else(mismatch)?;
        let block = function
            .blocks()
            .get(identity.block.index() as usize)
            .ok_or_else(mismatch)?;
        let Terminator::Call(call) = block.terminator().kind() else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?.place();
        if !producer.active
            || !destination.projections().is_empty()
            || destination.local() != identity.destination
            || destination.ty() != identity.source_type
            || semantic
                .types()
                .get(identity.source_type.index() as usize)
                .map(|ty| ty.rust_type_kind())
                != Some(RustType::Execution(expected))
        {
            return Err(mismatch());
        }
        let original = self.slots.correspondence(out)?.inventory(out.budget)?;
        let function = Function(
            u32::try_from(self.plan.root(root, out)?.physical).map_err(|_| Resource::Arithmetic)?,
        );
        let index = original
            .definition_index_for_value(function, identity.value, out.budget)?
            .ok_or_else(mismatch)?;
        out.budget.charge_work(4)?;
        let row = original.definitions().get(index).ok_or_else(mismatch)?;
        let Definition::Result {
            operation,
            result: 0,
        } = row.coordinate
        else {
            return Err(mismatch());
        };
        if row.ty != &Type::Execution(role) {
            return Err(mismatch());
        }
        let tile = self.slots.tile_owner_v176(out)?;
        let neutral = tile.neutral_source_v162(out.budget)?;
        let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
        let [descendant] = descendants else {
            return Err(mismatch());
        };
        let span = tile
            .operation_span(operation, out.budget)?
            .ok_or_else(mismatch)?
            .expansion;
        out.budget.charge_work(4)?;
        if descendant.kind != Descendant::Retained
            || descendant.output
                != (Definition::Result {
                    operation: span.input,
                    result: 0,
                })
            || span.end.checked_sub(span.first) != Some(1)
        {
            return Err(mismatch());
        }
        let predecessor = neutral.output_inventory(out.budget)?;
        out.budget.charge_work(6)?;
        let before = &predecessor.definitions()[definition_index(predecessor, descendant.output)?];
        if before.ty != &Type::Execution(role) {
            return Err(mismatch());
        }
        let operation = Operation {
            block: span.input.block,
            operation: span.first,
        };
        if operation.block.function != self.target.root_function(root, out)? {
            return Err(mismatch());
        }
        let actual = self.target.inventory(out)?;
        out.budget.charge_work(12)?;
        let definition = definition_index(
            actual,
            Definition::Result {
                operation,
                result: 0,
            },
        )?;
        let row = &actual.definitions()[definition];
        if row.value != before.value || row.ty != &Type::Execution(role) {
            return Err(mismatch());
        }
        let kind = &actual.operations()[operation_index(actual, operation)?]
            .operation
            .kind;
        if !matches!(
            (role, kind),
            (
                KirRole::Context,
                OperationKind::Execution(Execution::ContextIssue)
            ) | (
                KirRole::Workgroup,
                OperationKind::Execution(Execution::WorkgroupDerive { .. })
            )
        ) {
            return Err(mismatch());
        }
        Ok(Site {
            definition,
            operation,
        })
    }

    /// Emits a necessary dynamic relation conjunct for this exact original SSA
    /// value. A caller still owes cut, history, effect and progress preservation.
    pub(super) fn emit_conjunct(
        &self,
        root: usize,
        instance: usize,
        value: Value,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_relation(root, instance, value, false, out)
    }

    /// Emits the necessary relation with an explicit `execution_map` witness.
    /// Its initialization and updates must be proved by coupled transitions;
    /// neither this emitter nor the map's shape grants a history invariant.
    pub(super) fn emit_mapped_conjunct_v205(
        &self,
        root: usize,
        instance: usize,
        value: Value,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_relation(root, instance, value, true, out)
    }

    fn emit_relation(
        &self,
        root: usize,
        instance: usize,
        value: Value,
        mapped: bool,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let relation = self.slots.correspondence(out)?;
            let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
            let owner = endpoint.execution_owner_v199(out.budget)?.ok_or_else(mismatch)?;
            let row = self.plan.instance(root, instance, out)?;
            let local = endpoint.source_local(out.budget)?.index() as usize;
            out.budget.charge_work(6)?;
            if endpoint.source_function(out.budget)? != row.function || local >= row.locals.len() {
                return Err(mismatch());
            }
            let (role, number) = match owner.role {
                SourceRole::KernelContext => (KirRole::Context, 0),
                SourceRole::Workgroup => (KirRole::Workgroup, 1),
                _ => return Err(Error::Statement("tile payload needs its separate aggregate leaf relation")),
            };
            let recipe = if endpoint.execution_borrow_v163(out.budget)?.is_some() {
                let (recipe, _) = Recipe::derive(self.slots, &endpoint, out)?;
                if recipe.source_type != owner.identity.source_type.index()
                    || recipe.role != if number == 0 { Role::Context } else { Role::Workgroup }
                { return Err(mismatch()); }
                Some(recipe)
            } else {
                if endpoint.source_type(out.budget)? != owner.identity.source_type { return Err(mismatch()); }
                None
            };
            let scope = self.site(root, owner.identity, role, out)?;
            let context = self.site(root, owner.context, KirRole::Context, out)?;
            let lease_recipe = if number == 1 {
                let actual = self.target.inventory(out)?;
                out.budget.charge_work(8)?;
                let OperationKind::Execution(Execution::WorkgroupDerive { context: receiver }) =
                    &actual.operations()[operation_index(actual, scope.operation)?].operation.kind
                else { return Err(mismatch()); };
                if actual.definitions()[context.definition].value != Some(*receiver) { return Err(mismatch()); }
                let operand = execution_loans::call_argument(self.slots, self.plan, root,
                    owner.identity.instance, owner.identity.block.index() as usize, 0, out)?
                    .ok_or_else(mismatch)?;
                if !operand.moved || !operand.recipe.mutable || operand.recipe.role != Role::Context
                    || operand.recipe.source_type != owner.context.source_type.index() {
                    return Err(mismatch());
                }
                Some(operand.recipe)
            } else { None };
            let frame = self.frame(root, instance, out)?;
            let local = row.locals.start.checked_add(local).ok_or(Resource::Arithmetic)?;
            out.budget.charge_work(1)?;
            let predicate = if mapped {
                "invocation_execution_mapped_v205(source, target, execution_map, "
            } else {
                "invocation_execution_related_v199(source, target, "
            };
            write!(out, "{predicate}InvocationExecutionBindingV199 {{ local: {local}, source_type: {}, context_type: {}, role: {number}, source_frame: {frame}, source_owner: {}, definition: {}, context_definition: {}, site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, context_site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, recipe: ",
                owner.identity.source_type.index(), owner.context.source_type.index(), row.function.index(),
                scope.definition, context.definition, scope.operation.block.function.0,
                scope.operation.block.block, scope.operation.operation, context.operation.block.function.0,
                context.operation.block.block, context.operation.operation).map_err(|_| out.error())?;
            if let Some(recipe) = recipe {
                write!(out, "Some(").map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(out, ")").map_err(|_| out.error())?;
            } else { write!(out, "None").map_err(|_| out.error())?; }
            write!(out, ", lease_recipe: ").map_err(|_| out.error())?;
            if let Some(recipe) = lease_recipe {
                write!(out, "Some(").map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(out, ")").map_err(|_| out.error())?;
            } else { write!(out, "None").map_err(|_| out.error())?; }
            write!(out, " }})").map_err(|_| out.error())?;
            self.check(out)
        })
    }
}
