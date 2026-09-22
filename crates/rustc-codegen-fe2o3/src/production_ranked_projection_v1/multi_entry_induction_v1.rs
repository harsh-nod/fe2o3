//! Source initialization and entry custody for the closed multi-entry recurrence.
use super::*;
#[cfg(test)]
#[path = "multi_entry_induction_v1_tests.rs"]
mod tests;
use super::induction_initialization_v1::*;
pub(super) use super::induction_initialization_v1::{Context, Scope, with_scope};
#[cfg(test)]
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;

type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

impl ProjectedInductionPreheaderControlV1 {
    pub(super) fn clone_single_v1(&self) -> Result<Self> {
        match self {
            Self::Direct => Ok(Self::Direct),
            Self::Optional {
                discriminant,
                explicit_value,
                explicit_target,
                otherwise,
            } => Ok(Self::Optional {
                discriminant: discriminant.clone(),
                explicit_value: *explicit_value,
                explicit_target: *explicit_target,
                otherwise: *otherwise,
            }),
            Self::Multiple(_) => Err(reject(
                "multi-entry induction cannot be copied as a historical single-entry proof",
            )),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(Clone))]
pub(super) struct Entries {
    // Same-call custody only: the actual immutable source outlives this
    // private row, which is dropped inside with_scope before returning a root.
    source_address: usize,
    function: SemanticFunctionIdentityV1,
    pub(super) initialization: ScalarAssignmentSiteV1,
    pub(super) header: usize,
    induction: SemanticLocalIdV1,
    ranked_initial: Option<ProductionRankedValueV1>,
    pub(super) rows: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) block: usize,
    successor: usize,
    role: SemanticEdgeRoleV1,
    target: usize,
}

