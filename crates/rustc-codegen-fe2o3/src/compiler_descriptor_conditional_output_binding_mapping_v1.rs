//! Bounded descriptive packing from admitted FnAbi and retained compiler custody.
//! This does not select a new KIR body or establish source/KIR correspondence.
use super::{Error, product, require_flat_abi_v1, sum};
use crate::collector::{ContextRootVisitErrorV29, RetainedContextEntriesV29};
use fe2o3_kernel_descriptor::MAX_CONDITIONAL_ARGUMENTS_V1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::{
    SemanticLogicalArgumentErrorV1, SemanticLogicalArgumentMapV1, SemanticSourceArgumentBindingV1,
    semantic_mir_v1::{
        AdmittedInertSemanticMirV1, SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1,
        SemanticExecutionRoleV29, SemanticExternAbiV1, SemanticFunctionIdV1, SemanticLocalIdV1,
        SemanticOperandV1, SemanticRustTypeKindV1, SemanticSourceArgumentOwnershipV1,
        SemanticTerminatorKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
    },
};
use std::mem::size_of;

/// Query coordinates only. Neither these integers nor the result confer custody.
pub(crate) struct GeneratedFieldCoordinatesV1 {
    pub(crate) source: u32,
    pub(crate) adjusted: u32,
    pub(crate) local: SemanticLocalIdV1,
    pub(crate) ty: SemanticTypeIdV1,
}

/// Derives a generated position independently of the canonical parameter slot.
/// Logical context removal requires the original compiler receipt and exact
/// root-to-helper transport. Semantic execution tags alone never admit elision.
/// The generic lowerer must still authenticate the selected source/KIR body;
/// this query cannot turn a logical helper into a kernel entry.
pub(crate) fn checked_generated_field_v1(
    semantic: &AdmittedInertSemanticMirV1,
    (root_id, body_id): (SemanticFunctionIdV1, SemanticFunctionIdV1),
    contexts: Option<&RetainedContextEntriesV29>,
    query: GeneratedFieldCoordinatesV1,
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    budget.charge_work(16)?;
    let root = semantic
        .functions()
        .get(root_id.index() as usize)
        .ok_or(Error::Root)?;
    let body = semantic
        .functions()
        .get(body_id.index() as usize)
        .ok_or(Error::Root)?;
    if count > MAX_CONDITIONAL_ARGUMENTS_V1
        || root.abi().source_input_types().len() != count
        || body.abi().source_input_types().len() > MAX_CONDITIONAL_ARGUMENTS_V1 + 1
    {
        return Err(Error::UnsupportedAbi);
    }
    require_flat_abi_v1(root.abi(), semantic.types(), budget)?;
    let retained = contexts
        .map(|contexts| contexts.materialization_source_v29(semantic, budget))
        .transpose()
        .map_err(|error| match error {
            ContextRootVisitErrorV29::Resource(error) => Error::Resource(error),
            ContextRootVisitErrorV29::Source(_) => Error::ContextAdmission,
            ContextRootVisitErrorV29::Consumer(never) => match never {},
        })?
        .flatten();
    let mut context = None;
    if let Some(retained) = &retained {
        for entry in retained.roots() {
            budget.charge_work(4)?;
            if entry.root().0 == root_id
                && entry.helper().0 == body_id
                && context.replace(entry).is_some()
            {
                return Err(Error::ContextAdmission);
            }
        }
    }
    let abi = body.abi();
    if abi.c_variadic()
        || abi.extern_abi() == SemanticExternAbiV1::RustCall
        || abi.arguments().len() != abi.source_input_types().len()
        || !abi.hidden_arguments().is_empty()
    {
        return Err(Error::UnsupportedAbi);
    }
    if let Some(entry) = context {
        budget.charge_work(80)?;
        if entry.root().1 != root.identity()
            || entry.helper().1 != body.identity()
            || entry.helper_operands().len() != abi.source_input_types().len()
            || abi.source_input_types().len() != count + 1
        {
            return Err(Error::ContextAdmission);
        }
    } else {
        require_flat_abi_v1(abi, semantic.types(), budget)?;
        budget.charge_work(product(count, 2)?)?;
        if abi.source_input_types() != root.abi().source_input_types()
            || abi.source_argument_ownership() != root.abi().source_argument_ownership()
        {
            return Err(Error::UnsupportedAbi);
        }
        // Use the admitted transparent-wrapper selector unchanged. Matching
        // signatures alone must not substitute an unrelated helper body.
        budget.charge_work(product(
            sum(&[
                semantic.roots().len(),
                root.abi().source_input_types().len(),
                root.abi().source_argument_ownership().len(),
                root.blocks().len(),
                1,
            ])?,
            32,
        )?)?;
        for block in root.blocks() {
            budget.charge_work(product(block.statements().len(), 4)?)?;
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                budget.charge_work(product(call.arguments().len(), 8)?)?;
            }
        }
        if semantic
            .select_kernel_body_for_root_v1(root_id)
            .is_none_or(|selected| selected.body() != body_id)
        {
            return Err(Error::Root);
        }
    }

    with_argument_maps_v1(
        semantic,
        root_id,
        body_id,
        budget,
        |root_map, body_map, budget| {
            let mut root_arguments = root_map.source_arguments();
            let mut selected = None;
            let mut generated = 0;
            let mut elided = false;
            for (source, adjusted) in body_map
                .source_arguments()
                .zip(body_map.adjusted_arguments())
            {
                budget.charge_work(128)?;
                if adjusted.source_argument() != source.ordinal()
                    || adjusted.tuple_field().is_some()
                    || adjusted.local_field().is_some()
                    || source.binding() != SemanticSourceArgumentBindingV1::Whole(adjusted.local())
                    || adjusted.abi().role() != SemanticAbiArgumentRoleV1::Source
                    || adjusted.abi().ty() != source.ty()
                    || adjusted.abi().value().adjusted().is_some()
                    || adjusted.abi().value().pointee_override().is_some()
                {
                    return Err(Error::UnsupportedAbi);
                }
                let ty = semantic
                    .types()
                    .get(source.ty().index() as usize)
                    .ok_or(Error::UnsupportedAbi)?;
                let operand = context
                    .and_then(|entry| entry.helper_operands().get(source.ordinal() as usize));
                let is_context = context.is_some_and(|entry| {
                    entry.context() == (source.ty(), ty.identity())
                        && matches!(operand, Some(SemanticOperandV1::Move(place))
                        if place.local() == entry.helper_argument()
                            && place.ty() == source.ty() && place.projections().is_empty())
                });
                if is_context {
                    if elided
                        || ty.rust_type_kind()
                            != SemanticRustTypeKindV1::Execution(
                                SemanticExecutionRoleV29::KernelContext,
                            )
                        || ty.layout().size_bytes() != Some(0)
                        || !matches!(adjusted.abi().mode(), SemanticAbiPassModeV1::Ignore)
                        || source.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
                        || query.source == source.ordinal()
                    {
                        return Err(Error::ContextAdmission);
                    }
                    elided = true;
                    continue;
                }
                if matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_))
                    || matches!(ty.shape(), SemanticTypeShapeV1::Tuple(_))
                    || !matches!(
                        adjusted.abi().mode(),
                        SemanticAbiPassModeV1::Direct(_) | SemanticAbiPassModeV1::Pair { .. }
                    )
                {
                    return Err(Error::UnsupportedAbi);
                }
                let physical_source = root_arguments.next().ok_or(Error::OutputArgument)?;
                if source.ty() != physical_source.ty()
                    || source.source_ownership() != physical_source.source_ownership()
                {
                    return Err(Error::OutputArgument);
                }
                if context.is_some()
                    && !matches!(operand, Some(SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place))
                    if physical_source.binding() == SemanticSourceArgumentBindingV1::Whole(place.local())
                        && place.ty() == physical_source.ty() && place.projections().is_empty())
                {
                    return Err(Error::ContextAdmission);
                }
                if query.source == source.ordinal() {
                    if query.adjusted != adjusted.ordinal()
                        || query.local != adjusted.local()
                        || query.ty != source.ty()
                    {
                        return Err(Error::OutputArgument);
                    }
                    selected = Some(generated);
                }
                generated += 1;
            }
            if generated != count || root_arguments.next().is_some() || elided != context.is_some()
            {
                return Err(Error::OutputArgument);
            }
            selected.ok_or(Error::OutputArgument)
        },
    )
}

