//! Exact original Context issuance coupled to its retained target operation.
use super::*;

pub(super) fn headers() -> usize {
    size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Owner>()
        + size_of::<Site>()
        + size_of::<Definition>()
        + size_of::<Option<usize>>()
        + size_of::<Result<()>>()
        + 10 * size_of::<usize>()
}

impl ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_> {
    /// Emits the actual two transition results and their explicit witness update.
    /// The consumer must separately prove the enabled predicate, state invariants,
    /// cut advancement and preserved observations. This is not an admission API.
    pub(in super::super) fn emit_context_issue_step_v211(
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
            let row = self.plan.instance(root, instance, out)?;
            let local = endpoint.source_local(out.budget)?;
            out.budget.charge_work(9)?;
            if owner.role != SourceRole::KernelContext
                || endpoint.execution_borrow_v163(out.budget)?.is_some()
                || owner.identity.instance != instance
                || owner.identity.destination != local
                || endpoint.source_type(out.budget)? != owner.identity.source_type
                || endpoint.source_function(out.budget)? != row.function
                || local.index() as usize >= row.locals.len()
            {
                return Err(mismatch());
            }
            let original = self.slots.correspondence(out)?.inventory(out.budget)?;
            let definition = endpoint.original_definition(out.budget)?.ok_or_else(mismatch)?;
            out.budget.charge_work(5)?;
            let definition = original.definitions().get(definition).ok_or_else(mismatch)?;
            let Definition::Result { operation, result: 0 } = definition.coordinate else {
                return Err(mismatch());
            };
            if definition.value != Some(owner.identity.value)
                || !matches!(original.operations()[operation_index(original, operation)?]
                    .operation.kind, OperationKind::Execution(Execution::ContextIssue))
            {
                return Err(mismatch());
            }
            let site = self.site(root, owner.identity, KirRole::Context, out)?;
            let local = row.locals.start.checked_add(local.index() as usize)
                .ok_or(Resource::Arithmetic)?;
            write!(out, "invocation_context_issue_coupled_v211(source, target, execution_map, InvocationSourceContextIssueV161 {{ destination: {local}, source_type: {} }}, MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, {})",
                owner.identity.source_type.index(), site.operation.block.function.0,
                site.operation.block.block, site.operation.operation, site.definition)
                .map_err(|_| out.error())?;
            self.check(out)
        })
    }
}
