use super::*;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Body, BorrowKind, Local, Location, Operand, Place, Rvalue,
    StatementKind, Terminator, TerminatorKind, UnwindAction,
    visit::{
        MutatingUseContext as Mut, NonMutatingUseContext as NonMut, NonUseContext, PlaceContext,
        Visitor,
    },
};
use rustc_span::{Span, Spanned};

fn body<'a, 'tcx>(
    retained: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    function: SemanticFunctionIdV1,
    work: &mut usize,
) -> Result<&'a Body<'tcx>> {
    let producer = retained
        .function_producers()
        .get(function.index() as usize)
        .ok_or(Error::Source(
            "owned source function is outside retained roster",
        ))?;
    let body = retained
        .function_mir(function)
        .ok_or(Error::Source("owned source lost exact retained MIR body"))?;
    bounded::charge(
        work,
        body.basic_blocks
            .len()
            .checked_add(body.local_decls.len())
            .ok_or(Error::Work)?,
    )?;
    if body.source.instance != producer.instance.def
        || body.source.promoted.is_some()
        || body.phase != mir::MirPhase::Runtime(mir::RuntimePhase::Optimized)
    {
        return Err(Error::Source(
            "owned source body is not the retained original optimized instance",
        ));
    }
    Ok(body)
}

fn callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    operand: &Operand<'tcx>,
) -> Result<Instance<'tcx>> {
    let Operand::Constant(constant) = operand else {
        return Err(Error::Source(
            "owned source call is not a concrete function",
        ));
    };
    let ty::FnDef(def, args) = *constant.const_.ty().kind() else {
        return Err(Error::Source("owned source call is not a FnDef"));
    };
    Instance::try_resolve(
        tcx,
        ty::TypingEnv::fully_monomorphized(),
        def,
        normalize(tcx, caller, args)?,
    )
    .ok()
    .flatten()
    .ok_or(Error::Source(
        "owned source callee lost monomorphic resolution",
    ))
}

pub(super) fn find_call<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    retained: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    function: SemanticFunctionIdV1,
    expected: Instance<'tcx>,
    span: Span,
    work: &mut usize,
) -> Result<(Site, &'a Terminator<'tcx>)> {
    let source = body(retained, function, work)?;
    let caller = retained.function_producers()[function.index() as usize].instance;
    let mut found = None;
    for (block, data) in source.basic_blocks.iter_enumerated() {
        bounded::charge(work, 1)?;
        let term = data.terminator();
        if let TerminatorKind::Call { func, args, .. } = &term.kind {
            bounded::charge(work, args.len())?;
            if term.source_info.span == span && callee(tcx, caller, func)? == expected {
                if data.is_cleanup || found.replace((Site { function, block }, term)).is_some() {
                    return Err(Error::Source(
                        "owned source call site is ambiguous or cleanup",
                    ));
                }
            }
        }
    }
    found.ok_or(Error::Source(
        "owned HIR call has no exact retained MIR occurrence",
    ))
}

struct Call<'a, 'tcx> {
    site: Site,
    args: &'a [Spanned<Operand<'tcx>>],
    destination: Local,
    target: BasicBlock,
}

fn call<'a, 'tcx>(site: Site, term: &'a Terminator<'tcx>, arity: usize) -> Result<Call<'a, 'tcx>> {
    let TerminatorKind::Call {
        args,
        destination,
        target: Some(target),
        unwind: UnwindAction::Unreachable,
        ..
    } = &term.kind
    else {
        return Err(Error::Source(
            "owned source call lost its exact normal/unwind edge",
        ));
    };
    if args.len() != arity || !destination.projection.is_empty() {
        return Err(Error::Source(
            "owned source call changed arity or destination",
        ));
    }
    Ok(Call {
        site,
        args,
        destination: destination.local,
        target: *target,
    })
}

