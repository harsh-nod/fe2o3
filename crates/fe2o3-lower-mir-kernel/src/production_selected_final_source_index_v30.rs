//! Paid projection indexes preserve all logical roles of merged definitions.
use super::*;

type RoleKey = [usize; 3];

fn role_key(role: SelectedDefinitionRoleV30) -> RoleKey {
    match role {
        SelectedDefinitionRoleV30::AccessPointer => [0, 0, 0],
        SelectedDefinitionRoleV30::AccessValue => [1, 0, 0],
        SelectedDefinitionRoleV30::Node(node) => [2, node, 0],
        SelectedDefinitionRoleV30::Leaf { leaf, component } => [3, leaf, component],
        SelectedDefinitionRoleV30::Guard(guard) => [4, guard, 0],
        SelectedDefinitionRoleV30::GuardSelector(guard) => [5, guard, 0],
    }
}

#[derive(Clone, Copy)]
struct DefinitionProjectionV30 {
    access: usize,
    output: Definition,
    role: SelectedDefinitionRoleV30,
}

impl DefinitionProjectionV30 {
    fn key(&self) -> (usize, Definition, RoleKey) {
        (self.access, self.output, role_key(self.role))
    }
}

#[derive(Clone, Copy)]
pub(super) struct EdgeProjectionV30 {
    access: usize,
    output: EdgeArgument,
    pub(super) source: SourceEdgeV30,
    pub(super) relation: SelectedEdgeV30,
}

#[derive(Clone, Copy)]
pub(super) struct GuardProjectionV30 {
    access: usize,
    output: Edge,
    leaf: usize,
    pub(super) ordinal: usize,
    pub(super) selector: OutputUse,
}

pub(super) struct AccessIndexV30 {
    pub(super) disposition: ProductionOptimizedSourceOperationV18,
    pub(super) pointer: Option<Definition>,
    pub(super) value: Option<Definition>,
    pub(super) rows: Range<usize>,
    pub(super) leaves: Range<usize>,
    pub(super) guards: Range<usize>,
    pub(super) obligations: Range<usize>,
}

pub(super) struct ProjectionIndexV30<'a> {
    rows: &'a [SelectedTransportRowV30],
    pub(super) accesses: Vec<AccessIndexV30>,
    definitions: Vec<DefinitionProjectionV30>,
    edges: Vec<EdgeProjectionV30>,
    source_edges: Vec<usize>,
    guards: Vec<GuardProjectionV30>,
    leaves: Vec<usize>,
    guard_rows: Vec<usize>,
    obligations: Vec<usize>,
    obligation_order: Vec<(usize, usize, usize)>,
}

fn sorted_work<T>(
    rows: &[T],
    width: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    call_splice_sort_work_v1(argument_product_v1(rows.len(), width)?, budget)
        .map_err(source_address_call_error_v29)
        .map_err(source_emission_error_v18)
}

impl<'a> ProjectionIndexV30<'a> {
    #[cfg(test)]
    pub(super) fn test_corrupt_v30(
        &mut self,
        fault: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        match fault {
            0 => {
                budget.charge_work(self.definitions.len())?;
                self.definitions
                    .retain(|row| !matches!(row.role, SelectedDefinitionRoleV30::Leaf { .. }));
            }
            1 => {
                budget.charge_work(self.edges.len())?;
                self.edges.clear();
            }
            2 => {
                budget.charge_work(self.edges.len())?;
                let edge = self
                    .edges
                    .iter_mut()
                    .find(|row| row.relation.output_incoming != row.relation.output_target)
                    .unwrap();
                edge.relation.output_incoming = edge.relation.output_target;
            }
            3 => {
                budget.charge_work(self.guards.len())?;
                self.guards.clear();
            }
            _ => panic!("unknown selected source index corruption"),
        }
        Ok(())
    }
    pub(super) fn build(
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        rows: &'a [SelectedTransportRowV30],
        sources: &[PendingSourceSelectedAccessV30],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        Self::build_from_facts(
            &SelectedFinalFactsV30::scalar(optimized),
            root,
            rows,
            sources,
            budget,
        )
    }