fn with_argument_maps_v1<'a, 'w, T>(
    semantic: &'a AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
    body: SemanticFunctionIdV1,
    budget: &mut Budget<'w>,
    consume: impl for<'s> FnOnce(
        &'s SemanticLogicalArgumentMapV1<'a>,
        &'s SemanticLogicalArgumentMapV1<'a>,
        &mut Budget<'w>,
    ) -> Result<T, Error>,
) -> Result<T, Error> {
    budget.charge_work(4)?;
    let mut sources = 0;
    let mut locals = 0;
    for id in [root, body] {
        let function = semantic
            .functions()
            .get(id.index() as usize)
            .ok_or(Error::Root)?;
        if function.abi().extern_abi() == SemanticExternAbiV1::RustCall {
            return Err(Error::UnsupportedAbi);
        }
        sources = sum(&[sources, function.abi().source_input_types().len()])?;
        locals = sum(&[locals, function.locals().len()])?;
    }
    // The existing mapper allocates one source-local index per source argument.
    // RustCall expansion is excluded before construction. These match its
    // shared caller's logical charges, not allocator/RSS guarantees.
    let bytes = sum(&[
        product(2, size_of::<SemanticLogicalArgumentMapV1<'_>>())?,
        product(sources, size_of::<Option<SemanticLocalIdV1>>())?,
    ])?;
    let work = product(sum(&[locals, sources, 1])?, 16)?;
    let allocation_error = |error| match error {
        SemanticLogicalArgumentErrorV1::AllocationFailure => Error::Resource(Resource::Allocation),
        SemanticLogicalArgumentErrorV1::UnknownFunction => Error::Root,
    };
    budget.with_prepaid_scope(budget.storage(), 0, work, bytes, |budget| {
        let root_map = semantic
            .logical_arguments_v1(root)
            .map_err(allocation_error)?;
        let body_map = semantic
            .logical_arguments_v1(body)
            .map_err(allocation_error)?;
        // Both maps stay live and prepaid through the entire consumer callback.
        // They drop before the common scope performs its original-ledger cleanup.
        consume(&root_map, &body_map, budget)
    })
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_output_binding_mapping_v1_tests.rs"]
mod tests;
