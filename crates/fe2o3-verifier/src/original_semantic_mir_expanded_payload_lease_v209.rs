//! Payload values retain a generative Context lease independently of their
//! scalar expansion. Its dynamic identity comes from a coupled-history witness.
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36 as Endpoint;

pub(super) fn headers() -> usize {
    size_of::<Endpoint<'_, '_>>()
        + size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<Owner>()
        + size_of::<Identity>()
        + 2 * size_of::<Site>()
        + size_of::<execution_loans::ExecutionOperand>()
        + size_of::<Option<execution_loans::ExecutionOperand>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>()
}

impl ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_> {
    /// Emits the Context-parent conjunct for an owned tile or fragment. The
    /// caller separately proves all scalar leaves and coupled history updates.
    pub(in super::super) fn emit_payload_lease_conjunct_v209(
        &self,
        root: usize,
        instance: usize,
        value: Value,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let endpoint = self.slots.correspondence(out)?
                .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
            let owner = endpoint.execution_owner_v199(out.budget)?.ok_or_else(mismatch)?;
            out.budget.charge_work(5)?;
            if !matches!(owner.role, SourceRole::MaskedTileU32 { .. } | SourceRole::LaneFragmentU32 { .. })
                || endpoint.execution_borrow_v163(out.budget)?.is_some()
                || endpoint.source_type(out.budget)? != owner.identity.source_type {
                return Err(mismatch());
            }
            let row = self.plan.instance(root, instance, out)?;
            let local = endpoint.source_local(out.budget)?.index() as usize;
            out.budget.charge_work(3)?;
            if !row.active || local >= row.locals.len()
                || endpoint.source_function(out.budget)? != row.function {
                return Err(mismatch());
            }
            let workgroup = owner.workgroup.ok_or_else(mismatch)?;
            let scope = self.site(root, workgroup, KirRole::Workgroup, out)?;
            let context = self.site(root, owner.context, KirRole::Context, out)?;
            let actual = self.target.inventory(out)?;
            out.budget.charge_work(8)?;
            let OperationKind::Execution(Execution::WorkgroupDerive { context: receiver }) =
                &actual.operations()[operation_index(actual, scope.operation)?].operation.kind
            else { return Err(mismatch()); };
            if actual.definitions()[context.definition].value != Some(*receiver) {
                return Err(mismatch());
            }
            let operand = execution_loans::call_argument(self.slots, self.plan, root,
                workgroup.instance, workgroup.block.index() as usize, 0, out)?
                .ok_or_else(mismatch)?;
            if !operand.moved || !operand.recipe.mutable || operand.recipe.role != Role::Context
                || operand.recipe.source_type != owner.context.source_type.index() {
                return Err(mismatch());
            }
            let frame = self.frame(root, instance, out)?;
            let local = row.locals.start.checked_add(local).ok_or(Resource::Arithmetic)?;
            write!(out, "invocation_execution_payload_related_v209(source, target, execution_map, InvocationExecutionPayloadBindingV209 {{ local: {local}, source_type: {}, source_frame: {frame}, source_owner: {}, context_definition: {}, context_site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, lease_recipe: ",
                owner.identity.source_type.index(), row.function.index(), context.definition,
                context.operation.block.function.0, context.operation.block.block,
                context.operation.operation).map_err(|_| out.error())?;
            operand.recipe.emit(out)?;
            write!(out, " }})").map_err(|_| out.error())?;
            self.check(out)
        })
    }
}