fn recipe<'a, 'tcx>(
    auth: &'a Authentication<'_, 'tcx>,
    site: Site,
    expected: Instance<'tcx>,
    work: &mut usize,
) -> Result<&'a TerminalExpansionRecipeV1<'tcx>> {
    let mut found = None;
    for candidate in auth.retained.terminal_expansion_producers() {
        bounded::charge(work, 1)?;
        if candidate.caller == site.function && candidate.block as usize == site.block.index() {
            if candidate.instance != expected || found.replace(candidate).is_some() {
                return Err(Error::Source(
                    "owned source terminal recipe changed or duplicated",
                ));
            }
        }
    }
    found.ok_or(Error::Source(
        "owned source call has no retained terminal recipe",
    ))
}

fn direct_recipe(
    auth: &Authentication<'_, '_>,
    site: Site,
    expected: SemanticFunctionIdV1,
    work: &mut usize,
) -> Result<()> {
    let mut count = 0usize;
    for candidate in auth.retained.direct_call_producers() {
        bounded::charge(work, 1)?;
        if candidate.caller == site.function && candidate.block as usize == site.block.index() {
            if candidate.callee != expected {
                return Err(Error::Source("owned source helper call instance changed"));
            }
            count += 1;
        }
    }
    if count != 1 {
        return Err(Error::Source(
            "owned source helper recipe is missing or duplicated",
        ));
    }
    Ok(())
}

fn place_operand(operand: &Operand<'_>) -> Result<(Local, rules::LocalAction)> {
    match operand {
        Operand::Move(place) if place.projection.is_empty() => {
            Ok((place.local, rules::LocalAction::Move))
        }
        Operand::Copy(place) if place.projection.is_empty() => {
            Ok((place.local, rules::LocalAction::Copy))
        }
        Operand::Move(_) | Operand::Copy(_) | Operand::Constant(_) | Operand::RuntimeChecks(_) => {
            Err(Error::Source(
                "owned source operand is not an exact retained local",
            ))
        }
    }
}

fn exact_owned_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operand: &Operand<'tcx>,
    local: Local,
    ty: ty::Ty<'tcx>,
    span: Span,
) -> Result<bool> {
    match operand {
        Operand::Move(place) if place.local == local && place.projection.is_empty() => Ok(false),
        Operand::Constant(value)
            if normalize(tcx, instance, value.const_.ty())? == ty
                && normalize(tcx, instance, value.const_)?
                    .eval(tcx, ty::TypingEnv::fully_monomorphized(), span)
                    .ok()
                    == Some(mir::ConstValue::ZeroSized) =>
        {
            Ok(true)
        }
        _ => Err(Error::Source(
            "owned operand differs from its exact source Move/erasure",
        )),
    }
}

fn result_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    local: Local,
    expected: ty::Ty<'tcx>,
) -> Result<()> {
    if normalize(tcx, instance, body.local_decls[local].ty)? != expected {
        return Err(Error::Source("owned source result type changed"));
    }
    Ok(())
}

// A finite source-prefix shape check, not a replacement reachability/loan
// solver. Reject every bypass/backedge, including one from a dead block.
fn prefix(body: &Body<'_>, target: BasicBlock, work: &mut usize) -> Result<Vec<BasicBlock>> {
    let mut predecessors = Vec::new();
    bounded::reserve(&mut predecessors, body.basic_blocks.len(), work)?;
    predecessors.resize(body.basic_blocks.len(), (0usize, mir::START_BLOCK));
    for (from, data) in body.basic_blocks.iter_enumerated() {
        for successor in data.terminator().successors() {
            bounded::charge(work, 1)?;
            let incoming = predecessors
                .get_mut(successor.index())
                .ok_or(Error::Source("source CFG target outside retained body"))?;
            incoming.0 = incoming.0.checked_add(1).ok_or(Error::Work)?;
            incoming.1 = from;
        }
    }
    if predecessors[mir::START_BLOCK.index()].0 != 0 {
        return Err(Error::Source("owned source entry has a backedge"));
    }
    let mut path = Vec::new();
    let mut next = mir::START_BLOCK;
    for _ in 0..body.basic_blocks.len() {
        bounded::charge(work, 1)?;
        bounded::push(&mut path, next, work)?;
        if next == target {
            return Ok(path);
        }
        let after = match &body.basic_blocks[next].terminator().kind {
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::Call {
                target: Some(target),
                unwind: UnwindAction::Unreachable,
                ..
            } => *target,
            _ => {
                return Err(Error::Source(
                    "owned source prefix has an unmodeled branch or exit",
                ));
            }
        };
        if predecessors.get(after.index()) != Some(&(1, next)) {
            return Err(Error::Source("owned source prefix has a bypass or reentry"));
        }
        next = after;
    }
    Err(Error::Source("owned source prefix is cyclic or incomplete"))
}

