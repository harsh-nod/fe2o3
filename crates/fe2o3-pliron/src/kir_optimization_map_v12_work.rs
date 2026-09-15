//! Actual immutable-snapshot work, separate from the frozen observer growth cap.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplayCensusV12 {
    nodes: usize,
    events: usize,
    sources: usize,
    input_endpoints: usize,
    output_endpoints: usize,
    functions: usize,
    blocks: usize,
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or(KirOptimizationMapErrorV12::Arithmetic)
}

fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or(KirOptimizationMapErrorV12::Arithmetic)
}

// Only header lengths are read. Every structural traversal is prepaid before
// entering its iterator; result/argument vectors need no element traversal here.
fn endpoint_census(module: &Module, budget: &mut Budget<'_>) -> Result<(usize, usize)> {
    budget.charge_work(add(1, module.functions.len())?)?;
    let mut endpoints = 0;
    let mut blocks = 0;
    for function in &module.functions {
        let Some(body) = &function.body else { continue };
        budget.charge_work(add(1, body.blocks.len())?)?;
        blocks = add(blocks, body.blocks.len())?;
        endpoints = add(endpoints, body.parameters.len())?;
        for block in &body.blocks {
            budget.charge_work(add(1, block.operations.len())?)?;
            endpoints = add(endpoints, block.parameters.len())?;
            endpoints = add(endpoints, usize::from(block.terminator.is_some()))?;
            endpoints = add(endpoints, block.operations.len())?;
            for operation in &block.operations {
                endpoints = add(endpoints, operation.results.len())?;
            }
        }
    }
    Ok((endpoints, blocks))
}

impl ReplayCensusV12 {
    pub(super) fn derive(
        nodes: &[Node],
        events: &[Event],
        input: &Module,
        output: &Module,
        limits: CaptureLimitsV12,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        if nodes.len() > limits.nodes || events.len() > limits.events {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        budget.charge_work(add(1, nodes.len())?)?;
        let sources = nodes
            .iter()
            .filter(|node| matches!(node.input, Some(Endpoint::Operation(_))))
            .count();
        let (input_endpoints, input_blocks) = endpoint_census(input, budget)?;
        let (output_endpoints, output_blocks) = endpoint_census(output, budget)?;
        // The same immutable modules are traversed by check_inner. Reject growth
        // outside the original cap before either endpoint vector is allocated.
        if input_endpoints > limits.nodes || output_endpoints > limits.nodes {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        Ok(Self {
            nodes: nodes.len(),
            events: events.len(),
            sources,
            input_endpoints,
            output_endpoints,
            functions: add(input.functions.len(), output.functions.len())?,
            blocks: add(input_blocks, output_blocks)?,
        })
    }

    /// Conservative logical steps for one complete independent map check.
    /// N/E/S are actual nodes/events/input operations. Each source can visit
    /// N nodes and E replacement edges; the suffix cache only reduces that work.
    /// At most min(target_cap,S*N) targets are materialized. Explicit logarithms
    /// cover endpoint/row sorting and destination BTreeMap lookups. The 128-unit
    /// row envelope covers fixed-width comparisons, two adjacency scans, result
    /// edges, lifecycle validation, map comparisons and canonical digest bytes.
    pub(super) fn check_work(self, target_cap: usize) -> Result<usize> {
        let order = self
            .nodes
            .max(self.input_endpoints)
            .max(self.output_endpoints);
        let log = (usize::BITS - order.leading_zeros()) as usize;
        let targets = target_cap.min(mul(self.sources, self.nodes)?);
        let visits = mul(
            add(self.sources, 1)?,
            add(add(self.nodes, self.events)?, 1)?,
        )?;
        // Four N-sized orders cover raw input/terminal rows, source rows and
        // synthesized rows even when a malformed map fails endpoint coverage.
        let sorting = mul(
            add(
                mul(4, self.nodes)?,
                add(add(self.input_endpoints, self.output_endpoints)?, targets)?,
            )?,
            add(log, 1)?,
        )?;
        let rows = add(
            add(
                add(add(add(1, self.functions)?, self.blocks)?, self.nodes)?,
                self.events,
            )?,
            add(visits, sorting)?,
        )?;
        mul(128, rows)
    }

    /// Construction derives relations and hashes once; check_inner independently
    /// derives/hashes again. Keep both. Actual terminal-roster hash lookups are
    /// additionally bounded by L*(N+1), including worst-case key collisions.
    pub(super) fn finish_work(self, target_cap: usize, roster: usize) -> Result<usize> {
        add(
            mul(2, self.check_work(target_cap)?)?,
            mul(128, mul(roster, add(self.nodes, 1)?)?)?,
        )
    }
}
