//! Exact physical forwarding between source selection cuts, not alias analysis.
use super::*;
use crate::production_semantic_kir_v1::origin_worklist_v1::{
    OriginStateV1, OriginWorkErrorV1, OriginWorkV1,
};

#[derive(Clone, Copy)]
struct ParentV30 {
    next: usize,
    step: SelectedFinalForwardingStepV30,
}

pub(super) struct ForwardingV30 {
    anchors: Vec<bool>,
    origins: Vec<OriginStateV1<usize>>,
    marks: Vec<usize>,
    parents: Vec<Option<ParentV30>>,
    queue: Vec<usize>,
    generation: usize,
}

fn work_error(error: OriginWorkErrorV1) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        OriginWorkErrorV1::Resource(error) => error.into(),
        OriginWorkErrorV1::Shape => {
            ProductionSourceOwnedViewErrorV18::Binding("selected final forwarding equation shape")
        }
    }
}

pub(super) fn definition_node(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    nodes: &[ActualNode],
    definition: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let row = optimized_source_definition_row_v18(optimized.checked.output(), definition, budget)?;
    let value = row.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "selected final forwarding definition has no value",
    ))?;
    charge_execution_cfg_lookup_v29(nodes.len(), budget).map_err(source_emission_error_v18)?;
    let at = nodes
        .binary_search_by_key(&value, |node| node.value)
        .map_err(|_| {
            ProductionSourceOwnedViewErrorV18::Binding(
                "selected final forwarding pointer is absent",
            )
        })?;
    budget.charge_work(1)?;
    if nodes[at].definition != definition {
        return resources::binding("selected final forwarding definition differs");
    }
    Ok(at)
}

impl ForwardingV30 {
    pub(super) fn build(
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        index: &index::ProjectionIndexV30<'_>,
        nodes: &[ActualNode],
        incoming: &[ActualIncoming],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        let mut anchors = resources::vector(nodes.len(), budget)?;
        budget.charge_work(nodes.len())?;
        anchors.resize(nodes.len(), false);
        // The retained transport stream emits each Node immediately after its
        // exact Definition projection. Check that pairing before using it.
        let rows = index.transport_rows();
        for (at, row) in rows.iter().enumerate() {
            budget.charge_work(2)?;
            let SelectedTransportRowV30::Node {
                ordinal, original, ..
            } = row
            else {
                continue;
            };
            if !matches!(
                original.step,
                SourceReferenceSelectionStepV29::Parameter { .. }
            ) {
                continue;
            }
            let Some(SelectedTransportRowV30::Definition {
                role: SelectedDefinitionRoleV30::Node(definition_ordinal),
                relation,
            }) = at.checked_sub(1).and_then(|at| rows.get(at))
            else {
                return resources::binding("selected final source parameter projection is absent");
            };
            if ordinal != definition_ordinal {
                return resources::binding("selected final source parameter projection differs");
            }
            let first = relation.outputs.start as usize;
            let end = argument_sum_v1(&[first, relation.outputs.len as usize])?;
            let outputs = optimized
                .checked
                .rows()
                .definition_outputs
                .get(first..end)
                .ok_or(ArgumentResourceV1::Accounting)?;
            for output in outputs {
                budget.charge_work(2)?;
                let at = definition_node(optimized, nodes, output.output, budget)?;
                if matches!(nodes[at].step, ActualStep::Parameter { .. }) {
                    anchors[at] = true;
                }
            }
        }
        Self::derive(nodes, incoming, anchors, budget)
    }

