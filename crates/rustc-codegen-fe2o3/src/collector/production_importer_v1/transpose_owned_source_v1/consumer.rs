//! Consumes the live source plan against an existing replayed SSA owner. These
//! source-custody events are not a Workgroup/epoch/physical LDS proof or a new
//! generic SSA definition. The common typed lowerer must discharge the WG uses.
use super::*;
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedRootV1,
    SemanticExpandedStatementOriginV1 as StatementOrigin,
    SemanticExpandedTerminatorOriginV1 as TerminatorOrigin, SsaBlockIdV1, SsaEdgeIdV1,
    SsaResolvedEventV1, SsaValueV1,
};
use fe2o3_pliron::{
    ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceQueryV1 as Query,
    ProductionSemanticSsaSourceOperandV1 as SourceOperand,
    ProductionSemanticSsaSourceSiteV1 as QuerySite, ProductionSemanticSsaSourceUseV1 as SourceUse,
};

#[path = "consumer/expanded.rs"]
mod expanded;
#[path = "consumer/execution_roster.rs"]
mod execution_roster;
use expanded::{call_block, source_statement, transfer_statement};

pub(in super::super) struct BoundPlan<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    root_identity: [u8; 32],
    expansion_identity: [u8; 32],
    flows: Vec<BoundFlow<'a>>,
}

/// A source-authorized use of an existing value, not a scalar SSA use invented
/// at a Constant operand. Each field remains tied to this exact source flow.
pub(super) struct OwnedInput<'a> {
    pub(super) site: QuerySite,
    pub(super) operand: &'a SemanticOperandV1,
    pub(super) value: SsaValueV1,
    pub(super) erased: bool,
}

pub(in super::super) struct BoundFlow<'a> {
    pub(super) source_binding: [u8; 32],
    pub(super) outer: SemanticCallInstanceIdV1,
    pub(super) execution_sites: [u32; 3],
    pub(super) issue_partition: SourceUse<'a>,
    pub(super) matrix_subgroup: SourceUse<'a>,
    pub(super) matrix_epoch: SourceUse<'a>,
    pub(super) capture: OwnedInput<'a>,
    pub(super) stage: OwnedInput<'a>,
    pub(super) closure_return: SourceUse<'a>,
    pub(super) helper_return: SourceUse<'a>,
    pub(super) publish: OwnedInput<'a>,
    pub(super) workgroup: SourceOperand<'a>,
    pub(super) workgroup_borrows: Vec<SourceUse<'a>>,
}

impl<'a> MappedPlan<'a> {
    pub(in super::super) fn bind(
        self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        work: &mut usize,
    ) -> PlanResult<BoundPlan<'a>> {
        if !std::ptr::eq(self.source, owner.source_semantic()) {
            return Err(Error::Source("transpose source and SSA owners differ").into());
        }
        owner.verify_replay().map_err(PlanError::Ssa)?;
        let view = owner
            .execution_view_for_root(self.root)
            .ok_or(Error::Source("transpose source root has no execution view"))?;
        let query = owner.source_query_for_root(self.root, view.body())?;
        let mut roster = execution_roster::expected(owner, view, work)?;
        let mut flows = Vec::new();
        for flow in self.flows {
            let mut count = 0usize;
            for origin in view.block_origins() {
                bounded::charge(work, 1)?;
                if (origin.function(), origin.block()) != flow.publish
                    || origin.terminator() != TerminatorOrigin::Source
                {
                    continue;
                }
                let bound = bind_flow(&query, view, &flow, origin.instance(), work)
                    .inspect_err(|_error| {
                        #[cfg(test)]
                        eprintln!(
                            "transpose-source-query-flow outer={} issue={:?} helper={:?} closure_call={:?} stage={:?} publish={:?}",
                            origin.instance().index(), flow.issue, flow.matrix_call,
                            flow.closure_call, flow.stage, flow.publish,
                        );
                    })?;
                let [issue, stage, publish] = bound.execution_sites;
                roster.consume_flow(issue, stage, publish, work)?;
                bounded::push(&mut flows, bound, work)?;
                count = count.checked_add(1).ok_or(Error::Work)?;
            }
            if count == 0 {
                return Err(Error::Source(
                    "transpose source flow has no retained execution instance",
                )
                .into());
            }
        }
        roster.finish(work)?;
        Ok(BoundPlan {
            owner,
            root: self.root,
            root_identity: *view.identity(),
            expansion_identity: *owner.execution_expansion().identity(),
            flows,
        })
    }
}

