use super::*;
use fe2o3_pliron::ProductionSemanticSsaSourceQueryV1;
use std::mem::size_of;

// Only this module can construct or read the classification. Its sole
// production query accepts endpoints, never a caller-supplied region mask.
pub(super) struct Index<'a> {
    body: &'a SemanticFunctionDeclV1,
    ssa: &'a SsaConstructionPlanV1,
    classification: Vec<u8>,
}

pub(super) fn loan_region<'a>(
    graph: &mut CapabilitySsaGraphV1<'a>,
    from: u32,
    to: u32,
) -> Result<Arc<CapabilityLoanRegionV1>, ProductionSemanticKirErrorV1> {
    // The second fixed unit pays source/index dispatch; the first remains
    // the original Arc clone. Prepay before even a cached geometry is used.
    graph.charge(lookup_work(graph.reuse.loan_regions.len()).saturating_add(2))?;
    let source = graph.reuse.definition_source;
    if let Some(source) = source {
        // Two source bindings, and up to two index bindings plus its length.
        graph.charge(5)?;
        if !std::ptr::eq(graph.body, source.function())
            || !std::ptr::eq(graph.ssa, source.plan().plan())
            || graph.reuse.endpoint_scc.as_ref().is_some_and(|index| {
                !std::ptr::eq(graph.body, index.body)
                    || !std::ptr::eq(graph.ssa, index.ssa)
                    || index.classification.len() != graph.body.blocks().len()
            })
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    } else if graph.reuse.endpoint_scc.is_some() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    if let Some(region) = graph.reuse.loan_regions.get(&(from, to)) {
        return Ok(Arc::clone(region));
    }
    let blocks = graph.path_region(from, to)?;
    let mut pending = None;
    let acyclic = if let Some(source) = source {
        graph.charge(1)?;
        if graph.reuse.endpoint_scc.is_none() {
            pending = Some(Index::build(graph, &source)?);
        }
        // R = {v: from reaches v and v reaches to} includes entire SCCs.
        // Reversing every edge preserves SCCs. Never use this test for an
        // arbitrary induced mask; that API still performs the original Kahn.
        let mut acyclic = true;
        for (block, &included) in blocks.iter().enumerate() {
            graph.charge(1)?;
            if included {
                graph.charge(1)?;
                let index = pending
                    .as_ref()
                    .or(graph.reuse.endpoint_scc.as_ref())
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                match index.classification[block] {
                    1 => {}
                    2 => acyclic = false,
                    _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                }
            }
        }
        acyclic
    } else {
        graph.region_is_acyclic_reusing_scratch(&blocks)?
    };
    let allocation_words = size_of::<CapabilityLoanRegionV1>()
        .div_ceil(size_of::<usize>())
        .saturating_add(2);
    graph.charge(
        insertion_work::<(u32, u32), Arc<CapabilityLoanRegionV1>>(graph.reuse.loan_regions.len())
            .saturating_add(allocation_words)
            .saturating_add(usize::from(pending.is_some())),
    )?;
    // No fallible work remains. Failed cold queries drop pending, even when
    // construction finished but a mask read or geometry publication failed.
    let region = Arc::new(CapabilityLoanRegionV1 { blocks, acyclic });
    if let Some(index) = pending {
        graph.reuse.endpoint_scc = Some(index);
    }
    graph
        .reuse
        .loan_regions
        .insert((from, to), Arc::clone(&region));
    Ok(region)
}

fn reserve<T>(
    graph: &mut CapabilitySsaGraphV1<'_>,
    count: usize,
    words: usize,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    graph.charge(count.saturating_mul(words))?;
    let result = Vec::with_capacity(count);
    graph.charge((result.capacity() - count).saturating_mul(words))?;
    Ok(result)
}

