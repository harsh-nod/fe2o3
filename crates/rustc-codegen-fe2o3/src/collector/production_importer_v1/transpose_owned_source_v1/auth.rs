use super::*;
use crate::collector::production_importer_v1::{
    execution_terminal_operation_v1,
    subgroup_partition_v1::workgroup_source_v1::{
        subgroup_workgroup_reference_source_v1, workgroup_epoch_projection_source_v1,
    },
};
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(super) fn roster<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    work: &mut usize,
) -> PlanResult<()> {
    for count in [
        auth.retained.type_producers().len(),
        auth.retained.function_producers().len(),
        auth.retained.function_abi_producers().len(),
        auth.retained.terminal_producers().len(),
        auth.semantic.callables().len(),
        auth.contexts.roots.len(),
    ] {
        bounded::charge(work, count)?;
    }
    if !auth
        .contexts
        .roots
        .iter()
        .any(|root| std::ptr::eq(root, auth.root))
    {
        return Err(
            Error::Source("transpose root is not owned by this source context receipt").into(),
        );
    }
    // Existing generic source-roster conversion (also used by reusable LDS):
    // compare full type/layout and ABI payloads, not just identity labels. Its
    // recursive converters keep their existing bounds. Do this once per plan.
    crate::collector::production_importer_v1::numerical_policy_v1::defined_source_roster_v1(
        tcx,
        auth.retained,
        auth.semantic.types(),
        auth.semantic.functions(),
        auth.semantic.callables(),
    )?;
    Ok(())
}

fn abi<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &'a Authentication<'_, 'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> Result<&'a SemanticFunctionAbiV1> {
    let identity = canonical_function_identities_v1(tcx, instance).function();
    let mut found = None;
    for function in auth.semantic.functions() {
        bounded::charge(work, 1)?;
        if function.identity() == identity && found.replace(function.abi()).is_some() {
            return Err(Error::Source("ambiguous source function ABI"));
        }
    }
    for callable in auth.semantic.callables() {
        bounded::charge(work, 1)?;
        if let Some(binding) = callable.binding() {
            if binding.identity() == identity && found.replace(binding.abi()).is_some() {
                return Err(Error::Source("ambiguous source terminal ABI"));
            }
        }
    }
    found.ok_or(Error::Source("source instance has no admitted ABI"))
}

pub(super) fn terminal<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    recipe: &TerminalExpansionRecipeV1<'tcx>,
    expected: Terminal,
    work: &mut usize,
) -> Result<()> {
    if recipe.expansion != Expansion::Execution(expected) {
        return Err(Error::Source("transpose source terminal role changed"));
    }
    let producer = auth
        .retained
        .terminal_producers()
        .get(recipe.terminal as usize)
        .ok_or(Error::Source("source terminal outside retained roster"))?;
    if producer.instance != recipe.instance
        || producer.expansion != recipe.expansion
        || producer.identities != recipe.identities
    {
        return Err(Error::Source(
            "source terminal recipe and producer disagree",
        ));
    }
    let source_abi = abi(tcx, auth, recipe.instance, work)?;
    if source_abi.identity() != producer.abi.identity {
        return Err(Error::Source("source terminal ABI changed"));
    }
    // Reuse existing closed source/body/root validators. This child does not
    // introduce a second nominal or provider classifier.
    let expected_operation = execution_terminal_operation_v1(
        tcx,
        recipe.instance,
        expected,
        source_abi,
        auth.semantic.types(),
        Some(auth.root),
        recipe.identities.function(),
        auth.contexts,
    )
    .map_err(|_| Error::Source("transpose terminal lost closed source/body/root authentication"))?;
    let mut found = false;
    for callable in auth.semantic.callables() {
        bounded::charge(work, 1)?;
        if let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        {
            if binding.identity() == recipe.identities.function() {
                if found || operation != &expected_operation {
                    return Err(Error::Source(
                        "transpose terminal differs from replayed source contract",
                    ));
                }
                found = true;
            }
        }
    }
    if !found {
        return Err(Error::Source(
            "transpose source terminal is absent from canonical roster",
        ));
    }
    Ok(())
}

pub(super) fn shared_workgroup<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> Result<()> {
    let source_abi = abi(tcx, auth, instance, work)?;
    if trusted_device_items::classify(tcx, instance.def_id())
        == Some(TrustedDeviceItem::ExecutionSubgroupCurrent)
    {
        subgroup_workgroup_reference_source_v1(
            tcx,
            instance,
            source_abi,
            auth.semantic.types(),
            auth.root,
            auth.contexts,
        )
        .map_err(|_| Error::Source("Workgroup shared subgroup use failed source authentication"))?;
    } else {
        // This validator checks the exact reviewed epoch provider, body,
        // reference edge, ABI, root, brand and epoch. Unknown methods fail.
        workgroup_epoch_projection_source_v1(
            tcx,
            instance,
            source_abi,
            auth.semantic.types(),
            auth.root,
            auth.contexts,
        )
        .map_err(|_| Error::Source("Workgroup shared use is not an exact epoch projection"))?;
    }
    Ok(())
}