impl<'a> BoundPlan<'a> {
    /// The next typed consumer must check these same owner/expansion identities
    /// and all Workgroup loan endpoints before consuming the owned WG use.
    /// No API here labels that downstream obligation as already proved.
    pub(in super::super) fn into_flows(
        self,
        owner: &'a ProductionSemanticSsaOwnerV1,
    ) -> PlanResult<Vec<BoundFlow<'a>>> {
        let view = owner
            .execution_view_for_root(self.root)
            .ok_or(Error::Source("transpose bound root is absent"))?;
        if !std::ptr::eq(self.owner, owner)
            || self.root_identity != *view.identity()
            || self.expansion_identity != *owner.execution_expansion().identity()
        {
            return Err(Error::Source("transpose bound source owner changed").into());
        }
        Ok(self.flows)
    }
}

fn charge(work: &mut usize) -> bool {
    match work.checked_sub(1) {
        Some(next) => {
            *work = next;
            true
        }
        None => false,
    }
}

fn call<'a>(
    view: &'a SemanticExpandedRootV1,
    block: SemanticBlockIdV1,
) -> PlanResult<&'a SemanticDirectCallV1> {
    let block = view
        .body()
        .blocks()
        .get(block.index() as usize)
        .ok_or(Error::Source("transpose execution call block is absent"))?;
    match block.terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => Ok(call),
        _ => Err(Error::Source("transpose execution site is not the original call").into()),
    }
}

fn assignment<'a>(
    view: &'a SemanticExpandedRootV1,
    site: QuerySite,
) -> PlanResult<&'a SemanticAssignmentV1> {
    let statement = site
        .statement()
        .ok_or(Error::Source("transpose transfer lost its statement"))?;
    let statement = view
        .body()
        .blocks()
        .get(site.block().index() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .ok_or(Error::Source("transpose transfer statement absent"))?;
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => Ok(assignment),
        _ => Err(Error::Source("transpose transfer is not an assignment").into()),
    }
}

fn result(
    query: &Query<'_>,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    work: &mut usize,
) -> PlanResult<(SsaValueV1, SemanticTypeIdV1)> {
    let destination = call
        .destination()
        .ok_or(Error::Source("transpose call has no result"))?;
    let place = destination.place();
    if !place.projections().is_empty() {
        return Err(Error::Source("transpose result is not a whole original local").into());
    }
    let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(block.index()), 0);
    let definitions = query
        .plan()
        .plan()
        .edge_definitions(edge)
        .ok_or(Error::Source(
            "transpose original result lacks its call-return edge",
        ))?;
    let mut result = None;
    for definition in definitions {
        bounded::charge(work, 1)?;
        if definition.variable().get() == place.local().index()
            && result.replace(definition.value()).is_some()
        {
            return Err(
                Error::Source("transpose original result has duplicate edge definitions").into(),
            );
        }
    }
    Ok((
        result.ok_or(Error::Source(
            "transpose result lacks its actual SSA definition",
        ))?,
        place.ty(),
    ))
}

#[cfg_attr(test, track_caller)]
fn source_use<'a>(
    query: &Query<'a>,
    site: QuerySite,
    operand: &'a SemanticOperandV1,
    work: &mut usize,
) -> PlanResult<SourceUse<'a>> {
    let result = query.operand_use(site, operand, &mut || charge(work));
    #[cfg(test)]
    if let Err(error) = &result {
        let (kind, place) = match operand {
            SemanticOperandV1::Copy(place) => ("Copy", Some(place)),
            SemanticOperandV1::Move(place) => ("Move", Some(place)),
            SemanticOperandV1::Constant(_) => ("Constant", None),
        };
        let local = place.map(|place| place.local().index());
        // The planner collects promoted IDs by ascending input enumeration.
        let promoted = local.map(|local| {
            query.plan().plan().promoted_variables().binary_search(
                &fe2o3_mir_model::SsaVariableIdV1::new(local),
            ).is_ok()
        });
        let events = query.plan().plan().resolved_events(
            SsaBlockIdV1::new(site.block().index()),
        ).map(|events| events.len());
        let caller = std::panic::Location::caller();
        eprintln!(
            "transpose-source-query-failure root={} block={} statement={:?} kind={} local={:?} type={} projections={} promoted={:?} block_events={:?} remaining={} caller={}:{} error={error:?}",
            query.root().index(), site.block().index(), site.statement(), kind,
            local, operand.ty().index(), place.map_or(0, |place| place.projections().len()),
            promoted, events, *work, caller.file(), caller.line(),
        );
    }
    result.map_err(PlanError::Query)
}