impl Entries {
    pub(super) fn contains(&self, block: usize) -> bool {
        self.rows
            .binary_search_by_key(&block, |row| row.block)
            .is_ok()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    outside: &[bool],
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    entries: &[usize],
    context: &mut Context<'_, '_>,
) -> Result<Box<Entries>> {
    context.charge(8)?;
    if entries.len() < 2 || header == graph.entry || outside.len() != graph.successors.len() {
        return Err(reject(
            "multi-entry induction requires distinct outside entries and a non-entry header",
        ));
    }
    let mut previous = None;
    for &block in entries {
        context.charge(8)?;
        if previous.is_some_and(|previous| previous >= block)
            || graph.reachable.get(block).copied() != Some(true)
            || outside.get(block).copied() != Some(true)
            || graph.successors[block].as_slice() != [header]
            || !matches!(function.blocks()[block].terminator().kind(), SemanticTerminatorKindV1::Goto(edge)
                if edge.role() == SemanticEdgeRoleV1::Goto && edge.target().index() as usize == header)
        {
            return Err(reject(
                "multi-entry induction requires ordered exact outside Goto occurrences",
            ));
        }
        previous = Some(block);
    }
    context.reserve(std::mem::size_of::<Entries>())?;
    let mut rows = context.backing(entries.len())?;
    let mut scratch = Scratch::new(function.blocks().len(), context)?;
    let initialization = initialization(
        function,
        graph,
        outside,
        header,
        latch,
        induction,
        entries,
        &mut scratch,
        context,
    )?;
    let scratch_storage = scratch.retained()?;
    drop(scratch);
    context.scope.release(context.facts, scratch_storage)?;
    for &block in entries {
        context.charge(5)?;
        rows.push(Entry {
            block,
            successor: 0,
            role: SemanticEdgeRoleV1::Goto,
            target: header,
        });
    }
    context.charge(64)?;
    Ok(Box::new(Entries {
        source_address: function as *const _ as usize,
        function: function.identity(),
        initialization,
        header,
        induction,
        ranked_initial: None,
        rows,
    }))
}

pub(super) fn bind_initial(
    entries: &mut Entries,
    initial: ProductionRankedValueV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(3)?;
    if entries.ranked_initial.replace(initial).is_some() {
        return Err(reject("multi-entry initialization was bound twice"));
    }
    Ok(())
}

pub(super) fn prepay_header_entry_searches(
    predecessors: usize,
    entries: usize,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    let lookup = sum(4, usize::BITS as usize - entries.leading_zeros() as usize)?;
    context.charge(
        predecessors
            .checked_mul(lookup)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )
}

pub(super) fn check_roles(
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    for (ordinal, induction) in inductions.iter().enumerate() {
        context.charge(1)?;
        let ProjectedInductionPreheaderControlV1::Multiple(entries) = &induction.preheader_control
        else {
            continue;
        };
        for entry in &entries.rows {
            context.charge(1)?;
            for (other_ordinal, other) in inductions.iter().enumerate() {
                context.charge(5)?;
                if [other.initializer_block, other.header, other.latch].contains(&entry.block) {
                    return Err(reject(
                        "multi-entry induction has ambiguous entry, initializer, header or latch ownership",
                    ));
                }
                if other_ordinal == ordinal {
                    continue;
                }
                if let ProjectedInductionPreheaderControlV1::Multiple(other) =
                    &other.preheader_control
                {
                    context.charge(sum(
                        2,
                        usize::BITS as usize - other.rows.len().leading_zeros() as usize,
                    )?)?;
                    if other.contains(entry.block) {
                        return Err(reject("multi-entry inductions share an entry occurrence"));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn before_emission(
    function: &SemanticFunctionDeclV1,
    inductions: &[ProjectedUniformInductionV1],
    context: &mut Context<'_, '_>,
) -> Result<()> {
    check_roles(inductions, context)?;
    for induction in inductions {
        context.charge(1)?;
        let ProjectedInductionPreheaderControlV1::Multiple(entries) = &induction.preheader_control
        else {
            continue;
        };
        context.charge(50)?;
        if entries.source_address != function as *const _ as usize
            || entries.function != function.identity()
            || entries.header != induction.header
            || entries.induction != induction.source_progress.induction
            || entries.initialization.block != induction.initializer_block
            || entries.ranked_initial != Some(induction.initial)
            || !matches!(
                induction.source_progress.update,
                ProjectedSourceInductionUpdateV1::Ordinary
            )
        {
            return Err(reject(
                "multi-entry ranked initialization lost its source association",
            ));
        }
        let mut ordinal = 0;
        for (block, source) in function.blocks().iter().enumerate() {
            context.charge(sum(
                3,
                usize::BITS as usize - induction.loop_blocks.len().leading_zeros() as usize,
            )?)?;
            if induction.contains_block(block) {
                continue;
            }
            let mut successor = 0;
            source
                .terminator()
                .kind()
                .try_for_each_edge(|edge| -> Result<()> {
                    context.charge(4)?;
                    if edge.target().index() as usize == induction.header {
                        let expected = entries.rows.get(ordinal).ok_or_else(|| {
                            reject("multi-entry ranked source has an unlisted outside occurrence")
                        })?;
                        if *expected
                            != (Entry {
                                block,
                                successor,
                                role: edge.role(),
                                target: induction.header,
                            })
                        {
                            return Err(reject(
                                "multi-entry ranked source occurrence changed before emission",
                            ));
                        }
                        ordinal = sum(ordinal, 1)?;
                    }
                    successor = sum(successor, 1)?;
                    Ok(())
                })?;
        }
        if ordinal != entries.rows.len() {
            return Err(reject(
                "multi-entry ranked source omitted an outside occurrence",
            ));
        }
        let lookup = sum(
            4,
            usize::BITS as usize - entries.rows.len().leading_zeros() as usize,
        )?;
        context.charge(
            function
                .blocks()
                .len()
                .checked_mul(lookup)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
    }
    Ok(())
}

pub(super) fn replay(
    entries: &Entries,
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    header: usize,
    latch: usize,
    induction: SemanticLocalIdV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    context.charge(40)?;
    if entries.source_address != function as *const _ as usize
        || entries.function != function.identity()
        || entries.header != header
        || entries.induction != induction
    {
        return Err(reject(
            "multi-entry proof is attached to a different source recurrence",
        ));
    }
    let retained = context.scope.retained;
    let result = (|| {
        let count = function.blocks().len();
        let mut scratch = Scratch::new(count, context)?;
        if graph.entry != header {
            scratch.seed(graph.entry, context)?;
        }
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
        context.reserve(sum(
            std::mem::size_of::<Vec<bool>>(),
            std::mem::size_of::<Vec<usize>>(),
        )?)?;
        let mut outside = context.backing(count)?;
        context.charge(count)?;
        outside.extend(scratch.seen.iter().map(|value| *value != 0));
        let predecessors = graph
            .predecessors
            .get(header)
            .ok_or_else(|| reject("multi-entry header left the source graph"))?;
        let mut sources = context.backing(predecessors.len())?;
        for &source in predecessors {
            context.charge(3)?;
            if graph.reachable[source] && outside[source] {
                sources.push(source);
            }
        }
        let actual = build(
            function, graph, &outside, header, latch, induction, &sources, context,
        )?;
        context.charge(12)?;
        if actual.initialization != entries.initialization
            || actual.rows.len() != entries.rows.len()
        {
            return Err(reject(
                "multi-entry initializer or entry count changed before ranked emission",
            ));
        }
        for (actual, expected) in actual.rows.iter().zip(&entries.rows) {
            context.charge(5)?;
            if actual != expected {
                return Err(reject(
                    "multi-entry source occurrence order or identity changed",
                ));
            }
        }
        Ok(())
    })();
    let temporary = context
        .scope
        .retained
        .checked_sub(retained)
        .ok_or_else(|| resource(Resource::Accounting))?;
    context.scope.release(context.facts, temporary)?;
    result
}