struct LocalAudit<'a> {
    local: Local,
    field: Option<u32>,
    actions: Vec<(u32, usize, rules::LocalAction)>,
    work: &'a mut usize,
    error: Option<Error>,
}

impl LocalAudit<'_> {
    fn record(&mut self, location: Location, action: rules::LocalAction) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = bounded::push(
            &mut self.actions,
            (location.block.as_u32(), location.statement_index, action),
            self.work,
        ) {
            self.error = Some(error);
        }
    }
}

impl<'tcx> Visitor<'tcx> for LocalAudit<'_> {
    fn visit_ty(&mut self, _: ty::Ty<'tcx>, _: mir::visit::TyContext) {}
    fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: Location) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1) {
            self.error = Some(error);
            return;
        }
        self.super_statement(statement, location);
    }
    fn visit_terminator(&mut self, term: &Terminator<'tcx>, location: Location) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1) {
            self.error = Some(error);
            return;
        }
        self.super_terminator(term, location);
    }
    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1 + place.projection.len()) {
            self.error = Some(error);
            return;
        }
        if context == PlaceContext::NonUse(NonUseContext::VarDebugInfo) {
            return;
        }
        if place.local == self.local {
            let selected = match self.field {
                None => true,
                Some(field) => match place.projection.first() {
                    Some(mir::ProjectionElem::Field(actual, _)) => actual.as_u32() == field,
                    _ => true,
                },
            };
            let exact = match self.field {
                None => place.projection.is_empty(),
                Some(field) => matches!(place.as_ref().projection,
                    [mir::ProjectionElem::Field(actual, _)] if actual.as_u32() == field),
            };
            let action = if !exact {
                rules::LocalAction::Other
            } else {
                match context {
                    PlaceContext::MutatingUse(Mut::Store | Mut::Call) => rules::LocalAction::Define,
                    PlaceContext::NonMutatingUse(NonMut::Move) => rules::LocalAction::Move,
                    PlaceContext::NonMutatingUse(NonMut::Copy) => rules::LocalAction::Copy,
                    PlaceContext::NonMutatingUse(NonMut::SharedBorrow) => {
                        rules::LocalAction::SharedBorrow
                    }
                    _ => rules::LocalAction::Other,
                }
            };
            if selected {
                self.record(location, action);
            }
        }
        // Avoid double-counting the base via super_place, but retain every
        // projected index-local use. Other projections carry no local IDs.
        for projection in place.projection {
            if let mir::ProjectionElem::Index(index) = projection {
                if index == self.local {
                    self.record(location, rules::LocalAction::Other);
                }
            }
        }
    }
    fn visit_local(&mut self, local: Local, context: PlaceContext, location: Location) {
        if local == self.local && context != PlaceContext::NonUse(NonUseContext::VarDebugInfo) {
            let action =
                if self.field.is_none() && context == PlaceContext::NonMutatingUse(NonMut::Move) {
                    rules::LocalAction::Move
                } else {
                    rules::LocalAction::Other
                };
            self.record(location, action);
        }
    }
}

