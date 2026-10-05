//! Whole-roster source binding for the first explicitly bounded proof domain.
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18 as Correspondence;
use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1;
use std::fmt::Write as _;

// roots, active instances, source statements, assignments, target operations,
// target definitions. Every count describes the complete accepted request.
pub(crate) fn generate(
    relation: &Correspondence<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    let source = relation.source(out.budget)?;
    let inventory = relation.inventory(out.budget)?;
    let semantic = source.source_semantic(out.budget)?;
    let roots = source.root_count(out.budget)?;
    out.budget
        .charge_work(roots.checked_add(4).ok_or(Resource::Arithmetic)?)?;
    if roots == 0 || roots != semantic.roots().len() || roots != inventory.functions().len() {
        return Err(Error::Statement(
            "original MIR complete root/function roster is not modeled",
        ));
    }
    out.budget.reserve_storage(
        size_of::<[usize; 6]>()
            + size_of::<Vec<bool>>()
            + size_of::<Vec<relation::EndpointV30>>()
            + size_of::<Result<[usize; 6]>>()
            + size_of::<Result<Option<usize>>>()
            + 8 * size_of::<usize>(),
    )?;
    let mut seen = vector(inventory.functions().len(), out)?;
    seen.resize(inventory.functions().len(), false);
    let mut census = [
        roots,
        0,
        0,
        0,
        inventory.operations().len(),
        inventory.definitions().len(),
    ];
    write!(
        out,
        "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n"
    )
    .map_err(|_| out.error())?;
    relation::emit_prelude(out)?;
    for root in 0..roots {
        let (original, physical) = source.root(root, out.budget)?;
        let covered = seen
            .get_mut(physical)
            .ok_or(Error::Statement("original MIR physical root is absent"))?;
        if *covered || semantic.roots().get(root) != Some(&original) {
            return Err(Error::Statement(
                "original MIR root ordering or uniqueness differs",
            ));
        }
        *covered = true;
        // Calls need a separate source control/effect relation. Reject the
        // complete request, including inactive extra instances, until modeled.
        if source.instance_count(root, out.budget)? != 1
            || !source.instance_active(root, 0, out.budget)?
            || source.instance(root, 0, out.budget)? != (original, None)
        {
            return Err(Error::Statement(
                "original MIR invocation control is not modeled",
            ));
        }
        let function = semantic
            .functions()
            .get(original.index() as usize)
            .ok_or(Error::Statement("original MIR source function is absent"))?;
        let program = SourceProgramV30::derive(semantic.types(), function, out)?;
        let target = canonical::CanonicalProgramV30::derive(inventory, physical, out)?;
        let mut endpoints = vector(program.assignments.len(), out)?;
        for assignment in &program.assignments {
            let endpoint = relation.assignment_scalar_definition_v30(
                root,
                0,
                SemanticBlockIdV1::from_index(0),
                assignment.statement,
                out.budget,
            )?;
            if endpoints.len() == endpoints.capacity() {
                return Err(Resource::Accounting.into());
            }
            endpoints.push(match endpoint {
                Some(definition) => relation::EndpointV30::Definition(definition),
                None => relation::EndpointV30::Unit,
            });
        }
        relation::emit(&program, &target, &endpoints, root, out)?;
        for (index, amount) in [
            (1, 1),
            (2, program.statements),
            (3, program.assignments.len()),
        ] {
            census[index] = census[index]
                .checked_add(amount)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    out.budget.charge_work(seen.len())?;
    if seen.iter().any(|seen| !seen) {
        return Err(Error::Statement(
            "original MIR canonical function is unclaimed",
        ));
    }
    write!(out, "}}\n").map_err(|_| out.error())?;
    Ok(census)
}
