//! Same-source initial-value preservation, separate from loop control or bounds.
use super::*;

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(Clone))]
pub(in super::super) struct Proof {
    source_address: usize,
    function: SemanticFunctionIdentityV1,
    preheader: usize,
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    initialization: ScalarAssignmentSiteV1,
    ranked_initial: Option<ProductionRankedValueV1>,
}

impl Proof {
    pub(in super::super) fn initialization(&self) -> ScalarAssignmentSiteV1 {
        self.initialization
    }
}

fn exact_goto(
    function: &SemanticFunctionDeclV1,
    block: usize,
    header: usize,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(5)?;
    if !matches!(function.blocks().get(block).map(|block| block.terminator().kind()),
        Some(SemanticTerminatorKindV1::Goto(edge))
            if edge.role() == SemanticEdgeRoleV1::Goto
                && edge.target().index() as usize == header)
    {
        return Err(reject("distant initializer requires an exact Goto"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn discover(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    preheader: usize,
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<ScalarAssignmentSiteV1> {
    context.charge(24)?;
    let count = function.blocks().len();
    if count == 0
        || graph.entry != function.entry().index() as usize
        || graph.successors.len() != count
        || graph.predecessors.len() != count
        || graph.reachable.len() != count
        || header == graph.entry
        || preheader == header
        || preheader == latch
        || header == latch
        || [preheader, header, latch]
            .iter()
            .any(|&block| graph.reachable.get(block).copied() != Some(true))
    {
        return Err(reject("invalid distant initializer topology"));
    }
    exact_goto(function, preheader, header, context)?;
    exact_goto(function, latch, header, context)?;
    if graph.successors[preheader].as_slice() != [header]
        || graph.successors[latch].as_slice() != [header]
    {
        return Err(reject("distant initializer graph/control mismatch"));
    }
    let retained = context.scope.retained;
    let result = (|| {
        let mut scratch = Scratch::new(count, context)?;
        scratch.seed(graph.entry, context)?;
        while !scratch.pending.is_empty() {
            context.charge(3)?;
            let block = scratch
                .pending
                .pop()
                .ok_or_else(|| resource(Resource::Accounting))?;
            for &target in &graph.successors[block] {
                context.charge(2)?;
                if target != header {
                    scratch.seed(target, context)?;
                }
            }
        }
        context.reserve(std::mem::size_of::<Vec<bool>>())?;
        let mut outside = context.backing::<bool>(count)?;
        context.charge(count)?;
        outside.extend(scratch.seen.iter().map(|value| *value != 0));
        if !outside[preheader] || outside[latch] {
            return Err(reject("distant initializer entry/backedge mismatch"));
        }
        for &block in &graph.predecessors[header] {
            context.charge(3)?;
            if graph.reachable[block] && block != preheader && block != latch {
                return Err(reject(
                    "distant initializer has another reachable predecessor",
                ));
            }
        }
        let mut occurrences = 0;
        for (block, source) in function.blocks().iter().enumerate() {
            context.charge(3)?;
            if !outside[block] {
                continue;
            }
            let mut ordinal = 0;
            source
                .terminator()
                .kind()
                .try_for_each_edge(|edge| -> Result<()> {
                    context.charge(6)?;
                    if edge.target().index() as usize == header {
                        if block != preheader
                            || ordinal != 0
                            || edge.role() != SemanticEdgeRoleV1::Goto
                            || occurrences != 0
                        {
                            return Err(reject(
                                "distant initializer has another outside occurrence",
                            ));
                        }
                        occurrences = 1;
                    }
                    ordinal = sum(ordinal, 1)?;
                    Ok(())
                })?;
        }
        if occurrences != 1 {
            return Err(reject("distant initializer lacks its outside occurrence"));
        }
        let site = initialization(
            function,
            graph,
            &outside,
            header,
            latch,
            induction,
            &[preheader],
            &mut scratch,
            context,
        )?;
        context.charge(2)?;
        if site.block == preheader {
            return Err(reject("initializer is not distant"));
        }
        require_reinitialization(
            graph,
            header,
            site.block,
            &[preheader],
            &mut scratch,
            context,
        )?;
        Ok(site)
    })();
    // Closure-owned scratch has been dropped, including on a typed refusal.
    let temporary = context
        .scope
        .retained
        .checked_sub(retained)
        .ok_or_else(|| resource(Resource::Accounting))?;
    context.scope.release(context.facts, temporary)?;
    result
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn build(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    preheader: usize,
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<Box<Proof>> {
    let initialization = discover(
        function, graph, preheader, header, latch, induction, context,
    )?;
    context.charge(12)?;
    context.reserve(std::mem::size_of::<Proof>())?;
    Ok(Box::new(Proof {
        source_address: function as *const _ as usize,
        function: function.identity(),
        preheader,
        header,
        latch,
        induction,
        initialization,
        ranked_initial: None,
    }))
}

pub(in super::super) fn bind_initial(
    proof: &mut Proof,
    value: ProductionRankedValueV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(3)?;
    if proof.ranked_initial.is_some() {
        return Err(reject("distant initializer was bound twice"));
    }
    proof.ranked_initial = Some(value);
    Ok(())
}

pub(in super::super) fn before_emission(
    proof: &Proof,
    function: &SemanticFunctionDeclV1,
    row: &ProjectedUniformInductionV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(48)?;
    if proof.source_address != function as *const _ as usize
        || proof.function != function.identity()
        || proof.preheader != row.initializer_block
        || proof.header != row.header
        || proof.latch != row.latch
        || proof.induction != row.source_progress.induction
        || proof.ranked_initial != Some(row.initial)
        || [proof.preheader, proof.header, proof.latch].contains(&proof.initialization.block)
        || !matches!(&row.preheader_control,
            ProjectedInductionPreheaderControlV1::DistantDirect(actual)
                if std::ptr::eq(actual.as_ref(), proof))
    {
        return Err(reject("distant initializer lost source/row custody"));
    }
    let Some(SemanticStatementKindV1::Assign(assignment)) = function
        .blocks()
        .get(proof.initialization.block)
        .and_then(|block| block.statements().get(proof.initialization.statement))
        .map(|statement| statement.kind())
    else {
        return Err(reject("distant initializer site changed"));
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        return Err(reject("distant initializer is not a scalar use"));
    };
    let ty = function
        .locals()
        .get(proof.induction.index() as usize)
        .map(|local| local.ty())
        .ok_or_else(|| reject("distant initializer local changed"))?;
    if !assignment.destination().projections().is_empty()
        || assignment.destination().local() != proof.induction
        || assignment.destination().ty() != ty
        || assignment.value().result_type() != ty
        || operand.ty() != ty
        || row.source_progress.induction_type != ty
    {
        return Err(reject("distant initializer type/binding changed"));
    }
    exact_goto(function, proof.preheader, proof.header, context)?;
    exact_goto(function, proof.latch, proof.header, context)
}

pub(in super::super) fn replay(
    proof: &Proof,
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    row: &ProjectedUniformInductionV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    before_emission(proof, function, row, context)?;
    let site = discover(
        function,
        graph,
        proof.preheader,
        proof.header,
        proof.latch,
        proof.induction,
        context,
    )?;
    context.charge(2)?;
    if site != proof.initialization {
        return Err(reject("distant initializer replay changed its exact site"));
    }
    Ok(())
}