fn audit_local(
    body: &Body<'_>,
    local: Local,
    expected: &[(u32, usize, rules::LocalAction)],
    work: &mut usize,
) -> Result<()> {
    let mut audit = LocalAudit {
        local,
        field: None,
        actions: Vec::new(),
        work,
        error: None,
    };
    audit.visit_body(body);
    if let Some(error) = audit.error {
        return Err(error);
    }
    rules::exact_local_actions(audit.actions, expected, audit.work)
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    publish_recipe: &TerminalExpansionRecipeV1<'tcx>,
    source: &hir::Observed<'tcx>,
    work: &mut usize,
) -> Result<Coordinates> {
    let outer_id = publish_recipe.caller;
    let outer = body(auth.retained, outer_id, work)?;
    let (issue_site, issue_term) = find_call(
        tcx,
        auth.retained,
        outer_id,
        source.issue,
        source.issue_span,
        work,
    )?;
    let issue_recipe = recipe(auth, issue_site, source.issue, work)?;
    auth::terminal(
        tcx,
        auth,
        issue_recipe,
        Terminal::Gfx950TransposeIssue,
        work,
    )?;
    let issue = call(issue_site, issue_term, 1)?;
    result_type(
        tcx,
        source.outer,
        outer,
        issue.destination,
        source.issued_type,
    )?;
    let (helper_site, helper_term) = find_call(
        tcx,
        auth.retained,
        outer_id,
        source.helper,
        source.helper_span,
        work,
    )?;
    let helper_id = function_for_instance(auth.retained, source.helper, work)?;
    direct_recipe(auth, helper_site, helper_id, work)?;
    let helper = call(helper_site, helper_term, 3)?;
    result_type(
        tcx,
        source.outer,
        outer,
        helper.destination,
        source.staged_type,
    )?;
    let publish_site = Site {
        function: outer_id,
        block: BasicBlock::from_u32(publish_recipe.block),
    };
    let publish_term = outer
        .basic_blocks
        .get(publish_site.block)
        .ok_or(Error::Source("Publish raw block is absent"))?
        .terminator();
    let publish = call(publish_site, publish_term, 2)?;
    if publish_term.source_info.span != publish_recipe.span
        || callee(
            tcx,
            source.outer,
            match &publish_term.kind {
                TerminatorKind::Call { func, .. } => func,
                _ => return Err(Error::Source("Publish is not a retained call")),
            },
        )? != publish_recipe.instance
    {
        return Err(Error::Source(
            "Publish original callee/source occurrence changed",
        ));
    }
    let prefix = prefix(outer, helper.site.block, work)?;
    let mut issue_before = false;
    for block in &prefix {
        bounded::charge(work, 1)?;
        if *block == issue.site.block {
            issue_before = true;
        }
    }
    if !issue_before || issue.site.block == helper.site.block {
        return Err(Error::Source(
            "tile Issue does not precede its source capture/helper",
        ));
    }
    let mut incoming = Vec::new();
    for (from, data) in outer.basic_blocks.iter_enumerated() {
        for successor in data.terminator().successors() {
            bounded::charge(work, 1)?;
            if successor == publish.site.block {
                bounded::push(&mut incoming, from.as_u32(), work)?;
            }
        }
    }
    rules::immediate_normal_edge(
        helper.site.block.as_u32(),
        publish.site.block.as_u32(),
        helper.target.as_u32(),
        publish.target.as_u32(),
        outer.basic_blocks[publish.site.block].statements.len(),
        incoming,
        work,
    )?;
    let erased_publish = exact_owned_operand(
        tcx,
        source.outer,
        &publish.args[0].node,
        helper.destination,
        source.staged_type,
        publish_recipe.span,
    )?;
    let (environment, environment_action) = place_operand(&helper.args[2].node)?;
    if environment_action != rules::LocalAction::Move {
        return Err(Error::Source(
            "matrix helper does not move its original closure environment",
        ));
    }
    let (capture, erased_capture) = capture(
        tcx,
        outer,
        source,
        helper.site,
        environment,
        issue.destination,
        work,
    )?;
    let issue_end = outer.basic_blocks[issue.site.block].statements.len();
    let helper_end = outer.basic_blocks[helper.site.block].statements.len();
    let mut expected = Vec::new();
    bounded::reserve(&mut expected, 2, work)?;
    expected.push((
        issue.site.block.as_u32(),
        issue_end,
        rules::LocalAction::Define,
    ));
    if !erased_capture {
        bounded::push(
            &mut expected,
            (
                capture.0.block.as_u32(),
                capture.1,
                rules::LocalAction::Move,
            ),
            work,
        )?;
    }
    audit_local(outer, issue.destination, &expected, work)?;
    expected.clear();
    expected.push((
        helper.site.block.as_u32(),
        helper_end,
        rules::LocalAction::Define,
    ));
    if !erased_publish {
        bounded::push(
            &mut expected,
            (publish.site.block.as_u32(), 0, rules::LocalAction::Move),
            work,
        )?;
    }
    audit_local(outer, helper.destination, &expected, work)?;
    audit_local(
        outer,
        environment,
        &[
            (
                capture.0.block.as_u32(),
                capture.1,
                rules::LocalAction::Define,
            ),
            (
                helper.site.block.as_u32(),
                helper_end,
                rules::LocalAction::Move,
            ),
        ],
        work,
    )?;
    let (closure_call, stage) = helper_and_stage(tcx, auth, source, helper_id, work)?;
    let workgroup_local = workgroup(tcx, source, outer, publish, &prefix, work)?;
    Ok(Coordinates {
        issue: issue.site,
        capture,
        matrix_call: helper.site,
        closure_call,
        stage,
        publish: publish_site,
        workgroup_local,
    })
}

