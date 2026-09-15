use super::*;
use private_scalar_capture_v1::PrivateScalarReads;

// Synthetic retained owners isolate accounting cost without importing facts
// from source kernel dimensions or weakening the owner constructors.
fn workload(
    scalars: usize,
    overwrites: usize,
    tail_blocks: usize,
    reassign: bool,
) -> ProductionSemanticSsaOwnerV1 {
    assert!(scalars >= 2);
    let original = function();
    let mut locals = original.locals().to_vec();
    let first = locals.len();
    for ordinal in 0..scalars {
        let mut identity = [231; 32];
        identity[24..].copy_from_slice(&(ordinal as u64).to_be_bytes());
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity),
            U64,
            SemanticLocalRoleV1::Temporary,
            source(),
        ));
    }
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    for ordinal in 0..scalars {
        statements.push(statement(
            (first + ordinal) as u32,
            U64,
            SemanticRvalueKindV1::Use(constant(ordinal as u128)),
        ));
    }
    for ordinal in 0..overwrites {
        statements.push(statement(
            (first + ordinal % scalars) as u32,
            U64,
            SemanticRvalueKindV1::Use(copy((first + (ordinal + 1) % scalars) as u32, U64)),
        ));
    }
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    if reassign {
        let mut statements = blocks[4].statements().to_vec();
        statements.insert(
            2,
            statement(
                7,
                WRAPPER,
                aggregate(
                    SemanticAggregateKindV1::Aggregate,
                    vec![copy(2, REF), constant(9)],
                ),
            ),
        );
        blocks[4] = block(4, statements, blocks[4].terminator().kind().clone());
    }
    if tail_blocks != 0 {
        blocks[4] = block(
            4,
            blocks[4].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
        );
        for ordinal in 0..tail_blocks {
            let index = blocks.len();
            let mut identity = [232; 32];
            identity[24..].copy_from_slice(&(ordinal as u64).to_be_bytes());
            blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity),
                    source(),
                    vec![statement(
                        first as u32,
                        U64,
                        SemanticRvalueKindV1::Use(copy((first + 1) as u32, U64)),
                    )],
                    SemanticTerminatorV1::new(
                        source(),
                        if ordinal + 1 == tail_blocks {
                            SemanticTerminatorKindV1::Return
                        } else {
                            SemanticTerminatorKindV1::Goto(edge(
                                SemanticEdgeRoleV1::Goto,
                                (index + 1) as u32,
                            ))
                        },
                    ),
                )
                .unwrap(),
            );
        }
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

fn assert_roster(owner: &ProductionSemanticSsaOwnerV1, expected: &[(usize, usize)]) {
    let root = SemanticFunctionIdV1::from_index(0);
    let body = owner.execution_view_for_root(root).unwrap().body();
    let reads = PrivateScalarReads::for_root(owner, root).unwrap();
    let observation = reads.observation_for(body).unwrap();
    assert!(
        observation.completed_without_exhaustion(),
        "{observation:?}"
    );
    let mut roster = Vec::new();
    for (block, body_block) in body.blocks().iter().enumerate() {
        for (index, statement) in body_block.statements().iter().enumerate() {
            if reads.contains(body, statement) {
                roster.push((block, index));
            }
        }
    }
    assert_eq!(roster, expected);
}

#[test]
fn private_counting_dense_scalar_reassignments_preserve_guarded_read() {
    assert_roster(&workload(96, 1200, 0, false), &[(4, 2)]);
}

#[test]
fn private_counting_multiblock_owner_completes_all_final_read_replay() {
    for (scalars, tail) in [(325, 167), (304, 157)] {
        assert_roster(&workload(scalars, 0, tail, false), &[(4, 2)]);
    }
}

#[test]
fn private_counting_dense_owner_keeps_reference_reassignment_kill() {
    assert_roster(&workload(96, 1200, 0, true), &[]);
}
