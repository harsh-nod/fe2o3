//! Independent fixture-only census for the selected-edge cache work change.
//! This does not call transition State, control-index, or budget helpers.
use fe2o3_kernel_ir::{
    BinaryOp, BlockId, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirTransitionCandidateV1 as Candidate, ComparePredicate, Constant, FunctionBody,
    Module, OperationKind, Terminator, Type, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(super) enum Family {
    Licm,
    Preheaders,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Census {
    blocks: i64,
    conditionals: i64,
    switches: i64,
    conditional_phi_arguments: i64,
    rounds: i64,
}

impl Census {
    fn state_delta(self) -> i64 {
        let Self {
            blocks: b,
            conditionals: c,
            switches: s,
            conditional_phi_arguments: p,
            rounds: r,
        } = self;
        // New: B initialized slots, R refreshes of B blocks and C+S roots.
        // Old: 2C reachability roots and P phi roots each round, then 2C
        // final-control roots. Every selector in these fixtures has one root.
        b + r * (b + c + s) - r * (2 * c + p) - 2 * c
    }

    fn control_index_delta(self) -> i64 {
        // Index construction also retires C selected-successor queries and
        // 2C final executable-edge queries, each with one selector-root visit.
        self.state_delta() - 3 * self.conditionals
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplaySchedule {
    bc: Census,
    oi: Census,
}

impl ReplaySchedule {
    pub(super) fn complete_replay_delta(self) -> usize {
        // Two ordinary B/C checks, its control index, and one O/I check.
        usize::try_from(
            2 * self.bc.state_delta() + self.bc.control_index_delta() + self.oi.state_delta(),
        )
        .unwrap()
    }

    pub(super) fn map_refused_replay_delta(self) -> usize {
        // The P6 Map Storage denial precedes the O/I check.
        usize::try_from(2 * self.bc.state_delta() + self.bc.control_index_delta()).unwrap()
    }
}

fn expected(family: Family, mutation: bool) -> Census {
    let (blocks, conditionals, switches, conditional_phi_arguments, rounds) =
        match (family, mutation) {
            (Family::Preheaders, false) => (1, 0, 0, 0, 1),
            (Family::Licm, false) => (2, 1, 0, 0, 1),
            (Family::Preheaders, true) => (5, 1, 1, 0, 1),
            (Family::Licm, true) => (6, 2, 1, 0, 1),
        };
    Census {
        blocks,
        conditionals,
        switches,
        conditional_phi_arguments,
        rounds,
    }
}

fn edges(terminator: &Terminator) -> Vec<(BlockId, &[ValueId])> {
    match terminator {
        Terminator::Branch { target, arguments } => vec![(*target, arguments)],
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => vec![
            (*then_target, then_arguments),
            (*else_target, else_arguments),
        ],
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .iter()
            .map(|case| (case.target, case.arguments.as_slice()))
            .chain([(*default_target, default_arguments.as_slice())])
            .collect(),
        Terminator::Return { .. } => Vec::new(),
        other => panic!("fixture schedule does not cover {other:?}"),
    }
}

fn representative(parents: &BTreeMap<ValueId, ValueId>, mut value: ValueId) -> ValueId {
    for _ in 0..=parents.len() {
        let next = parents[&value];
        if next == value {
            return value;
        }
        value = next;
    }
    panic!("fixture equivalence cycle")
}

// Independent equality saturation over the actual edge payloads. There is no
// scalar folding in these fixture selectors. Policy6 precedes private-cell
// promotion, so these actual endpoints have no non-entry phi parameters. The separate
// synthetic control below still checks dependency-sensitive phi ordering.
// Literal descendant aliases are counted separately from this phi census.
fn phi_rounds(body: &FunctionBody) -> usize {
    let mut parents = BTreeMap::new();
    for value in body
        .parameters
        .iter()
        .copied()
        .chain(body.blocks.iter().flat_map(|block| {
            block.parameters.iter().map(|value| value.id).chain(
                block
                    .operations
                    .iter()
                    .flat_map(|op| op.results.iter().map(|value| value.id)),
            )
        }))
    {
        parents.insert(value, value);
    }
    for round in 1..=parents.len() + 1 {
        let mut changed = false;
        for block in body.blocks.iter().skip(1) {
            for (ordinal, parameter) in block.parameters.iter().enumerate() {
                let current = representative(&parents, parameter.id);
                let mut candidates = BTreeSet::new();
                for predecessor in &body.blocks {
                    for (target, arguments) in edges(predecessor.terminator.as_ref().unwrap()) {
                        if target == block.id {
                            assert_eq!(arguments.len(), block.parameters.len());
                            let incoming = representative(&parents, arguments[ordinal]);
                            if incoming != current {
                                candidates.insert(incoming);
                            }
                        }
                    }
                }
                if candidates.len() == 1 {
                    let incoming = *candidates.first().unwrap();
                    parents.insert(current.max(incoming), current.min(incoming));
                    changed = true;
                }
            }
        }
        if !changed {
            return round;
        }
    }
    panic!("fixture phi equations did not converge")
}

fn census(module: &Module) -> Census {
    assert_eq!(module.functions.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    let mut result = Census {
        blocks: body.blocks.len().try_into().unwrap(),
        conditionals: 0,
        switches: 0,
        conditional_phi_arguments: 0,
        rounds: phi_rounds(body).try_into().unwrap(),
    };
    let mut reachable = BTreeSet::from([body.blocks[0].id]);
    loop {
        let before = reachable.len();
        for block in &body.blocks {
            if reachable.contains(&block.id) {
                for (target, _) in edges(block.terminator.as_ref().unwrap()) {
                    reachable.insert(target);
                }
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    assert_eq!(reachable.len(), body.blocks.len());
    for block in &body.blocks {
        let terminator = block.terminator.as_ref().unwrap();
        let selector = match terminator {
            Terminator::ConditionalBranch { condition, .. } => {
                result.conditionals += 1;
                for (target, arguments) in edges(terminator) {
                    let target = body.blocks.iter().find(|block| block.id == target).unwrap();
                    assert_eq!(arguments.len(), target.parameters.len());
                    result.conditional_phi_arguments +=
                        i64::try_from(target.parameters.len()).unwrap();
                }
                Some(*condition)
            }
            Terminator::Switch {
                selector, cases, ..
            } => {
                assert!(!cases.is_empty());
                result.switches += 1;
                Some(*selector)
            }
            _ => None,
        };
        if let Some(selector) = selector {
            // Selectors are original dynamic function parameters or their
            // directly defining nonconstant operation, never phi aliases.
            assert!(
                !body
                    .blocks
                    .iter()
                    .any(|block| block.parameters.iter().any(|p| p.id == selector))
            );
            if !body.parameters.contains(&selector) {
                let operation = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|op| op.results.iter().any(|result| result.id == selector))
                    .unwrap();
                assert!(!matches!(operation.kind, OperationKind::Constant(_)));
            }
        }
    }
    result
}

fn definition_literal(module: &Module, definition: Definition) -> Option<(&Constant, &Type)> {
    let Definition::Result { operation, result } = definition else {
        return None;
    };
    let body = module
        .functions
        .get(operation.block.function.0 as usize)?
        .body
        .as_ref()?;
    let operation = body
        .blocks
        .get(operation.block.block as usize)?
        .operations
        .get(operation.operation as usize)?;
    let OperationKind::Constant(value) = &operation.kind else {
        return None;
    };
    Some((value, &operation.results.get(result as usize)?.ty))
}

// This is deliberately not a transition solver. It accepts only the concrete
// pre-promotion fixture grammar, whose scalar expressions cannot add facts.
// Equal typed constants are seeded before rounds; all their descendant unions
// therefore settle in one sweep, followed by one unchanged sweep if needed.
fn literal_alias_rounds(module: &Module, rows: Candidate<'_>) -> Result<i64, &'static str> {
    if module.functions.len() != 1 {
        return Err("fixture function count");
    }
    let body = module.functions[0]
        .body
        .as_ref()
        .ok_or("fixture declaration")?;
    // Initial entry values have no predecessor-proven phi fact. The genuine
    // transition solver also excludes the first block from phi aliases.
    if body
        .blocks
        .iter()
        .skip(1)
        .any(|block| !block.parameters.is_empty())
    {
        return Err("fixture phi dependency");
    }
    let literals: BTreeMap<_, _> = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Constant(value) if operation.results.len() == 1 => {
                Some((operation.results[0].id, value))
            }
            _ => None,
        })
        .collect();
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        match &operation.kind {
            OperationKind::Binary {
                op,
                lhs: left,
                rhs: right,
            } => {
                let a = literals.get(left).copied();
                let b = literals.get(right).copied();
                // Actual loop increment and parity mask only; neither is an
                // integer identity or a constant-folding expression.
                if !matches!(op, BinaryOp::Add | BinaryOp::BitAnd)
                    || !matches!(
                        (a, b),
                        (None, Some(Constant::U32(1))) | (Some(Constant::U32(1)), None)
                    )
                {
                    return Err("fixture scalar alias or folding dependency");
                }
            }
            OperationKind::Compare {
                predicate,
                lhs: left,
                rhs: right,
            } => {
                let a = literals.get(left).copied();
                let b = literals.get(right).copied();
                if !matches!(
                    (predicate, a, b),
                    (ComparePredicate::Equal, None, Some(Constant::U32(0)))
                        | (ComparePredicate::Equal, Some(Constant::U32(0)), None)
                        | (ComparePredicate::LessThan, None, Some(Constant::U32(3)))
                ) {
                    return Err("fixture scalar comparison dependency");
                }
            }
            OperationKind::Unary { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. } => {
                return Err("fixture scalar chain dependency");
            }
            OperationKind::Constant(_)
            | OperationKind::Alloca { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Load { .. }
            | OperationKind::Store { .. } => (),
            _ => return Err("fixture operation outside reviewed grammar"),
        }
    }
    let mut anchors = BTreeMap::new();
    for row in rows.definitions {
        let start = row.outputs.start as usize;
        let end = start
            .checked_add(row.outputs.len as usize)
            .ok_or("fixture row range")?;
        for descendant in rows
            .definition_outputs
            .get(start..end)
            .ok_or("fixture row range")?
        {
            if descendant.kind == Descendant::Retained {
                if anchors.insert(descendant.output, row.input).is_some() {
                    return Err("fixture duplicate anchor");
                }
            }
        }
    }
    for row in rows.operations {
        if let Origin::ConstantFrom(source) = row.origin {
            if definition_literal(module, source).is_none() {
                return Err("fixture synthesized nonliteral");
            }
            let target = Definition::Result {
                operation: row.output,
                result: 0,
            };
            if anchors.insert(target, source).is_some() {
                return Err("fixture duplicate anchor");
            }
        }
    }
    let mut aliases = false;
    for row in rows.definitions {
        let start = row.outputs.start as usize;
        let end = start
            .checked_add(row.outputs.len as usize)
            .ok_or("fixture row range")?;
        for descendant in rows
            .definition_outputs
            .get(start..end)
            .ok_or("fixture row range")?
        {
            let anchor = *anchors
                .get(&descendant.output)
                .ok_or("fixture missing anchor")?;
            if row.input != anchor {
                let source =
                    definition_literal(module, row.input).ok_or("fixture nonliteral alias")?;
                let target =
                    definition_literal(module, anchor).ok_or("fixture nonliteral alias")?;
                if source != target {
                    return Err("fixture unequal literal alias");
                }
                aliases = true;
            }
        }
    }
    Ok(1 + i64::from(aliases))
}

pub(super) fn assert_policy6_endpoints(
    owner: &crate::ProductionCheckedOutputOwnerPolicy6V1,
    family: Family,
    mutation: bool,
) -> ReplaySchedule {
    let fifth = owner.checked_output().intermediate_policy5();
    let third = fifth.intermediate_policy4().intermediate_policy3();
    for (name, module) in [
        ("B", owner.bound().module()),
        ("C", third.owner().module()),
        ("O", fifth.owner().module()),
        ("I", owner.output().module()),
    ] {
        assert_eq!(
            census(module),
            expected(family, mutation),
            "{family:?}/{mutation}/{name}"
        );
    }
    let mut bc = expected(family, mutation);
    bc.rounds = literal_alias_rounds(owner.bound().module(), third.occurrences().candidate())
        .expect("B/C fixture must have only independent literal aliases");
    let mut oi = expected(family, mutation);
    oi.rounds = literal_alias_rounds(
        fifth.owner().module(),
        owner
            .checked_output()
            .continuation()
            .occurrences()
            .candidate(),
    )
    .expect("O/I fixture must have only independent literal aliases");
    ReplaySchedule { bc, oi }
}

#[test]
fn transition_cache_fixture_work_equations_are_literal_and_phase_sensitive() {
    for (family, mutation, d, index, complete, partial) in [
        (Family::Preheaders, false, 2, 2, 8, 6),
        (Family::Licm, false, 1, -2, 1, 0),
        (Family::Preheaders, true, 8, 5, 29, 21),
        (Family::Licm, true, 7, 1, 22, 15),
    ] {
        let census = expected(family, mutation);
        assert_eq!(census.state_delta(), d);
        assert_eq!(census.control_index_delta(), index);
        let schedule = ReplaySchedule {
            bc: census,
            oi: census,
        };
        assert_eq!(schedule.complete_replay_delta(), complete);
        assert_eq!(schedule.map_refused_replay_delta(), partial);
    }
    assert_eq!(249_631 + 3 * 1, 249_634);
    assert_eq!(238_296 + 2 * 1 + 0, 238_298);
    assert_eq!(65_552 + 8 + 6, 65_566);
    // BC and OI have independent literal-merge stabilization schedules.
    for (family, bc_rounds, oi_rounds, complete, partial) in [
        (Family::Preheaders, 1, 1, 29, 21),
        (Family::Preheaders, 1, 2, 34, 21),
        (Family::Preheaders, 2, 1, 44, 36),
        (Family::Preheaders, 2, 2, 49, 36),
        (Family::Licm, 1, 1, 22, 15),
        (Family::Licm, 1, 2, 27, 15),
        (Family::Licm, 2, 1, 37, 30),
        (Family::Licm, 2, 2, 42, 30),
    ] {
        let mut bc = expected(family, true);
        let mut oi = bc;
        bc.rounds = bc_rounds;
        oi.rounds = oi_rounds;
        let schedule = ReplaySchedule { bc, oi };
        assert_eq!(schedule.complete_replay_delta(), complete);
        assert_eq!(schedule.map_refused_replay_delta(), partial);
    }
}

#[test]
fn transition_cache_fixture_literal_alias_rounds_are_exact_and_refuse_other_dependencies() {
    use fe2o3_kernel_ir::{
        BasicBlock, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirDefinitionDescendantV1 as Output, CanonicalKirDefinitionTransitionV1 as Row,
        CanonicalKirFunctionCoordinateV1 as FunctionId,
        CanonicalKirOperationCoordinateV1 as OperationId, CanonicalKirTransitionRangeV1 as Range,
        Function, Operation, ScalarType, Signature, ValueDef,
    };
    fn candidate<'a>(definitions: &'a [Row], outputs: &'a [Output]) -> Candidate<'a> {
        Candidate {
            functions: &[],
            blocks: &[],
            segments: &[],
            operations: &[],
            definitions,
            definition_outputs: outputs,
            uses: &[],
            edges: &[],
            edge_arguments: &[],
        }
    }
    let definition = |operation| Definition::Result {
        operation: OperationId {
            block: Block {
                function: FunctionId(0),
                block: 0,
            },
            operation,
        },
        result: 0,
    };
    let mut block = BasicBlock::new(BlockId(0));
    for id in 0..2 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("literal_round_control");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut definitions = [
        Row {
            input: definition(0),
            outputs: Range { start: 0, len: 1 },
        },
        Row {
            input: definition(1),
            outputs: Range { start: 1, len: 1 },
        },
    ];
    let mut outputs = [
        Output {
            output: definition(0),
            kind: Descendant::Retained,
        },
        Output {
            output: definition(1),
            kind: Descendant::Retained,
        },
    ];
    // These are inert equation controls, not admitted transition owners.
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(1)
    );
    outputs[1] = Output {
        output: definition(0),
        kind: Descendant::Substituted,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(2)
    );
    definitions[1].input = Definition::FunctionArgument {
        function: FunctionId(0),
        argument: 0,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture nonliteral alias")
    );
    definitions[1].input = definition(1);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::U32(8));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture unequal literal alias")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::U32(7));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture scalar alias or folding dependency")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "unreviewed_call".into(),
                arguments: vec![],
            },
        ));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture operation outside reviewed grammar")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(2),
        "initial entry parameters do not add a phi round"
    );
    outputs[1] = Output {
        output: definition(1),
        kind: Descendant::Retained,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(1),
        "initial entry parameters do not require any merge sweep"
    );
    let mut phi = BasicBlock::new(BlockId(1));
    phi.parameters
        .push(ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)));
    phi.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks.push(phi);
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture phi dependency")
    );
}

#[test]
fn transition_cache_fixture_rounds_follow_phi_dependency_order_not_block_count() {
    use fe2o3_kernel_ir::{BasicBlock, ScalarType, Type, ValueDef};
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)));
    header.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(1)],
    });
    let mut latch = BasicBlock::new(BlockId(2));
    latch
        .parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)));
    latch.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    // This is only a control for the independent census, not an admitted owner.
    let mut body = FunctionBody {
        parameters: vec![ValueId(0)],
        blocks: vec![entry, header, latch],
    };
    assert_eq!(phi_rounds(&body), 3);
    body.blocks.swap(1, 2);
    assert_eq!(phi_rounds(&body), 2);
}