fn capture<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    source: &hir::Observed<'tcx>,
    helper: Site,
    environment: Local,
    issued: Local,
    work: &mut usize,
) -> Result<((Site, usize), bool)> {
    let mut found = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for (statement, item) in data.statements.iter().enumerate() {
            bounded::charge(work, 1)?;
            let StatementKind::Assign(assignment) = &item.kind else {
                continue;
            };
            let (destination, value) = &**assignment;
            if destination.local != environment {
                continue;
            }
            let Rvalue::Aggregate(kind, fields) = value else {
                return Err(Error::Source(
                    "closure environment is not its retained aggregate",
                ));
            };
            bounded::charge(work, fields.len())?;
            let AggregateKind::Closure(def, args) = **kind else {
                return Err(Error::Source("closure environment aggregate kind changed"));
            };
            if !destination.projection.is_empty()
                || block != helper.block
                || item.source_info.span != source.capture_span
                || def != source.closure.def_id()
                || normalize(tcx, source.outer, args)? != source.closure.args
            {
                return Err(Error::Source(
                    "closure capture source/body/instance changed",
                ));
            }
            let operand = fields
                .raw
                .get(source.nodes.capture_field as usize)
                .ok_or(Error::Source(
                    "tile capture field outside closure aggregate",
                ))?;
            let erased = exact_owned_operand(
                tcx,
                source.outer,
                operand,
                issued,
                source.issued_type,
                source.capture_span,
            )?;
            if found
                .replace((
                    (
                        Site {
                            function: helper.function,
                            block,
                        },
                        statement,
                    ),
                    erased,
                ))
                .is_some()
            {
                return Err(Error::Source(
                    "closure environment has multiple source definitions",
                ));
            }
        }
    }
    found.ok_or(Error::Source("closure source aggregate is absent"))
}

