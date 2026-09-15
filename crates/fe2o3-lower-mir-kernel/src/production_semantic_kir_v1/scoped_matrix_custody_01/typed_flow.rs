use super::super::borrowed_workgroup_01::{
    WorkgroupSourceResolverV1, checked_execution_source_call_v1,
};
use super::resolve::{Requirement, Resolver, Role, aggregate_fields, shared_pointee};
use super::typed_uses::ProductionTransposeOwnedSourceUsesV1;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticGfx950TransposeOperationV1 as T;

pub(super) fn check<'a>(
    session: &mut ProductionScopedMatrixSourceSessionV1<'a>,
    uses: &ProductionTransposeOwnedSourceUsesV1<'a, '_>,
    old_uses: &mut Option<super::old_epoch::Index<'a>>,
    matrix_sites: &mut Option<super::typed_inventory::MatrixSites<'a>>,
) -> Result<()> {
    let owner = session.owner;
    let view = session.view;
    let (row, helper) = super::typed_sites::pair(owner, view, &mut session.graph, uses)?;
    let issue_block = uses.partition.site().block().index();
    let publish_block = uses.workgroup.site().block().index();
    let (_, issue) = checked_execution_source_call_v1(owner, view, &session.context, issue_block)?;
    let (publish_call, publish) =
        checked_execution_source_call_v1(owner, view, &session.context, publish_block)?;
    let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(i) = issue.operation() else {
        return Err(mismatch());
    };
    let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(p) = publish.operation() else {
        return Err(mismatch());
    };
    let T::Issue {
        partition_reference,
        partition,
        tile: issued_tile,
    } = i.operation()
    else {
        return Err(mismatch());
    };
    let T::Publish {
        input_tile,
        input_workgroup,
        ..
    } = p.operation()
    else {
        return Err(mismatch());
    };
    if uses.partition.operand().ty() != partition_reference
        || shared_pointee(owner.source_semantic().types(), partition_reference) != Some(partition)
        || uses.workgroup.operand().ty() != input_workgroup
        || i.format() != p.format()
        || i.subgroup_brand() != p.subgroup_brand()
        || issue.provenance() != publish.provenance()
        || issue.workgroup_brand() != publish.workgroup_brand()
        || issue.epoch_before() != publish.epoch_before()
        || issue.epoch_after().is_some()
        || publish.epoch_after().is_none()
    {
        return Err(reject(
            "transpose typed Issue/Publish changed its exact root, types or epoch",
        ));
    }
    let mut resolver = Resolver {
        owner,
        view,
        context: &session.context,
        graph: &mut session.graph,
        occurrences: &session.occurrences,
        epochs: &session.epochs,
        requirement: Requirement::Transpose(issue),
    };
    let partition_origin =
        resolver.operand(issue_block, uses.partition.operand(), Role::Partition)?;
    resolver.live(&partition_origin, point(uses.partition))?;
    let subgroup_origin = resolver.operand(
        uses.subgroup.site().block().index(),
        uses.subgroup.operand(),
        Role::Subgroup,
    )?;
    resolver.live(&subgroup_origin, point(uses.subgroup))?;
    let partition_workgroup = partition_origin.workgroup.as_ref().ok_or_else(mismatch)?;
    let subgroup_workgroup = subgroup_origin.workgroup.as_ref().ok_or_else(mismatch)?;
    if partition_origin.subgroup != subgroup_origin.subgroup
        || partition_origin.context != subgroup_origin.context
        || partition_workgroup.issuer != subgroup_workgroup.issuer
        || partition_workgroup.contract != subgroup_workgroup.contract
    {
        return Err(reject(
            "transpose partition and matrix helper changed their actual subgroup or Context issuer",
        ));
    }
    let subgroup_value = subgroup_origin.subgroup.ok_or_else(mismatch)?;
    let subgroup_site = session.graph.definition(subgroup_value)?;
    let (_, subgroup_contract) =
        checked_execution_source_call_v1(owner, view, &session.context, subgroup_site.block)?;
    let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
        subgroup,
        workgroup,
        ..
    } = subgroup_contract.operation()
    else {
        return Err(mismatch());
    };
    if workgroup != input_workgroup {
        return Err(mismatch());
    }
    session.graph.charge(partition_workgroup.loans.len() + 8)?;
    let mut workgroup_origin = partition_workgroup.clone();
    let mut source = WorkgroupSourceResolverV1 {
        owner,
        view,
        context: &session.context,
        graph: &mut session.graph,
        epochs: &session.epochs,
    };
    let epoch = source.resolve_operand(
        uses.epoch.site().block().index(),
        uses.epoch.operand(),
        &[],
        subgroup_contract,
        0,
    )?;
    source.check_live(&epoch, uses.epoch.site().block().index())?;
    if !epoch.epoch_projection || epoch.issuer != workgroup_origin.issuer {
        return Err(reject(
            "transpose matrix helper epoch changed its Workgroup issuer",
        ));
    }
    for borrow in uses.borrows {
        let origin = source.resolve_operand(
            borrow.site().block().index(),
            borrow.operand(),
            &[],
            subgroup_contract,
            0,
        )?;
        source.check_live(&origin, borrow.site().block().index())?;
        if origin.issuer != workgroup_origin.issuer
            || origin.contract != workgroup_origin.contract
            || origin.loans.is_empty()
        {
            return Err(reject(
                "transpose shared-use roster contains another Workgroup origin",
            ));
        }
        source.graph.charge(origin.loans.len())?;
        workgroup_origin.loans.extend(origin.loans);
    }
    source.before_transpose_publish(&workgroup_origin, publish_block, subgroup_site.block)?;

    let issuer_site = session.graph.definition(workgroup_origin.issuer)?;
    let (issuer, issuer_contract) =
        checked_execution_source_call_v1(owner, view, &session.context, issuer_site.block)?;
    let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { context, .. } =
        issuer_contract.operation()
    else {
        return Err(mismatch());
    };
    let [context_operand] = issuer.arguments() else {
        return Err(mismatch());
    };
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = context_operand else {
        return Err(mismatch());
    };
    if !place.projections().is_empty() || place.ty() != context {
        return Err(mismatch());
    }
    let value = session
        .graph
        .use_value(issuer_site.block, place.local().index())?;
    let context_origin = super::super::numerical_policy_math_custody_01::checked_context_source_v1(
        owner,
        &session.context,
        &mut session.graph,
        issuer_contract.provenance(),
        value,
        (context != session.context.semantic_type).then_some(context),
    )?;
    if context_origin.context != subgroup_origin.context {
        return Err(mismatch());
    }
    let before = Site {
        block: publish_block,
        statement: Some(
            u32::try_from(
                view.body().blocks()[publish_block as usize]
                    .statements()
                    .len(),
            )
            .map_err(|_| mismatch())?,
        ),
        local: 0,
    };
    for loan in context_origin.loans {
        session.graph.loan_live(loan, before)?;
    }

    // The helper's actual MatrixAccess must use the same subgroup and epoch,
    // not just have a source-compatible function signature.
    let mut matrix_type = None;
    if matrix_sites.is_none() {
        *matrix_sites = Some(super::typed_inventory::MatrixSites::new(
            owner, view, &mut session.graph,
        )?);
    }
    for block in matrix_sites.as_ref().ok_or_else(mismatch)?
        .blocks(owner, view, helper, &mut session.graph)?
    {
        session.graph.charge(1)?;
        let (call, record) =
            checked_execution_source_call_v1(owner, view, &session.context, block as u32)?;
        let SemanticExecutionCapabilityOperationV1::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } = record.operation()
        else {
            return Err(mismatch());
        };
        if matrix_type.replace(matrix).is_some()
            || subgroup_brand != i.subgroup_brand()
            || width != 64
            || record.provenance() != issue.provenance()
            || record.workgroup_brand() != issue.workgroup_brand()
            || record.epoch_before() != issue.epoch_before()
            || record.epoch_after().is_some()
        {
            return Err(reject(
                "transpose helper changed its exact MatrixAccess occurrence",
            ));
        }
        let [subgroup_arg, epoch_arg] = call.arguments() else {
            return Err(mismatch());
        };
        let mut resolver = Resolver {
            owner,
            view,
            context: &session.context,
            graph: &mut session.graph,
            occurrences: &session.occurrences,
            epochs: &session.epochs,
            requirement: Requirement::Transpose(issue),
        };
        let actual = resolver.operand(block as u32, subgroup_arg, Role::Subgroup)?;
        resolver.live(
            &actual,
            Site {
                block: block as u32,
                statement: None,
                local: 0,
            },
        )?;
        if actual.subgroup != Some(subgroup_value) || actual.context != context_origin.context {
            return Err(mismatch());
        }
        let mut source = WorkgroupSourceResolverV1 {
            owner,
            view,
            context: &session.context,
            graph: &mut session.graph,
            epochs: &session.epochs,
        };
        let actual_epoch =
            source.resolve_operand(block as u32, epoch_arg, &[], subgroup_contract, 0)?;
        source.check_live(&actual_epoch, block as u32)?;
        if !actual_epoch.epoch_projection || actual_epoch.issuer != workgroup_origin.issuer {
            return Err(mismatch());
        }
    }
    let matrix = matrix_type
        .ok_or_else(|| reject("transpose helper lost its original MatrixAccess source call"))?;
    let epoch_type = shared_pointee(owner.source_semantic().types(), uses.epoch.operand().ty())
        .ok_or_else(mismatch)?;
    let lane = aggregate_fields(owner.source_semantic().types(), subgroup)
        .and_then(|fields| fields.first())
        .copied()
        .ok_or_else(mismatch)?;
    let next = publish_call
        .destination()
        .ok_or_else(mismatch)?
        .edge()
        .target()
        .index();
    let seeds = super::old_epoch_seeds::collect(
        owner,
        view,
        &session.context,
        &mut session.graph,
        issue,
        &[
            workgroup,
            subgroup,
            partition,
            epoch_type,
            lane,
            matrix,
            issued_tile,
            input_tile,
        ],
    )?;
    super::old_epoch::reject_uses(owner, view, &mut session.graph, next, &seeds, old_uses)?;
    // Keep the canonical caller/body role binding live until every typed check
    // has completed; no successful partial row is published on failure.
    if row.sites().publish.function != view.block_origins()[publish_block as usize].function() {
        return Err(mismatch());
    }
    Ok(())
}

fn point(use_: &fe2o3_pliron::ProductionSemanticSsaSourceUseV1<'_>) -> Site {
    Site {
        block: use_.site().block().index(),
        statement: use_.site().statement(),
        local: use_.variable().get(),
    }
}