fn owned_input<'a>(
    query: &Query<'a>,
    site: QuerySite,
    operand: &'a SemanticOperandV1,
    value: SsaValueV1,
    ty: SemanticTypeIdV1,
    work: &mut usize,
) -> PlanResult<OwnedInput<'a>> {
    bounded::charge(work, 1)?;
    if operand.ty() != ty {
        return Err(Error::Source("transpose source-owned input type changed").into());
    }
    let erased = match operand {
        SemanticOperandV1::Move(_) => {
            let use_ = source_use(query, site, operand, work)?;
            if use_.value() != value {
                return Err(
                    Error::Source("transpose explicit move changed its SSA producer").into(),
                );
            }
            false
        }
        SemanticOperandV1::Constant(constant)
            if matches!(constant.value(), SemanticConstantValueV1::ZeroSized) =>
        {
            true
        }
        _ => return Err(Error::Source("transpose owned source use changed operand kind").into()),
    };
    // This private function is reached only through a consumed, live-replayed
    // MappedPlan. It never returns an SSA definition for the erased operand.
    Ok(OwnedInput {
        site,
        operand,
        value,
        erased,
    })
}

fn return_value<'a>(
    query: &Query<'a>,
    view: &'a SemanticExpandedRootV1,
    site: QuerySite,
    expected: SsaValueV1,
    ty: SemanticTypeIdV1,
    work: &mut usize,
) -> PlanResult<(SourceUse<'a>, SsaValueV1)> {
    let assign = assignment(view, site)?;
    let SemanticRvalueKindV1::Use(operand @ SemanticOperandV1::Move(_)) = assign.value().kind()
    else {
        return Err(Error::Source("transpose original return is not a retained Move").into());
    };
    if !assign.destination().projections().is_empty()
        || assign.destination().ty() != ty
        || operand.ty() != ty
    {
        return Err(Error::Source("transpose return transfer type or projection changed").into());
    }
    let use_ = source_use(query, site, operand, work)?;
    if use_.value() != expected {
        return Err(Error::Source("transpose return transfer changed actual SSA input").into());
    }
    let block = SsaBlockIdV1::new(site.block().index());
    let events = query
        .plan()
        .plan()
        .resolved_events(block)
        .ok_or(Error::Source("transpose return has no resolved SSA events"))?;
    let mut value = None;
    let mut kill = 0usize;
    for (index, event) in events {
        bounded::charge(work, 1)?;
        if query.event_site(block, *index, &mut || charge(work))? != site {
            continue;
        }
        match event {
            SsaResolvedEventV1::Define {
                variable,
                value: next,
            } if variable.get() == assign.destination().local().index() => {
                if value.replace(*next).is_some() {
                    return Err(
                        Error::Source("transpose return has duplicate SSA definitions").into(),
                    );
                }
            }
            SsaResolvedEventV1::Kill {
                variable,
                previous: Some(previous),
            } if *variable == use_.variable() && *previous == expected => {
                kill += 1;
            }
            _ => {}
        }
    }
    if kill != 1 {
        return Err(Error::Source("transpose Move return lacks its exact SSA kill").into());
    }
    Ok((
        use_,
        value.ok_or(Error::Source(
            "transpose return lost its actual SSA definition",
        ))?,
    ))
}