fn helper_and_stage<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    source: &hir::Observed<'tcx>,
    helper_id: SemanticFunctionIdV1,
    work: &mut usize,
) -> Result<(Site, Site)> {
    let helper = body(auth.retained, helper_id, work)?;
    let closure_id = function_for_instance(auth.retained, source.closure, work)?;
    let closure = body(auth.retained, closure_id, work)?;
    if helper.arg_count != 3 || helper.basic_blocks.len() != 3 {
        return Err(Error::Source(
            "matrix helper is not the retained three-argument single-call body",
        ));
    }
    let mut arguments = helper.args_iter();
    let subgroup = arguments
        .next()
        .ok_or(Error::Source("matrix helper lost subgroup argument"))?;
    let epoch = arguments
        .next()
        .ok_or(Error::Source("matrix helper lost epoch argument"))?;
    let environment = arguments
        .next()
        .ok_or(Error::Source("matrix helper lost closure argument"))?;
    let matrix_site = Site {
        function: helper_id,
        block: mir::START_BLOCK,
    };
    let matrix_term = helper.basic_blocks[matrix_site.block].terminator();
    let matrix = call(matrix_site, matrix_term, 2)?;
    let matrix_callee = match &matrix_term.kind {
        TerminatorKind::Call { func, .. } => callee(tcx, source.helper, func)?,
        _ => return Err(Error::Source("matrix helper lost its access call")),
    };
    let matrix_recipe = recipe(auth, matrix_site, matrix_callee, work)?;
    auth::terminal(tcx, auth, matrix_recipe, Terminal::MatrixAccess, work)?;
    if !helper.basic_blocks[matrix_site.block].statements.is_empty()
        || place_operand(&matrix.args[0].node)? != (subgroup, rules::LocalAction::Copy)
        || place_operand(&matrix.args[1].node)? != (epoch, rules::LocalAction::Copy)
    {
        return Err(Error::Source("matrix helper changed exact access inputs"));
    }
    let invoke_site = Site {
        function: helper_id,
        block: matrix.target,
    };
    let invoke_data = helper
        .basic_blocks
        .get(invoke_site.block)
        .ok_or(Error::Source("matrix helper invocation block is absent"))?;
    let invoke = call(invoke_site, invoke_data.terminator(), 2)?;
    let TerminatorKind::Call {
        func: Operand::Constant(func),
        ..
    } = &invoke_data.terminator().kind
    else {
        return Err(Error::Source(
            "matrix helper invocation is not its concrete FnOnce call",
        ));
    };
    let ty::FnDef(def, _) = *func.const_.ty().kind() else {
        return Err(Error::Source("matrix helper invocation is not a FnDef"));
    };
    if tcx.trait_of_assoc(def) != tcx.lang_items().fn_once_trait()
        || tcx.lang_items().fn_once_trait().is_none()
        || callee(tcx, source.helper, &Operand::Constant(func.clone()))? != source.closure
    {
        return Err(Error::Source(
            "matrix helper invokes a different closure or call trait",
        ));
    }
    direct_recipe(auth, invoke_site, closure_id, work)?;
    let (environment_local, environment_action) = place_operand(&invoke.args[0].node)?;
    let (tuple, tuple_action) = place_operand(&invoke.args[1].node)?;
    if environment_local != environment
        || tuple_action != rules::LocalAction::Move
        || invoke.destination != mir::RETURN_PLACE
    {
        return Err(Error::Source(
            "matrix helper invocation changed environment/tuple/result",
        ));
    }
    let after = helper
        .basic_blocks
        .get(invoke.target)
        .ok_or(Error::Source("matrix helper return block is absent"))?;
    if matrix.target == mir::START_BLOCK
        || invoke.target == matrix.target
        || invoke.target == mir::START_BLOCK
        || !after.statements.is_empty()
        || !matches!(after.terminator().kind, TerminatorKind::Return)
        || invoke_data.statements.len() != 3
        || invoke_data.is_cleanup
        || after.is_cleanup
    {
        return Err(Error::Source(
            "matrix helper changed original linear body/return",
        ));
    }
    let assignment = |index: usize| -> Result<&(Place<'tcx>, Rvalue<'tcx>)> {
        match &invoke_data.statements[index].kind {
            StatementKind::Assign(value) => Ok(&**value),
            _ => Err(Error::Source(
                "matrix helper introduced another statement kind",
            )),
        }
    };
    let (matrix_ref, matrix_value) = assignment(0)?;
    let (lane_ref, lane_value) = assignment(1)?;
    let (tuple_place, tuple_value) = assignment(2)?;
    if !matrix_ref.projection.is_empty()
        || !lane_ref.projection.is_empty()
        || tuple_place.as_local() != Some(tuple)
        || !matches!(matrix_value, Rvalue::Ref(_, BorrowKind::Shared, place)
            if place.as_local() == Some(matrix.destination))
        || !matches!(lane_value, Rvalue::Ref(_, BorrowKind::Shared, place)
            if place.local == subgroup && matches!(place.as_ref().projection,
                [mir::ProjectionElem::Deref, mir::ProjectionElem::Field(field, _)] if field.as_u32() == 0))
    {
        return Err(Error::Source(
            "matrix helper changed the exact matrix/lane shared references",
        ));
    }
    let Rvalue::Aggregate(kind, fields) = tuple_value else {
        return Err(Error::Source("matrix helper lost its argument tuple"));
    };
    if **kind != AggregateKind::Tuple
        || fields.len() != 2
        || place_operand(&fields.raw[0])? != (matrix_ref.local, rules::LocalAction::Copy)
        || place_operand(&fields.raw[1])? != (lane_ref.local, rules::LocalAction::Copy)
    {
        return Err(Error::Source(
            "matrix helper changed its exact closure argument tuple",
        ));
    }
    result_type(
        tcx,
        source.helper,
        helper,
        mir::RETURN_PLACE,
        source.staged_type,
    )?;
    audit_local(
        helper,
        environment,
        &[(invoke.site.block.as_u32(), 3, environment_action)],
        work,
    )?;
    audit_local(
        helper,
        mir::RETURN_PLACE,
        &[
            (invoke.site.block.as_u32(), 3, rules::LocalAction::Define),
            (invoke.target.as_u32(), 0, rules::LocalAction::Move),
        ],
        work,
    )?;
    let (stage_site, stage_term) = find_call(
        tcx,
        auth.retained,
        closure_id,
        source.stage,
        source.stage_span,
        work,
    )?;
    let stage = call(stage_site, stage_term, 4)?;
    let stage_recipe = recipe(auth, stage_site, source.stage, work)?;
    let stage_kind = match stage_recipe.expansion {
        Expansion::Execution(
            kind @ (Terminal::Gfx950TransposeStageB4 | Terminal::Gfx950TransposeStageB8),
        ) => kind,
        _ => {
            return Err(Error::Source(
                "source closure tail is not a closed Stage terminal",
            ));
        }
    };
    auth::terminal(tcx, auth, stage_recipe, stage_kind, work)?;
    result_type(
        tcx,
        source.closure,
        closure,
        stage.destination,
        source.staged_type,
    )?;
    if stage.destination != mir::RETURN_PLACE {
        return Err(Error::Source(
            "Stage does not define the original closure return place",
        ));
    }
    let return_data = closure
        .basic_blocks
        .get(stage.target)
        .ok_or(Error::Source("Stage closure return block is absent"))?;
    if return_data.is_cleanup
        || !return_data.statements.is_empty()
        || !matches!(return_data.terminator().kind, TerminatorKind::Return)
    {
        return Err(Error::Source(
            "Stage normal result does not directly return from its closure",
        ));
    }
    for (block, data) in closure.basic_blocks.iter_enumerated() {
        bounded::charge(work, 1)?;
        if matches!(data.terminator().kind, TerminatorKind::Return) && block != stage.target {
            return Err(Error::Source("Stage closure has another return producer"));
        }
        for successor in data.terminator().successors() {
            bounded::charge(work, 1)?;
            if successor == stage.target && block != stage.site.block {
                return Err(Error::Source(
                    "Stage result return has a bypass predecessor",
                ));
            }
        }
    }
    let closure_environment = closure.args_iter().next().ok_or(Error::Source(
        "Stage closure has no original environment argument",
    ))?;
    let environment_ty = normalize(
        tcx,
        source.closure,
        closure.local_decls[closure_environment].ty,
    )?;
    if !matches!(environment_ty.kind(), ty::Closure(def, args)
        if *def == source.closure.def_id() && *args == source.closure.args)
    {
        return Err(Error::Source(
            "Stage closure environment is not its owned source instance",
        ));
    }
    let stage_move = match &stage.args[0].node {
        Operand::Move(place)
            if place.local == closure_environment
                && matches!(place.as_ref().projection, [mir::ProjectionElem::Field(field, field_ty)]
                if field.as_u32() == source.nodes.capture_field
                    && normalize(tcx, source.closure, *field_ty)? == source.issued_type) =>
        {
            true
        }
        Operand::Constant(_) => {
            exact_owned_operand(
                tcx,
                source.closure,
                &stage.args[0].node,
                closure_environment,
                source.issued_type,
                source.stage_span,
            )?;
            false
        }
        _ => {
            return Err(Error::Source(
                "Stage receiver is not its exact owned capture/erasure",
            ));
        }
    };
    let stage_end = closure.basic_blocks[stage.site.block].statements.len();
    let mut field_audit = LocalAudit {
        local: closure_environment,
        field: Some(source.nodes.capture_field),
        actions: Vec::new(),
        work,
        error: None,
    };
    field_audit.visit_body(closure);
    if let Some(error) = field_audit.error {
        return Err(error);
    }
    let expected = [(
        stage.site.block.as_u32(),
        stage_end,
        rules::LocalAction::Move,
    )];
    rules::exact_local_actions(
        field_audit.actions,
        if stage_move { &expected } else { &[] },
        field_audit.work,
    )?;
    audit_local(
        closure,
        mir::RETURN_PLACE,
        &[
            (
                stage.site.block.as_u32(),
                stage_end,
                rules::LocalAction::Define,
            ),
            (stage.target.as_u32(), 0, rules::LocalAction::Move),
        ],
        work,
    )?;
    Ok((invoke_site, stage_site))
}

