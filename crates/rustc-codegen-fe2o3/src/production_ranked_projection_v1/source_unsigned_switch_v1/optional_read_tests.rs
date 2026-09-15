use super::*;

#[derive(Default)]
struct Settings {
    nodes: usize,
    next_value: Option<u32>,
    exhaust_work: bool,
    use_block: Option<usize>,
}

struct Probe {
    result: Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1>,
    operations: Vec<ProductionRankedOperationV1>,
    initial_value: u32,
    next_value: u32,
    work_before: usize,
    work_after: usize,
}

fn fixture() -> Fixture {
    let mut f = Fixture::new(SemanticBinaryOpV1::LessThan, 16, false);
    f.locals.extend([WORD, WORD, WORD]);
    let mut statements = f.blocks[2].statements().to_vec();
    for (destination, operation, left, right) in [
        (
            6,
            SemanticBinaryOpV1::Divide,
            copy(3, WORD),
            constant(WORD, 64, 8),
        ),
        (
            7,
            SemanticBinaryOpV1::Multiply,
            copy(6, WORD),
            constant(WORD, 16, 8),
        ),
        (8, SemanticBinaryOpV1::Add, copy(7, WORD), copy(4, WORD)),
    ] {
        statements.push(assign(
            destination,
            WORD,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            },
        ));
    }
    f.replace_block(2, statements, f.blocks[2].terminator().kind().clone());
    f
}

fn probe(f: &Fixture, operand: SemanticOperandV1, settings: Settings) -> Probe {
    let function = f.function();
    let original = function.clone();
    let inventory = assertion_definition_inventory(&function).unwrap();
    let constants = vec![None; f.locals.len()];
    let mut proof = SemanticAssertProofsV1::new(&f.types, &function).unwrap();
    let mut indices = vec![None; f.locals.len()];
    for slot in &mut indices[1..=3] {
        *slot = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
    }
    let mut arguments = vec![Some(73); f.locals.len()];
    let mut next_argument = 74;
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    let initial_value = settings.next_value.unwrap_or(1);
    let mut next_value = initial_value;
    let mut projector = TotalUnsignedIndexProjectorV1::new(
        &f.types,
        &function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        &mut proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap()
    .with_invocation_roots(&f.callables, &indices, 1024)
    .unwrap();
    projector.node_work = settings.nodes;
    if settings.exhaust_work {
        projector
            .assertion_proofs
            .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - projector.assertion_proofs.work)
            .unwrap();
    }
    let work_before = projector.assertion_proofs.work;
    let use_block = settings.use_block.unwrap_or(2);
    let result = projector.optional_read_index_v1(
        &operand,
        use_block,
        function.blocks()[use_block].statements().len(),
    );
    assert_eq!(function, original);
    assert_eq!(arguments, vec![Some(73); f.locals.len()]);
    assert_eq!(next_argument, 74);
    Probe {
        result,
        operations,
        initial_value,
        next_value,
        work_before,
        work_after: proof.work,
    }
}

fn assert_rollback(p: &Probe) {
    assert_eq!(
        p.operations,
        vec![ProductionRankedOperationV1::InvocationIndex {
            result: ProductionRankedValueIdV1::new(0),
            dimension: 0,
            launch_extent: 1024,
        }]
    );
    assert_eq!(p.next_value, p.initial_value);
    assert!(p.work_after >= p.work_before);
}

#[test]
fn optional_read_index_commits_exact_total_source_but_not_constant_authority() {
    let f = fixture();
    let p = probe(&f, copy(8, WORD), Settings::default());
    assert!(matches!(
        p.result,
        Ok(Some(ProductionRankedValueV1::Local(_)))
    ));
    assert!(p.operations.iter().any(|op| matches!(
        op,
        ProductionRankedOperationV1::IndexBinary {
            kind: IndexBinaryKindAttr::Remainder,
            ..
        }
    )));
    assert!(p.work_after > p.work_before);
    let p = probe(&f, constant(WORD, 17, 8), Settings::default());
    assert!(matches!(p.result, Ok(None)));
    assert_rollback(&p);
}

#[test]
fn optional_read_index_node_limit_rolls_back_after_partial_emission() {
    let f = fixture();
    // This operand emits at least its literal before requiring another node.
    let p = probe(
        &f,
        copy(8, WORD),
        Settings {
            nodes: MAX_PURE_UNIFORM_INDEX_NODES_V1 - 8,
            ..Settings::default()
        },
    );
    assert!(matches!(p.result, Ok(None)));
    assert_rollback(&p);
    assert!(p.work_after > p.work_before);
}

#[test]
fn optional_read_index_fatal_emission_error_is_not_a_precision_miss() {
    let p = probe(
        &fixture(),
        copy(8, WORD),
        Settings {
            next_value: Some(u32::MAX - 1),
            ..Settings::default()
        },
    );
    assert!(matches!(
        p.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "too many ranked SSA values"
        ))
    ));
    assert_rollback(&p);
    assert!(p.work_after > p.work_before);
}

#[test]
fn optional_read_index_shared_work_exhaustion_remains_fatal_without_refund() {
    let p = probe(
        &fixture(),
        copy(8, WORD),
        Settings {
            exhaust_work: true,
            ..Settings::default()
        },
    );
    assert!(matches!(
        p.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_rollback(&p);
    assert!(p.work_after >= MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn optional_read_index_rejects_declared_type_and_signedness_mismatch() {
    for ty in [SIGNED, NARROW, BOOL] {
        let p = probe(&fixture(), copy(8, ty), Settings::default());
        assert!(matches!(p.result, Ok(None)));
        assert_rollback(&p);
    }
}

#[test]
fn optional_read_index_rejects_reassignment_and_deinitialize() {
    for statement in [
        assign(8, WORD, SemanticRvalueKindV1::Use(copy(4, WORD))),
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Deinitialize(place(8, WORD)),
        ),
    ] {
        let mut f = fixture();
        let mut statements = f.blocks[2].statements().to_vec();
        statements.push(statement);
        f.replace_block(2, statements, f.blocks[2].terminator().kind().clone());
        let p = probe(&f, copy(8, WORD), Settings::default());
        assert!(matches!(p.result, Ok(None)));
        assert_rollback(&p);
    }
}

#[test]
fn optional_read_index_requires_definition_before_the_exact_use() {
    let p = probe(
        &fixture(),
        copy(8, WORD),
        Settings {
            use_block: Some(1),
            ..Settings::default()
        },
    );
    assert!(matches!(p.result, Ok(None)));
    assert_rollback(&p);
}