fn incoming_count(
    graph: &mut CapabilitySsaGraphV1<'_>,
    source: &ProductionSemanticSsaSourceQueryV1<'_>,
    block: u32,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut failure = None;
    let result =
        source.predecessor_count(SsaBlockIdV1::new(block), &mut || match graph.charge(1) {
            Ok(()) => true,
            Err(error) => {
                failure = Some(error);
                false
            }
        });
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}

fn incoming_source(
    graph: &mut CapabilitySsaGraphV1<'_>,
    source: &ProductionSemanticSsaSourceQueryV1<'_>,
    block: u32,
    ordinal: usize,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    let mut failure = None;
    let result = source.predecessor(
        SsaBlockIdV1::new(block),
        ordinal,
        &mut || match graph.charge(1) {
            Ok(()) => true,
            Err(error) => {
                failure = Some(error);
                false
            }
        },
    );
    if let Some(error) = failure {
        return Err(error);
    }
    let row = result
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let predecessor = row.id().source().get();
    if row.target() != SsaBlockIdV1::new(block)
        || predecessor as usize >= graph.body.blocks().len()
        || !graph.ssa.is_reachable(SsaBlockIdV1::new(predecessor))
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(predecessor)
}

impl<'a> Index<'a> {
    fn build(
        graph: &mut CapabilitySsaGraphV1<'a>,
        source: &ProductionSemanticSsaSourceQueryV1<'a>,
    ) -> Result<Box<Self>, ProductionSemanticKirErrorV1> {
        graph.charge(1)?;
        let count = graph.body.blocks().len();
        let ssa = graph.ssa;
        if count > u32::MAX as usize || ssa.reverse_postorder().len() > count {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let word = size_of::<usize>();
        graph.charge(size_of::<Self>().div_ceil(word))?;
        graph.charge(size_of::<Vec<u32>>().div_ceil(word))?;
        let mut classification = reserve::<u8>(graph, count, 1)?;
        let mut members = reserve::<u32>(graph, count, 1)?;
        graph.charge(count)?;
        classification.resize(count, 0);

        // Kosaraju's transpose pass. The checked planner's stored RPO is
        // decreasing DFS finish order (planner.rs::compute_reachability_and_order),
        // not a sort or a caller-supplied ordering. Replay compares the full
        // plan. Incoming rows belong to that same exact live semantic CFG.
        for root in ssa.reverse_postorder() {
            graph.charge(1)?;
            let root = root.get() as usize;
            if root >= count || !ssa.is_reachable(SsaBlockIdV1::new(root as u32)) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            if classification[root] != 0 {
                continue;
            }
            graph.charge(1)?;
            members.clear();
            graph.charge(2)?;
            classification[root] = 1;
            members.push(root as u32);
            let mut next = 0;
            let mut self_loop = false;
            while next < members.len() {
                graph.charge(1)?;
                let block = members[next];
                next += 1;
                let incoming = incoming_count(graph, source, block)?;
                for ordinal in 0..incoming {
                    // This visit handles the loop predicate, label lookup and
                    // self-loop test. Inventory access adds two API steps.
                    graph.charge(1)?;
                    let predecessor = incoming_source(graph, source, block, ordinal)?;
                    self_loop |= predecessor == block;
                    if classification[predecessor as usize] == 0 {
                        graph.charge(2)?;
                        classification[predecessor as usize] = 1;
                        members.push(predecessor);
                    }
                }
            }
            // All members are one SCC, not merely vertices reached by an
            // arbitrary reverse traversal: root order is essential here.
            graph.charge(1usize.saturating_add(members.len()))?;
            let label = if members.len() > 1 || self_loop { 2 } else { 1 };
            for &member in &members {
                classification[member as usize] = label;
            }
        }
        // Full domain coverage is checked, including excluded dead vertices.
        graph.charge(count)?;
        for (block, &label) in classification.iter().enumerate() {
            if (label != 0) != ssa.is_reachable(SsaBlockIdV1::new(block as u32)) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        graph.charge(2)?;
        Ok(Box::new(Self {
            body: graph.body,
            ssa,
            classification,
        }))
    }
}
