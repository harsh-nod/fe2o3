use super::*;

fn normal_switch(targets: &[usize], moved_selector: bool) -> SemanticTerminatorKindV1 {
    let Some(&otherwise) = targets.first() else {
        return SemanticTerminatorKindV1::Return;
    };
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: if moved_selector {
            moved(1, INTEGER)
        } else {
            copy(2, INTEGER)
        },
        targets: SemanticSwitchTargetsV1::new(
            targets
                .iter()
                .enumerate()
                .map(|(value, &target)| {
                    SemanticSwitchTargetV1::new(
                        value as u128,
                        edge(SemanticEdgeRoleV1::SwitchValue, target as u32),
                    )
                })
                .collect(),
            edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise as u32),
        )
        .unwrap(),
    }
}

fn compare_queries(function: &SemanticFunctionDeclV1, cases: &[(usize, Option<SiteV1>, SiteV1)]) {
    let original = function.clone();
    let mut used = 0;
    let mut graph = LosslessCsrV1::build(
        function,
        CsrWorkV1::new(&mut used, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1),
    )
    .unwrap();
    let mut legacy = LegacyBindingV1 {
        function,
        graph: projected_loop_cfg_graph_v1(function).unwrap(),
        budget: BudgetV1::default(),
    };
    for &(local, expected, site) in cases {
        let before = graph.binding_allocation_identity();
        let result = graph
            .query(
                CsrWorkV1::new(&mut used, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1),
                |mut query| query.exact_binding_reaches(local, INTEGER, expected, site),
            )
            .unwrap();
        assert_eq!(
            result,
            legacy
                .exact_binding_reaches(local, INTEGER, expected, site)
                .unwrap(),
            "exact local {local} definition {expected:?} use {site:?}"
        );
        let after = graph.binding_allocation_identity();
        assert_eq!(
            before.map(|(pointer, _, capacity)| (pointer, capacity)),
            after.map(|(pointer, _, capacity)| (pointer, capacity))
        );
        assert!(after[7].1 <= function.blocks().len());
    }
    assert_eq!(function, &original);
}

#[test]
fn csr_binding_all_three_block_graphs_match_original_query() {
    for encoding in 0..512_usize {
        let source = function(
            (0..3)
                .map(|block| {
                    let mask = (encoding >> (3 * block)) & 7;
                    let targets = (0..3)
                        .filter(|target| mask & (1 << target) != 0)
                        .collect::<Vec<_>>();
                    let statements = if block == 1 {
                        vec![alias(5, copy(1, INTEGER))]
                    } else {
                        vec![SemanticStatementV1::new(
                            SemanticSourceProvenanceV1::unavailable(),
                            SemanticStatementKindV1::Nop,
                        )]
                    };
                    (statements, normal_switch(&targets, false))
                })
                .collect(),
        );
        let mut cases = Vec::new();
        for local in [1, 5] {
            for block in 0..3 {
                for before in 0..=1 {
                    cases.push((local, (local == 5).then_some((1, 0)), (block, before)));
                }
            }
        }
        compare_queries(&source, &cases);
    }
}