    fn derive(
        nodes: &[ActualNode],
        incoming: &[ActualIncoming],
        mut anchors: Vec<bool>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        if anchors.len() != nodes.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut count = 0;
        for (at, node) in nodes.iter().enumerate() {
            budget.charge_work(3)?;
            if matches!(node.step, ActualStep::Formation { .. }) {
                anchors[at] = true;
            }
            if !anchors[at] {
                let edges = match node.step {
                    ActualStep::Cast { .. } => 1,
                    ActualStep::Parameter { count, .. } => count,
                    _ => 0,
                };
                count = argument_sum_v1(&[count, edges])?;
            }
        }
        let mut work = OriginWorkV1::new(nodes.len(), count, budget).map_err(work_error)?;
        for (at, node) in nodes.iter().enumerate() {
            budget.charge_work(2)?;
            let seed = if anchors[at] {
                OriginStateV1::Exact(at)
            } else {
                match node.step {
                    ActualStep::Cast { .. } | ActualStep::Parameter { count: 1.., .. } => {
                        OriginStateV1::Pending
                    }
                    _ => OriginStateV1::Unknown,
                }
            };
            work.seed_next(seed, budget).map_err(work_error)?;
        }
        for (at, node) in nodes.iter().enumerate() {
            budget.charge_work(2)?;
            if anchors[at] {
                continue;
            }
            match node.step {
                ActualStep::Cast { input } => {
                    work.add_link(input, at, budget).map_err(work_error)?;
                }
                ActualStep::Parameter { first, count } => {
                    let end = argument_sum_v1(&[first, count])?;
                    for edge in incoming
                        .get(first..end)
                        .ok_or(ArgumentResourceV1::Accounting)?
                    {
                        budget.charge_work(1)?;
                        if edge.parameter != at {
                            return resources::binding("selected final forwarding target differs");
                        }
                        // Unreachable and parallel edges remain equations too.
                        work.add_link(edge.argument, at, budget)
                            .map_err(work_error)?;
                    }
                }
                _ => (),
            }
        }
        let origins = work.solve(budget).map_err(work_error)?;
        let mut marks = resources::vector(nodes.len(), budget)?;
        let mut parents = resources::vector(nodes.len(), budget)?;
        let queue = resources::vector(nodes.len(), budget)?;
        budget.charge_work(argument_product_v1(2, nodes.len())?)?;
        marks.resize(nodes.len(), 0);
        parents.resize(nodes.len(), None);
        Ok(Self {
            anchors,
            origins,
            marks,
            parents,
            queue,
            generation: 0,
        })
    }

    fn origin(
        &self,
        at: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        budget.charge_work(1)?;
        match self.origins.get(at).ok_or(ArgumentResourceV1::Accounting)? {
            OriginStateV1::Exact(anchor) => Ok(Some(*anchor)),
            _ => Ok(None),
        }
    }

    pub(super) fn edge(
        &self,
        at: usize,
        target: usize,
        nodes: &[ActualNode],
        incoming: &[ActualIncoming],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        budget.charge_work(7)?;
        let edge = incoming.get(at).ok_or(ArgumentResourceV1::Accounting)?;
        let input = nodes
            .get(edge.argument)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let output = nodes.get(target).ok_or(ArgumentResourceV1::Accounting)?;
        let ActualStep::Parameter { first, count } = output.step else {
            return Ok(None);
        };
        let Definition::BlockArgument { argument, .. } = output.definition else {
            return Ok(None);
        };
        if self
            .anchors
            .get(target)
            .copied()
            .ok_or(ArgumentResourceV1::Accounting)?
            || at < first
            || at >= argument_sum_v1(&[first, count])?
            || edge.parameter != target
            || edge.occurrence.argument != argument
            || (input.scalar, input.space, input.access)
                != (output.scalar, output.space, output.access)
        {
            return Ok(None);
        }
        let left = self.origin(edge.argument, budget)?;
        let right = self.origin(target, budget)?;
        Ok(if left.is_some() && left == right {
            left
        } else {
            None
        })
    }

    fn add(
        &mut self,
        at: usize,
        parent: Option<ParentV30>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(3)?;
        let mark = self
            .marks
            .get_mut(at)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if *mark != self.generation {
            if self.queue.len() == self.queue.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            *mark = self.generation;
            self.parents[at] = parent;
            self.queue.push(at);
        }
        Ok(())
    }

