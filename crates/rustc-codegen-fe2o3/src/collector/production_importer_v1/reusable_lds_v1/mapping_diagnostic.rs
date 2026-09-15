use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticLocalRoleV1;

pub(super) fn report(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    functions: &[SemanticFunctionDeclV1],
    function: SemanticFunctionIdV1,
    stage: &'static str,
) {
    let (Some(semantic), Some(retained), Some(raw)) = (
        functions.get(function.index() as usize),
        plan.body_producers().get(function.index() as usize),
        plan.function_mir(function),
    ) else {
        return;
    };
    // Failure-only, fixed payloads and bounded scans. No admission decision uses this output.
    eprintln!(
        "[fe2o3-reusable-mapping-v1] stage={stage} function={function:?} identity={:?} entry={:?}/{:?} source_equal={} locals={}/{} blocks={}/{} raw_arguments={}",
        semantic.identity(),
        semantic.entry(),
        retained.entry,
        semantic.source() == retained.source.provenance,
        semantic.locals().len(),
        retained.locals.len(),
        semantic.blocks().len(),
        retained.blocks.len(),
        raw.arg_count
    );
    let mut remaining = 4;
    for (index, (local, binding)) in semantic
        .locals()
        .iter()
        .zip(&retained.locals)
        .take(256)
        .enumerate()
    {
        let role = match binding.rustc_local {
            0 => SemanticLocalRoleV1::Return,
            index if index as usize <= raw.arg_count => SemanticLocalRoleV1::Argument(index - 1),
            _ => SemanticLocalRoleV1::Temporary,
        };
        if local.identity() != binding.identity
            || local.ty() != binding.ty
            || local.source() != binding.source.provenance
            || local.role() != role
        {
            eprintln!(
                "[fe2o3-reusable-mapping-v1] local={index} raw={} identity_equal={} type={:?}/{:?} source_equal={} role={:?}/{role:?}",
                binding.rustc_local,
                local.identity() == binding.identity,
                local.ty(),
                binding.ty,
                local.source() == binding.source.provenance,
                local.role()
            );
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }
    let mut remaining = 4;
    for (index, (block, binding)) in semantic
        .blocks()
        .iter()
        .zip(&retained.blocks)
        .take(256)
        .enumerate()
    {
        let statement_source = block
            .statements()
            .iter()
            .zip(&binding.statements)
            .take(256)
            .position(|(statement, source)| statement.source() != source.provenance);
        if block.identity() != binding.identity
            || block.source() != binding.source.provenance
            || block.statements().len() != binding.statements.len()
            || statement_source.is_some()
            || block.terminator().source() != binding.terminator.provenance
        {
            eprintln!(
                "[fe2o3-reusable-mapping-v1] block={index} raw={} identity_equal={} source_equal={} statements={}/{} first_statement_source_mismatch={statement_source:?} terminator_source_equal={}",
                binding.rustc_block,
                block.identity() == binding.identity,
                block.source() == binding.source.provenance,
                block.statements().len(),
                binding.statements.len(),
                block.terminator().source() == binding.terminator.provenance
            );
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }
}