#[test]
fn csr_binding_initial_prefix_does_not_hide_full_block_loop_revisit() {
    for backedge in [false, true] {
        let source = function(vec![
            (
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            (
                vec![alias(5, moved(1, INTEGER))],
                normal_switch(if backedge { &[1, 2] } else { &[2] }, false),
            ),
            (vec![], SemanticTerminatorKindV1::Return),
        ]);
        compare_queries(&source, &[(1, None, (1, 0)), (1, None, (1, 1))]);
        let mut binding = BindingAuditV1::new(&source, BudgetV1::default()).unwrap();
        assert_eq!(
            binding
                .exact_binding_reaches(1, INTEGER, None, (1, 0))
                .unwrap(),
            !backedge
        );
        assert!(
            !binding
                .exact_binding_reaches(1, INTEGER, None, (1, 1))
                .unwrap()
        );
    }
}

#[test]
fn csr_binding_shared_predecessor_terminator_kill_is_not_skipped() {
    for killed in [false, true] {
        let source = function(vec![
            (vec![], normal_switch(&[1, 2], killed)),
            (
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            (vec![], SemanticTerminatorKindV1::Return),
        ]);
        compare_queries(&source, &[(1, None, (2, 0))]);
        let mut binding = BindingAuditV1::new(&source, BudgetV1::default()).unwrap();
        assert_eq!(
            binding
                .exact_binding_reaches(1, INTEGER, None, (2, 0))
                .unwrap(),
            !killed
        );
    }
}

#[test]
fn csr_binding_exact_definition_type_lifetime_and_escape_gates_remain() {
    let source = function(vec![
        (
            vec![alias(5, copy(1, INTEGER))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        (vec![], SemanticTerminatorKindV1::Return),
    ]);
    let mut binding = BindingAuditV1::new(&source, BudgetV1::default()).unwrap();
    assert!(
        binding
            .exact_binding_reaches(5, INTEGER, Some((0, 0)), (1, 0))
            .unwrap()
    );
    assert!(
        !binding
            .exact_binding_reaches(5, INTEGER, Some((1, 0)), (1, 0))
            .unwrap()
    );
    assert!(
        !binding
            .exact_binding_reaches(5, OTHER_INTEGER, Some((0, 0)), (1, 0))
            .unwrap()
    );
    let types = types(false, 64);
    assert!(
        binding
            .matches_resolver(&types, &copy(5, INTEGER), (1, 0))
            .unwrap()
    );
    binding.inventory.address_escaped[5] = true;
    assert!(
        !binding
            .matches_resolver(&types, &copy(5, INTEGER), (1, 0))
            .unwrap()
    );
    for kind in [
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        SemanticStatementKindV1::Deinitialize(place(1, INTEGER)),
    ] {
        let source = function(vec![(
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                kind,
            )],
            SemanticTerminatorKindV1::Return,
        )]);
        compare_queries(&source, &[(1, None, (0, 1))]);
        assert!(
            !BindingAuditV1::new(&source, BudgetV1::default())
                .unwrap()
                .exact_binding_reaches(1, INTEGER, None, (0, 1))
                .unwrap()
        );
    }
}

#[test]
fn csr_binding_constructor_and_query_keep_inherited_work_ceiling() {
    let source = function(vec![(vec![], SemanticTerminatorKindV1::Return)]);
    let mut graph_work = 0;
    LosslessCsrV1::build(
        &source,
        CsrWorkV1::new(&mut graph_work, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1),
    )
    .unwrap();
    let mut binding = BindingAuditV1::new(&source, BudgetV1 { used: 17 }).unwrap();
    assert_eq!(binding.budget.used, 17 + graph_work + source.blocks().len());
    binding.budget.used = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1;
    let before = binding.graph.binding_allocation_identity();
    assert_eq!(
        binding.exact_binding_reaches(1, INTEGER, None, (0, 0)),
        Err(WORK_LIMIT)
    );
    assert_eq!(binding.graph.binding_allocation_identity(), before);
    let mut exhausted = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1;
    assert!(
        LosslessCsrV1::build(
            &source,
            CsrWorkV1::new(&mut exhausted, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1)
        )
        .is_err()
    );
}

#[test]
fn csr_binding_rejected_leaf_cannot_reallocate_or_poison_later_query() {
    let source = function(vec![
        (vec![], normal_switch(&[1, 2], false)),
        (
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        (vec![], SemanticTerminatorKindV1::Return),
    ]);
    let mut binding = BindingAuditV1::new(&source, BudgetV1::default()).unwrap();
    let allocations = binding
        .graph
        .binding_allocation_identity()
        .map(|(p, _, c)| (p, c));
    for site in [(3, 0), (0, 1), (usize::MAX, usize::MAX)] {
        assert!(
            !binding
                .exact_binding_reaches(1, INTEGER, None, site)
                .unwrap()
        );
        assert!(
            binding
                .exact_binding_reaches(1, INTEGER, None, (2, 0))
                .unwrap()
        );
    }
    assert_eq!(
        binding
            .graph
            .binding_allocation_identity()
            .map(|(p, _, c)| (p, c)),
        allocations
    );
    assert_eq!(binding.graph.storage_items(), 6 * 3 + 2 * 3 + 2);
    assert!(binding.graph.storage_items() <= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1);
}

// Exact pre-conversion query, including its independent old normal graph.
struct LegacyBindingV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    graph: ProjectedLoopCfgV1,
    budget: BudgetV1,
}
impl LegacyBindingV1<'_> {
    fn exact_binding_reaches(
        &mut self,
        local: usize,
        ty: SemanticTypeIdV1,
        expected: Option<SiteV1>,
        use_site: SiteV1,
    ) -> Result<bool, &'static str> {
        let mut pending = Vec::new();
        self.budget.push(&mut pending, use_site)?;
        let mut visited = HashSet::new();
        let mut found = false;
        'query: while let Some((block_index, before)) = pending.pop() {
            self.budget.charge(1)?;
            visited.try_reserve(1).map_err(|_| STORAGE)?;
            if !visited.insert((block_index, before)) {
                continue;
            }
            if !self
                .graph
                .reachable
                .get(block_index)
                .copied()
                .unwrap_or(false)
            {
                return Ok(false);
            }
            let block = &self.function.blocks()[block_index];
            for statement_index in (0..before).rev() {
                self.budget.charge(1)?;
                match block.statements()[statement_index].kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if !assignment.destination().projections().is_empty() {
                            return Ok(false);
                        }
                        if assignment.destination().local().index() as usize == local {
                            if expected != Some((block_index, statement_index))
                                || assignment.destination().ty() != ty
                                || assignment.value().result_type() != ty
                            {
                                return Ok(false);
                            }
                            found = true;
                            continue 'query;
                        }
                        let mut moved = false;
                        assignment.value().kind().try_visit_operands(|operand| {
                            self.budget.charge(1)?;
                            moved |= moves_local(operand, local);
                            Ok::<_, &'static str>(())
                        })?;
                        if moved {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::Store(_)
                    | SemanticStatementKindV1::AtomicRmw(_)
                    | SemanticStatementKindV1::AtomicCompareExchange(_) => return Ok(false),
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                    | SemanticStatementKindV1::Deinitialize(place) => {
                        if !place.projections().is_empty()
                            || place.local().index() as usize == local
                        {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::StorageLive(id)
                    | SemanticStatementKindV1::StorageDead(id) => {
                        if id.index() as usize == local {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::Assume(operand) if moves_local(operand, local) => {
                        return Ok(false);
                    }
                    SemanticStatementKindV1::Assume(_) | SemanticStatementKindV1::Nop => {}
                }
            }
            if block_index == self.graph.entry {
                if expected.is_some() {
                    return Ok(false);
                }
                found = true;
            }
            for &predecessor in &self.graph.predecessors[block_index] {
                self.budget.charge(1)?;
                if !self.graph.reachable[predecessor] {
                    continue;
                }
                let block = &self.function.blocks()[predecessor];
                let killed = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(_)
                    | SemanticTerminatorKindV1::TailCall(_)
                    | SemanticTerminatorKindV1::Drop { .. } => true,
                    SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                        moves_local(discriminant, local)
                    }
                    SemanticTerminatorKindV1::Assert { condition, .. } => {
                        moves_local(condition, local)
                    }
                    _ => false,
                };
                if killed {
                    return Ok(false);
                }
                self.budget
                    .push(&mut pending, (predecessor, block.statements().len()))?;
            }
        }
        Ok(found)
    }
}