    // Search exact pointer dependencies backwards, stopping at source cuts.
    // Equal anchor labels alone do not connect sibling source paths.
    pub(super) fn path(
        &mut self,
        start: usize,
        end: usize,
        anchor: usize,
        initial: SelectedFinalForwardingStepV30,
        nodes: &[ActualNode],
        incoming: &[ActualIncoming],
        output: &mut Vec<SelectedFinalForwardingStepV30>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        let SelectedFinalForwardingStepV30::Incoming { actual } = initial else {
            return Ok(false);
        };
        if incoming.get(actual).map(|edge| edge.parameter) != Some(start)
            || self.edge(actual, start, nodes, incoming, budget)? != Some(anchor)
        {
            return Ok(false);
        }
        if self.origin(start, budget)? != Some(anchor) || self.origin(end, budget)? != Some(anchor)
        {
            return Ok(false);
        }
        budget.charge_work(self.queue.len())?;
        self.queue.clear();
        self.generation = argument_sum_v1(&[self.generation, 1])?;
        self.add(end, None, budget)?;
        let mut next = 0;
        let mut found = false;
        while let Some(&at) = self.queue.get(next) {
            next = argument_sum_v1(&[next, 1])?;
            budget.charge_work(2)?;
            if at == start {
                found = true;
                break;
            }
            if self.anchors[at] {
                continue;
            }
            match nodes[at].step {
                ActualStep::Cast { input } => self.add(
                    input,
                    Some(ParentV30 {
                        next: at,
                        step: SelectedFinalForwardingStepV30::Cast { input, target: at },
                    }),
                    budget,
                )?,
                ActualStep::Parameter { first, count } => {
                    let end = argument_sum_v1(&[first, count])?;
                    for edge in first..end {
                        if self.edge(edge, at, nodes, incoming, budget)? != Some(anchor) {
                            return resources::binding(
                                "selected final forwarding path lost an equation",
                            );
                        }
                        self.add(
                            incoming[edge].argument,
                            Some(ParentV30 {
                                next: at,
                                step: SelectedFinalForwardingStepV30::Incoming { actual: edge },
                            }),
                            budget,
                        )?;
                    }
                }
                _ => return resources::binding("selected final unsupported forwarding path"),
            }
        }
        if !found {
            return Ok(false);
        }
        build::push(output, initial, budget)?;
        let mut at = start;
        while at != end {
            budget.charge_work(2)?;
            let parent = self.parents[at].ok_or(ArgumentResourceV1::Accounting)?;
            build::push(output, parent.step, budget)?;
            at = parent.next;
        }
        Ok(true)
    }
}

pub(super) fn headers() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ForwardingV30>()?,
        h::<OriginWorkV1<usize>>()?,
        size_of::<Result<OriginWorkV1<usize>, OriginWorkErrorV1>>(),
        size_of::<Result<Vec<OriginStateV1<usize>>, OriginWorkErrorV1>>(),
        argument_product_v1(3, size_of::<Result<(), OriginWorkErrorV1>>())?,
        h::<Result<usize, usize>>()?,
        h::<Vec<bool>>()?,
        h::<Vec<OriginStateV1<usize>>>()?,
        argument_product_v1(2, h::<Vec<usize>>()?)?,
        h::<Vec<Option<ParentV30>>>()?,
        h::<ParentV30>()?,
        h::<Option<ParentV30>>()?,
        h::<Option<usize>>()?,
        h::<SelectedFinalForwardingStepV30>()?,
        h::<Vec<SelectedFinalForwardingStepV30>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<&[SelectedTransportRowV30]>()?,
        h::<Option<&SelectedTransportRowV30>>()?,
        h::<&[ActualNode]>()?,
        h::<&[ActualIncoming]>()?,
        h::<ActualNode>()?,
        h::<ActualIncoming>()?,
        h::<OriginStateV1<usize>>()?,
        h::<Definition>()?,
        h::<[usize; 16]>()?,
        h::<[bool; 4]>()?,
        h::<()>()?,
    ])
}

#[cfg(test)]
#[path = "production_selected_final_forwarding_v30_tests.rs"]
mod tests;