    pub(super) fn build_from_facts(
        facts: &SelectedFinalFactsV30<'_>,
        root: usize,
        rows: &'a [SelectedTransportRowV30],
        sources: &[PendingSourceSelectedAccessV30],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        // Count actual retained projections, never graph-limit products.
        let mut counts = [0usize; 7];
        for row in rows {
            budget.charge_work(3)?;
            let (slot, count) = match row {
                SelectedTransportRowV30::Definition { relation, .. } => {
                    (0, relation.outputs.len as usize)
                }
                SelectedTransportRowV30::Incoming { relation, .. }
                | SelectedTransportRowV30::Invocation { relation, .. } => {
                    (1, usize::from(relation.output.is_some()))
                }
                SelectedTransportRowV30::Guard { control, .. } => {
                    counts[6] = argument_sum_v1(&[counts[6], 1])?;
                    (
                        2,
                        usize::from(matches!(control.placement, EdgePlacement::Retained(_))),
                    )
                }
                SelectedTransportRowV30::Leaf { .. } => (3, 1),
                SelectedTransportRowV30::Obligation { .. } => (4, 1),
                SelectedTransportRowV30::Access { .. } => (5, 1),
                SelectedTransportRowV30::Node { .. } => continue,
            };
            counts[slot] = argument_sum_v1(&[counts[slot], count])?;
        }
        if counts[5] != sources.len() {
            return resources::binding("selected final original access census differs");
        }
        let mut index = Self {
            rows,
            accesses: resources::vector(counts[5], budget)?,
            definitions: resources::vector(counts[0], budget)?,
            edges: resources::vector(counts[1], budget)?,
            source_edges: resources::vector(counts[1], budget)?,
            guards: resources::vector(counts[2], budget)?,
            leaves: resources::vector(counts[3], budget)?,
            guard_rows: resources::vector(counts[6], budget)?,
            obligations: resources::vector(counts[4], budget)?,
            obligation_order: resources::vector(counts[4], budget)?,
        };
        for (row_index, row) in rows.iter().enumerate() {
            budget.charge_work(8)?;
            if let SelectedTransportRowV30::Access {
                root: actual_root,
                ordinal,
                instance,
                anchor,
                disposition,
                output_pointer,
                output_value,
            } = *row
            {
                if let Some(prior) = index.accesses.last_mut() {
                    prior.rows.end = row_index;
                    prior.leaves.end = index.leaves.len();
                    prior.guards.end = index.guard_rows.len();
                    prior.obligations.end = index.obligations.len();
                }
                let source =
                    sources
                        .get(ordinal)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected final source ordinal",
                        ))?;
                if actual_root != root
                    || ordinal != index.accesses.len()
                    || source.instance != instance
                    || source.anchor != anchor
                {
                    return resources::binding("selected final exact original access differs");
                }
                index.accesses.push(AccessIndexV30 {
                    disposition,
                    pointer: output_pointer,
                    value: output_value,
                    rows: row_index..row_index,
                    leaves: index.leaves.len()..index.leaves.len(),
                    guards: index.guard_rows.len()..index.guard_rows.len(),
                    obligations: index.obligations.len()..index.obligations.len(),
                });
                continue;
            }
            let access = index
                .accesses
                .len()
                .checked_sub(1)
                .ok_or(ArgumentResourceV1::Accounting)?;
            match *row {
                SelectedTransportRowV30::Definition { role, relation } => {
                    let first = relation.outputs.start as usize;
                    let end = argument_sum_v1(&[first, relation.outputs.len as usize])?;
                    for at in facts.definition_range(first..end)? {
                        budget.charge_work(3)?;
                        index.definitions.push(DefinitionProjectionV30 {
                            access,
                            output: facts.definition_output(at)?,
                            role,
                        });
                    }
                }
                SelectedTransportRowV30::Incoming {
                    ordinal, relation, ..
                } => {
                    if let Some(output) = relation.output {
                        index.edges.push(EdgeProjectionV30 {
                            access,
                            output,
                            source: SourceEdgeV30::Incoming(ordinal),
                            relation,
                        });
                    }
                }
                SelectedTransportRowV30::Invocation { node, relation, .. } => {
                    if let Some(output) = relation.output {
                        index.edges.push(EdgeProjectionV30 {
                            access,
                            output,
                            source: SourceEdgeV30::Invocation(node),
                            relation,
                        });
                    }
                }
                SelectedTransportRowV30::Guard {
                    ordinal,
                    original,
                    control,
                    output_selector,
                    ..
                } => {
                    index.guard_rows.push(row_index);
                    if let EdgePlacement::Retained(output) = control.placement {
                        let selector =
                            output_selector.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected final guard lost its selector",
                            ))?;
                        index.guards.push(GuardProjectionV30 {
                            access,
                            output,
                            leaf: original.leaf,
                            ordinal,
                            selector,
                        });
                    }
                }
                SelectedTransportRowV30::Leaf { .. } => index.leaves.push(row_index),
                SelectedTransportRowV30::Obligation {
                    ordinal, original, ..
                } => {
                    index.obligations.push(row_index);
                    index
                        .obligation_order
                        .push((access, original.leaf, ordinal));
                }
                SelectedTransportRowV30::Node { .. } => {}
                SelectedTransportRowV30::Access { .. } => unreachable!(),
            }
        }
        if let Some(last) = index.accesses.last_mut() {
            last.rows.end = rows.len();
            last.leaves.end = index.leaves.len();
            last.guards.end = index.guard_rows.len();
            last.obligations.end = index.obligations.len();
        }
        sorted_work(&index.definitions, 6, budget)?;
        index
            .definitions
            .sort_unstable_by_key(DefinitionProjectionV30::key);
        sorted_work(&index.edges, 7, budget)?;
        index
            .edges
            .sort_unstable_by_key(|row| (row.access, row.output, row.source));
        budget.charge_work(index.edges.len())?;
        index.source_edges.extend(0..index.edges.len());
        sorted_work(&index.source_edges, 3, budget)?;
        index.source_edges.sort_unstable_by_key(|&at| {
            let row = index.edges[at];
            (row.access, row.source)
        });
        budget.charge_work(index.source_edges.len())?;
        if index.source_edges.windows(2).any(|pair| {
            let left = index.edges[pair[0]];
            let right = index.edges[pair[1]];
            (left.access, left.source) == (right.access, right.source)
        }) {
            return resources::binding("selected final repeated source edge projection");
        }
        sorted_work(&index.guards, 6, budget)?;
        index
            .guards
            .sort_unstable_by_key(|row| (row.access, row.output, row.leaf, row.ordinal));
        sorted_work(&index.obligation_order, 3, budget)?;
        index.obligation_order.sort_unstable();
        budget.charge_work(argument_sum_v1(&[
            index.definitions.len(),
            index.edges.len(),
            index.guards.len(),
        ])?)?;
        if index
            .definitions
            .windows(2)
            .any(|p| p[0].key() == p[1].key())
            || index.edges.windows(2).any(|p| {
                (p[0].access, p[0].output, p[0].source) == (p[1].access, p[1].output, p[1].source)
            })
            || index.guards.windows(2).any(|p| {
                (p[0].access, p[0].output, p[0].leaf, p[0].ordinal)
                    == (p[1].access, p[1].output, p[1].leaf, p[1].ordinal)
            })
        {
            return resources::binding("selected final repeated exact projection row");
        }
        Ok(index)
    }

    pub(super) fn transport_rows(&self) -> &[SelectedTransportRowV30] {
        self.rows
    }

    pub(super) fn source_edge_projection(
        &self,
        access: usize,
        source: SourceEdgeV30,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&EdgeProjectionV30>> {
        if self.source_edges.len() != self.edges.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        charge_execution_cfg_lookup_v29(self.source_edges.len(), budget)
            .map_err(source_emission_error_v18)?;
        let found = self
            .source_edges
            .binary_search_by_key(&(access, source), |&at| {
                self.edges
                    .get(at)
                    .map(|row| (row.access, row.source))
                    .unwrap_or((usize::MAX, SourceEdgeV30::Invocation(usize::MAX)))
            });
        match found {
            Ok(at) => self
                .edges
                .get(self.source_edges[at])
                .map(Some)
                .ok_or(ArgumentResourceV1::Accounting.into()),
            Err(_) => Ok(None),
        }
    }

    pub(super) fn matches(
        &self,
        access: usize,
        role: SelectedDefinitionRoleV30,
        output: Definition,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        charge_execution_cfg_lookup_v29(self.definitions.len(), budget)
            .map_err(source_emission_error_v18)?;
        Ok(self
            .definitions
            .binary_search_by_key(
                &(access, output, role_key(role)),
                DefinitionProjectionV30::key,
            )
            .is_ok())
    }

    pub(super) fn leaf_candidates(
        &self,
        access: usize,
        pointer: Definition,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Range<usize>> {
        charge_execution_cfg_lookup_v29(self.definitions.len(), budget)
            .map_err(source_emission_error_v18)?;
        charge_execution_cfg_lookup_v29(self.definitions.len(), budget)
            .map_err(source_emission_error_v18)?;
        let first = self.definitions.partition_point(|row| {
            (row.access, row.output, role_key(row.role)[0]) < (access, pointer, 3)
        });
        let end = self.definitions.partition_point(|row| {
            (row.access, row.output, role_key(row.role)[0]) <= (access, pointer, 3)
        });
        Ok(first..end)
    }

    pub(super) fn candidate_leaf(
        &self,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, usize)> {
        budget.charge_work(1)?;
        let Some(DefinitionProjectionV30 {
            role: SelectedDefinitionRoleV30::Leaf { leaf, component },
            ..
        }) = self.definitions.get(index)
        else {
            return resources::binding("selected final leaf projection role");
        };
        Ok((*leaf, *component))
    }

    pub(super) fn leaf(
        &self,
        access: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<PendingSourceSelectedLeafV30> {
        budget.charge_work(3)?;
        let range = &self
            .accesses
            .get(access)
            .ok_or(ArgumentResourceV1::Accounting)?
            .leaves;
        let at = argument_sum_v1(&[range.start, ordinal])?;
        let at = *self
            .leaves
            .get(at)
            .filter(|_| at < range.end)
            .ok_or(ArgumentResourceV1::Accounting)?;
        match self.rows.get(at) {
            Some(SelectedTransportRowV30::Leaf {
                ordinal: found,
                original,
            }) if *found == ordinal => Ok(*original),
            _ => resources::binding("selected final original leaf ordinal"),
        }
    }

    pub(super) fn edge_projections(
        &self,
        access: usize,
        output: EdgeArgument,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[EdgeProjectionV30]> {
        charge_execution_cfg_lookup_v29(self.edges.len(), budget)
            .map_err(source_emission_error_v18)?;
        charge_execution_cfg_lookup_v29(self.edges.len(), budget)
            .map_err(source_emission_error_v18)?;
        let first = self
            .edges
            .partition_point(|row| (row.access, row.output) < (access, output));
        let end = self
            .edges
            .partition_point(|row| (row.access, row.output) <= (access, output));
        Ok(&self.edges[first..end])
    }

    pub(super) fn guard_projections(
        &self,
        access: usize,
        output: Edge,
        leaf: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[GuardProjectionV30]> {
        charge_execution_cfg_lookup_v29(self.guards.len(), budget)
            .map_err(source_emission_error_v18)?;
        charge_execution_cfg_lookup_v29(self.guards.len(), budget)
            .map_err(source_emission_error_v18)?;
        let first = self
            .guards
            .partition_point(|row| (row.access, row.output, row.leaf) < (access, output, leaf));
        let end = self
            .guards
            .partition_point(|row| (row.access, row.output, row.leaf) <= (access, output, leaf));
        Ok(&self.guards[first..end])
    }

    pub(super) fn guard(
        &self,
        access: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<PendingSourceSelectedGuardV30> {
        budget.charge_work(3)?;
        let range = &self
            .accesses
            .get(access)
            .ok_or(ArgumentResourceV1::Accounting)?
            .guards;
        let at = argument_sum_v1(&[range.start, ordinal])?;
        let at = *self
            .guard_rows
            .get(at)
            .filter(|_| at < range.end)
            .ok_or(ArgumentResourceV1::Accounting)?;
        match self.rows.get(at) {
            Some(SelectedTransportRowV30::Guard {
                ordinal: found,
                original,
                ..
            }) if *found == ordinal => Ok(*original),
            _ => resources::binding("selected final original guard ordinal"),
        }
    }

    pub(super) fn leaf_obligations(
        &self,
        access: usize,
        leaf: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[(usize, usize, usize)]> {
        charge_execution_cfg_lookup_v29(self.obligation_order.len(), budget)
            .map_err(source_emission_error_v18)?;
        charge_execution_cfg_lookup_v29(self.obligation_order.len(), budget)
            .map_err(source_emission_error_v18)?;
        let first = self
            .obligation_order
            .partition_point(|row| (row.0, row.1) < (access, leaf));
        let end = self
            .obligation_order
            .partition_point(|row| (row.0, row.1) <= (access, leaf));
        Ok(&self.obligation_order[first..end])
    }

    pub(super) fn obligation(
        &self,
        access: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(PendingSourceSelectedObligationV30, BlockControl)> {
        budget.charge_work(3)?;
        let range = &self
            .accesses
            .get(access)
            .ok_or(ArgumentResourceV1::Accounting)?
            .obligations;
        let at = argument_sum_v1(&[range.start, ordinal])?;
        let at = *self
            .obligations
            .get(at)
            .filter(|_| at < range.end)
            .ok_or(ArgumentResourceV1::Accounting)?;
        match self.rows.get(at) {
            Some(SelectedTransportRowV30::Obligation {
                ordinal: found,
                original,
                control,
                ..
            }) if *found == ordinal => Ok((*original, *control)),
            _ => resources::binding("selected final original obligation ordinal"),
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
            h::<SelectedFinalFactsV30<'_>>()?,
            h::<SelectedFinalDefinitionsV30<'_>>()?,
            h::<&SelectedFinalFactsV30<'_>>()?,
            h::<Range<usize>>()?,
            h::<Option<Definition>>()?,
            h::<Self>()?,
            h::<AccessIndexV30>()?,
            h::<Vec<AccessIndexV30>>()?,
            h::<DefinitionProjectionV30>()?,
            h::<Vec<DefinitionProjectionV30>>()?,
            h::<EdgeProjectionV30>()?,
            h::<Vec<EdgeProjectionV30>>()?,
            h::<GuardProjectionV30>()?,
            h::<Vec<GuardProjectionV30>>()?,
            argument_product_v1(4, h::<Vec<usize>>()?)?,
            h::<Vec<(usize, usize, usize)>>()?,
            h::<[usize; 7]>()?,
            h::<&[EdgeProjectionV30]>()?,
            h::<Option<&EdgeProjectionV30>>()?,
            h::<SourceEdgeV30>()?,
            h::<(usize, SourceEdgeV30)>()?,
            h::<Result<usize, usize>>()?,
            h::<&[GuardProjectionV30]>()?,
            h::<&[(usize, usize, usize)]>()?,
            h::<(usize, usize)>()?,
            h::<(PendingSourceSelectedObligationV30, BlockControl)>()?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_final_merged_leaf_guard_queries_exclude_unrelated_guards() {
        let block = Block {
            function: FunctionCoordinate(0),
            block: 0,
        };
        let output = Edge {
            source: block,
            successor: 0,
        };
        let selector = OutputUse {
            coordinate: UseCoordinate::TerminatorOperand { block, operand: 0 },
            definition: Definition::BlockArgument { block, argument: 0 },
        };
        for count in [8usize, 64, 512, 4096] {
            // Every original leaf shares one optimized pointer/guard edge.
            // Two guards per leaf must both survive the indexed query.
            let guards = (0..count)
                .flat_map(|leaf| {
                    (0..2).map(move |offset| GuardProjectionV30 {
                        access: 0,
                        output,
                        leaf,
                        ordinal: 2 * leaf + offset,
                        selector,
                    })
                })
                .collect::<Vec<_>>();
            let index = ProjectionIndexV30 {
                rows: &[],
                accesses: Vec::new(),
                definitions: Vec::new(),
                edges: Vec::new(),
                source_edges: Vec::new(),
                guards,
                leaves: Vec::new(),
                guard_rows: Vec::new(),
                obligations: Vec::new(),
                obligation_order: Vec::new(),
            };
            let floor = 17 + index.guards.capacity() * size_of::<GuardProjectionV30>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            let mut visited = 0;
            for leaf in 0..count {
                let guards = index
                    .guard_projections(0, output, leaf, &mut budget)
                    .unwrap();
                assert_eq!(guards.len(), 2);
                assert_eq!(
                    (guards[0].ordinal, guards[1].ordinal),
                    (2 * leaf, 2 * leaf + 1)
                );
                assert!(guards.iter().all(|row| row.leaf == leaf));
                visited += guards.len();
            }
            assert_eq!(visited, 2 * count);
            assert!(
                index
                    .guard_projections(0, output, count, &mut budget)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                index
                    .guard_projections(1, output, 0, &mut budget)
                    .unwrap()
                    .is_empty()
            );
            let log = (usize::BITS - (2 * count).leading_zeros()) as usize + 1;
            // Each range query pays its two independent partition searches.
            assert_eq!(budget.work(), 2 * 16 * (count + 2) * log);
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
        }
    }

    #[test]
    fn selected_final_projection_lookup_has_logarithmic_paid_queries_without_scratch_growth() {
        for count in [8usize, 64, 512, 4096] {
            let definitions = (0..count)
                .map(|ordinal| DefinitionProjectionV30 {
                    access: 0,
                    output: Definition::Result {
                        operation: OpCoordinate {
                            block: Block {
                                function: FunctionCoordinate(0),
                                block: 0,
                            },
                            operation: ordinal as u32,
                        },
                        result: 0,
                    },
                    role: SelectedDefinitionRoleV30::Leaf {
                        leaf: ordinal,
                        component: 6,
                    },
                })
                .collect::<Vec<_>>();
            let index = ProjectionIndexV30 {
                rows: &[],
                accesses: Vec::new(),
                definitions,
                edges: Vec::new(),
                source_edges: Vec::new(),
                guards: Vec::new(),
                leaves: Vec::new(),
                guard_rows: Vec::new(),
                obligations: Vec::new(),
                obligation_order: Vec::new(),
            };
            let floor = 17 + index.definitions.capacity() * size_of::<DefinitionProjectionV30>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            for ordinal in 0..count {
                let row = index.definitions[ordinal];
                assert!(index.matches(0, row.role, row.output, &mut budget).unwrap());
                assert_eq!(
                    index.leaf_candidates(0, row.output, &mut budget).unwrap(),
                    ordinal..ordinal + 1
                );
                assert!(!index.matches(1, row.role, row.output, &mut budget).unwrap());
            }
            let log = (usize::BITS - count.leading_zeros()) as usize + 1;
            // Two membership searches and the two leaf-range boundaries.
            assert_eq!(budget.work(), 4 * 16 * count * log);
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
        }
    }

    #[test]
    fn selected_final_projection_index_header_has_an_independent_field_oracle() {
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
        }
        type Fields<'a> = (
            &'a [SelectedTransportRowV30],
            Vec<AccessIndexV30>,
            Vec<DefinitionProjectionV30>,
            Vec<EdgeProjectionV30>,
            Vec<usize>,
            Vec<GuardProjectionV30>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
            Vec<(usize, usize, usize)>,
        );
        assert_eq!(size_of::<ProjectionIndexV30<'_>>(), size_of::<Fields<'_>>());
        assert_eq!(
            size_of::<GuardProjectionV30>(),
            size_of::<(usize, Edge, usize, usize, OutputUse)>()
        );
        let expected = h::<SelectedFinalFactsV30<'_>>()
            + h::<SelectedFinalDefinitionsV30<'_>>()
            + h::<&SelectedFinalFactsV30<'_>>()
            + h::<Range<usize>>()
            + h::<Option<Definition>>()
            + h::<Fields<'_>>()
            + h::<AccessIndexV30>()
            + h::<Vec<AccessIndexV30>>()
            + h::<DefinitionProjectionV30>()
            + h::<Vec<DefinitionProjectionV30>>()
            + h::<EdgeProjectionV30>()
            + h::<Vec<EdgeProjectionV30>>()
            + h::<GuardProjectionV30>()
            + h::<Vec<GuardProjectionV30>>()
            + 4 * h::<Vec<usize>>()
            + h::<Vec<(usize, usize, usize)>>()
            + h::<[usize; 7]>()
            + h::<&[EdgeProjectionV30]>()
            + h::<Option<&EdgeProjectionV30>>()
            + h::<SourceEdgeV30>()
            + h::<(usize, SourceEdgeV30)>()
            + h::<Result<usize, usize>>()
            + h::<&[GuardProjectionV30]>()
            + h::<&[(usize, usize, usize)]>()
            + h::<(usize, usize)>()
            + h::<(PendingSourceSelectedObligationV30, BlockControl)>();
        assert_eq!(ProjectionIndexV30::headers().unwrap(), expected);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let limit = 17 + expected - usize::from(short);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            budget.reserve_storage(17).unwrap();
            let result = budget.reserve_storage(ProjectionIndexV30::headers().unwrap());
            if short {
                assert!(
                    matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 17 + expected && error.limit() == limit)
                );
            } else {
                result.unwrap();
                budget.release_storage(expected).unwrap();
            }
            assert_eq!(budget.storage(), 17);
        }
    }
}
