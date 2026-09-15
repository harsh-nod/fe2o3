use super::*;

#[path = "publication_owner74_tests.rs"]
mod publication_owner74_tests;
use fe2o3_mir_model::semantic_mir_v1::*;
use global_enum_transport_v1::tests::{build_owner, build_owner_from_function};

#[path = "unchanged_join_owner_tests.rs"]
mod unchanged_join_owner_tests;

#[path = "neutral_owner_tests.rs"]
mod neutral_owner_tests;

const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

fn copy(local: u32) -> SemanticRvalueV1 {
    SemanticRvalueV1::new(
        U64,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], U64).unwrap(),
        )),
    )
}

fn assignment(local: u32, value: SemanticRvalueV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], U64).unwrap(),
            value,
        )),
    )
}

fn edge(role: SemanticEdgeRoleV1, target: usize) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target as u32))
}

fn goto(target: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

fn switch(left: usize, right: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(11), vec![], U64).unwrap(),
        ),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    }
}

fn block(
    index: usize,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let mut identity = [233; 32];
    identity[24..].copy_from_slice(&(index as u64).to_be_bytes());
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), term),
    )
    .unwrap()
}

// Real source/MIR/SSA owner constructors remain mandatory. The extra diamonds
// only stress repeated joins; their dimensions are not kernel fingerprints.
fn owner(scalars: usize, diamonds: usize, loop_kill: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = build_owner();
    let original = &base.source_semantic().functions()[0];
    let mut locals = original.locals().to_vec();
    let first = locals.len();
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    for ordinal in 0..scalars {
        let mut identity = [233; 32];
        identity[24..].copy_from_slice(&(ordinal as u64).to_be_bytes());
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity),
            U64,
            SemanticLocalRoleV1::Temporary,
            original.source(),
        ));
        statements.push(assignment((first + ordinal) as u32, copy(11)));
    }
    let terminator = if diamonds == 0 {
        original.blocks()[0].terminator().kind().clone()
    } else {
        goto(6)
    };
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        SemanticTerminatorV1::new(original.source(), terminator),
    )
    .unwrap();
    for ordinal in 0..diamonds {
        let split = blocks.len();
        blocks.push(block(split, vec![], switch(split + 1, split + 2)));
        for offset in 1..=2 {
            let statements = if scalars == 0 {
                vec![]
            } else {
                vec![assignment((first + ordinal % scalars) as u32, copy(11))]
            };
            blocks.push(block(split + offset, statements, goto(split + 3)));
        }
        blocks.push(block(
            split + 3,
            vec![],
            if ordinal + 1 == diamonds {
                original.blocks()[0].terminator().kind().clone()
            } else {
                goto(split + 4)
            },
        ));
    }
    if loop_kill {
        let read = blocks.len();
        blocks[4] = SemanticBasicBlockV1::new(
            blocks[4].identity(),
            blocks[4].source(),
            blocks[4].statements()[..2].to_vec(),
            SemanticTerminatorV1::new(original.source(), goto(read)),
        )
        .unwrap();
        blocks.push(block(
            read,
            original.blocks()[4].statements()[2..].to_vec(),
            switch(5, read + 1),
        ));
        // A late write leaves storage initialized but kills the old borrow.
        // Do not publish the read seen on the first traversal of the loop.
        let SemanticStatementKindV1::Assign(construction) =
            original.blocks()[0].statements()[2].kind()
        else {
            panic!()
        };
        let write = SemanticStatementV1::new(
            original.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(7),
                    vec![],
                    original.locals()[7].ty(),
                )
                .unwrap(),
                construction.value().clone(),
            )),
        );
        blocks.push(block(read + 1, vec![write], goto(read)));
    }
    build_owner_from_function(
        SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            locals,
            original.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone()),
    )
}

fn roster(reads: &PrivateScalarReads<'_>, body: &SemanticFunctionDeclV1) -> Vec<(usize, usize)> {
    body.blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body_block)| {
            body_block
                .statements()
                .iter()
                .enumerate()
                .filter_map(move |(index, statement)| {
                    reads.contains(body, statement).then_some((block, index))
                })
        })
        .collect()
}

#[test]
fn private_ordered_join_checked_owner_diamonds_preserve_complete_roster() {
    for (scalars, diamonds) in [(0, 1), (16, 4), (32, 12), (64, 24)] {
        let owner = owner(scalars, diamonds, false);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let before = view.body().clone();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        assert!(
            reads.observation.completed_without_exhaustion(),
            "{:?}",
            reads.observation
        );
        assert_eq!(roster(&reads, view.body()), vec![(4, 2)]);
        assert_eq!(view.body(), &before);
        eprintln!(
            "join56-owner scalars={scalars} diamonds={diamonds} work={} roster=4:2",
            MAX_WORK - reads.observation.remaining_work
        );
    }
}

#[test]
fn private_ordered_join_checked_owner_rejects_source_substitution() {
    let first = owner(16, 4, false);
    let other = owner(16, 4, false);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = first.execution_view_for_root(root).unwrap();
    let foreign = other.execution_view_for_root(root).unwrap();
    let reads = PrivateScalarReads::for_root(&first, root).unwrap();
    let clone = view.body().clone();
    assert_eq!(view.body(), foreign.body());
    assert!(!reads.contains(foreign.body(), &foreign.body().blocks()[4].statements()[2]));
    assert!(!reads.contains(&clone, &clone.blocks()[4].statements()[2]));
    assert!(reads.observation_for(&clone).is_none());
    assert!(
        reads
            .conditions_for(other.source_semantic().types(), view.body())
            .is_none()
    );
    assert!(
        reads
            .conditions_for(first.source_semantic().types(), &clone)
            .is_none()
    );
}

#[test]
fn private_ordered_join_checked_owner_final_replay_uses_same_budget() {
    let owner = owner(32, 12, false);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let types = owner.source_semantic().types();
    let mut full = Analysis::new(types, view);
    let expected = full.run().unwrap();
    let used = MAX_WORK - full.observation.remaining_work;
    for available in [0, 1, used / 2, used - 1, used] {
        let mut analysis = Analysis::new(types, view);
        analysis.budget = Budget::new(available);
        let result = analysis.run();
        if available == used {
            assert_eq!(result.unwrap(), expected);
            assert!(analysis.observation.completed_without_exhaustion());
        } else {
            assert!(result.is_err());
            assert_eq!(analysis.observation.admitted_reads, 0);
            assert!(!analysis.observation.completed);
            assert!(analysis.observation.work_exhausted);
        }
    }
    let mut storage = Analysis::new(types, view);
    let inherited = storage.budget.reserve(MAX_STORAGE).unwrap();
    assert!(storage.run().is_err());
    assert_eq!(storage.observation.admitted_reads, 0);
    assert!(!storage.observation.work_exhausted);
    drop(inherited);
}

#[test]
fn private_ordered_join_checked_owner_late_backedge_kills_early_read() {
    let owner = owner(8, 2, true);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
    assert!(
        reads.observation.completed_without_exhaustion(),
        "{:?}",
        reads.observation
    );
    assert_eq!(roster(&reads, view.body()), vec![]);
}