fn bind_flow<'a>(
    query: &Query<'a>,
    view: &'a SemanticExpandedRootV1,
    flow: &MappedFlow<'a>,
    outer: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PlanResult<BoundFlow<'a>> {
    let issue_block = call_block(view, outer, flow.issue, None, work)?.0;
    let publish_block = call_block(view, outer, flow.publish, None, work)?.0;
    let (_, helper) = call_block(
        view,
        outer,
        flow.matrix_call,
        Some(flow.closure_call.0),
        work,
    )?;
    let helper = helper.ok_or(Error::Source("transpose helper has no expanded instance"))?;
    let (_, closure) = call_block(view, helper, flow.closure_call, Some(flow.stage.0), work)?;
    let closure = closure.ok_or(Error::Source("transpose closure has no expanded instance"))?;
    let stage_block = call_block(view, closure, flow.stage, None, work)?.0;
    let issue = call(view, issue_block)?;
    let stage = call(view, stage_block)?;
    let publish = call(view, publish_block)?;
    let [partition] = issue.arguments() else {
        return Err(Error::Source("transpose Issue changed its exact partition argument").into());
    };
    let issue_partition = source_use(
        query, QuerySite::new(issue_block, None), partition, work,
    )?;
    let (site, operand) = expanded::argument_operand(view, outer, flow.matrix_call, 0, 3, work)?;
    let matrix_subgroup = source_use(query, site, operand, work)?;
    let (site, operand) = expanded::argument_operand(view, outer, flow.matrix_call, 1, 3, work)?;
    let matrix_epoch = source_use(query, site, operand, work)?;
    let (issued_value, issued_ty) = result(query, issue_block, issue, work)?;
    let (staged_value, staged_ty) = result(query, stage_block, stage, work)?;
    let capture_site = source_statement(view, outer, flow.capture, work)?;
    let capture_assign = assignment(view, capture_site)?;
    let SemanticRvalueKindV1::Aggregate(aggregate) = capture_assign.value().kind() else {
        return Err(Error::Source("transpose capture lost its source aggregate").into());
    };
    let captured = aggregate
        .operands()
        .get(flow.capture_field as usize)
        .ok_or(Error::Source("transpose capture field is absent"))?;
    let capture = owned_input(query, capture_site, captured, issued_value, issued_ty, work)?;
    let stage = owned_input(
        query,
        QuerySite::new(stage_block, None),
        stage
            .arguments()
            .first()
            .ok_or(Error::Source("Stage source receiver absent"))?,
        issued_value,
        issued_ty,
        work,
    )?;
    let first = transfer_statement(view, helper, closure, flow.closure_call, work)?;
    let (closure_return, closure_value) =
        return_value(query, view, first, staged_value, staged_ty, work)?;
    let second = transfer_statement(view, outer, helper, flow.matrix_call, work)?;
    let (helper_return, helper_value) =
        return_value(query, view, second, closure_value, staged_ty, work)?;
    let [tile, workgroup] = publish.arguments() else {
        return Err(Error::Source("Publish source arity changed").into());
    };
    let publish = owned_input(
        query,
        QuerySite::new(publish_block, None),
        tile,
        helper_value,
        staged_ty,
        work,
    )?;
    let workgroup = query.operand_source(
        QuerySite::new(publish_block, None), workgroup, &mut || charge(work),
    )?;
    let local = view
        .local_origins()
        .get(workgroup.local().index() as usize)
        .ok_or(Error::Source("Publish Workgroup has no source local"))?;
    if local.instance() != outer
        || local.function() != flow.publish.0
        || local.local() != flow.workgroup_local
    {
        return Err(Error::Source("Publish Workgroup changed its exact source binding").into());
    }
    let mut borrows = Vec::new();
    bounded::reserve(&mut borrows, flow.workgroup_borrows.len(), work)?;
    for source in &flow.workgroup_borrows {
        let (site, operand) = expanded::borrow_operand(view, outer, *source, work)?;
        borrows.push(source_use(query, site, operand, work)?);
    }
    Ok(BoundFlow {
        source_binding: flow.source_binding,
        outer,
        execution_sites: [issue_block.index(), stage_block.index(), publish_block.index()],
        issue_partition,
        matrix_subgroup,
        matrix_epoch,
        capture,
        stage,
        closure_return,
        helper_return,
        publish,
        workgroup,
        workgroup_borrows: borrows,
    })
}