fn workgroup<'tcx>(
    tcx: TyCtxt<'tcx>,
    source: &hir::Observed<'tcx>,
    outer: &Body<'tcx>,
    publish: Call<'_, 'tcx>,
    prefix: &[BasicBlock],
    work: &mut usize,
) -> Result<Local> {
    let hir_body = tcx.hir_body_owned_by(source.nodes.outer);
    let environment_args =
        usize::from(tcx.def_kind(source.nodes.outer) == rustc_hir::def::DefKind::Closure);
    if outer.arg_count
        != hir_body
            .params
            .len()
            .checked_add(environment_args)
            .ok_or(Error::Work)?
    {
        return Err(Error::Source(
            "owned Workgroup source parameter ABI shape changed",
        ));
    }
    let raw_index = source
        .nodes
        .workgroup_parameter
        .checked_add(1 + environment_args)
        .ok_or(Error::Work)?;
    let local = outer
        .args_iter()
        .find(|local| local.index() == raw_index)
        .ok_or(Error::Source(
            "owned Workgroup source parameter has no exact MIR argument",
        ))?;
    result_type(tcx, source.outer, outer, local, source.workgroup_type)?;
    let (argument, action) = place_operand(&publish.args[1].node)?;
    if argument != local {
        return Err(Error::Source(
            "Publish consumes another Workgroup source local",
        ));
    }
    let mut expected = Vec::new();
    bounded::reserve(
        &mut expected,
        source
            .nodes
            .borrows
            .len()
            .checked_add(1)
            .ok_or(Error::Work)?,
        work,
    )?;
    expected.push((publish.site.block.as_u32(), 0, action));
    for borrow in &source.nodes.borrows {
        let mut before = false;
        for block in prefix {
            bounded::charge(work, 1)?;
            if borrow.site.block == *block {
                before = true;
            }
        }
        if borrow.site.function != publish.site.function || !before {
            return Err(Error::Source(
                "Workgroup shared use is outside its pre-Publish source prefix",
            ));
        }
        let data = &outer.basic_blocks[borrow.site.block];
        let borrow_call = call(borrow.site, data.terminator(), 1)?;
        let (reference, reference_action) = place_operand(&borrow_call.args[0].node)?;
        let mut definition = None;
        for (statement, item) in data.statements.iter().enumerate() {
            bounded::charge(work, 1)?;
            let StatementKind::Assign(assignment) = &item.kind else {
                continue;
            };
            let (destination, value) = &**assignment;
            if destination.as_local() == Some(reference) {
                if !matches!(value, Rvalue::Ref(_, BorrowKind::Shared, place) if place.as_local() == Some(local))
                    || definition.replace(statement).is_some()
                {
                    return Err(Error::Source(
                        "Workgroup receiver reference has another source definition",
                    ));
                }
            }
        }
        let definition = definition.ok_or(Error::Source(
            "Workgroup shared use has no exact retained borrow",
        ))?;
        expected.push((
            borrow.site.block.as_u32(),
            definition,
            rules::LocalAction::SharedBorrow,
        ));
        audit_local(
            outer,
            reference,
            &[
                (
                    borrow.site.block.as_u32(),
                    definition,
                    rules::LocalAction::Define,
                ),
                (
                    borrow.site.block.as_u32(),
                    data.statements.len(),
                    reference_action,
                ),
            ],
            work,
        )?;
        // Source identity and ABI were checked by the shared Workgroup source
        // validator. Lifetime/descendant-loan discharge remains the common
        // Workgroup endpoint's mandatory responsibility, never inferred here.
    }
    audit_local(outer, local, &expected, work)?;
    Ok(local)
}
