// Component solver input only; the native fixture separately uses real owners.
pub(super) fn multi_entry_source_fixture_v1() -> SemanticFunctionDeclV1 {
    let base = uniform_induction_function(SemanticLocalRoleV1::Argument(0));
    let jump = |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
    projection_function_with_locals(
        vec![
            block(
                211,
                base.blocks()[0].statements().to_vec(),
                zero_switch(3, SCALAR_TYPE, 1, 2),
            ),
            block(212, vec![], jump(3)),
            block(213, vec![], jump(3)),
            block(
                214,
                base.blocks()[1].statements().to_vec(),
                zero_switch(2, SCALAR_TYPE, 5, 4),
            ),
            block(215, base.blocks()[2].statements().to_vec(), jump(3)),
            block(216, vec![], SemanticTerminatorKindV1::Return),
        ],
        base.locals().to_vec(),
    )
}

pub(super) fn distant_initializer_source_fixture_v1() -> SemanticFunctionDeclV1 {
    let base = multi_entry_source_fixture_v1();
    let mut blocks = base.blocks().to_vec();
    let jump = |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
    blocks[1] = block(212, vec![], jump(6));
    blocks[2] = block(213, vec![], jump(6));
    blocks.push(block(217, vec![], jump(3)));
    projection_function_with_locals(blocks, base.locals().to_vec())
}

pub(super) fn distant_initializer_induction_fixture_v1() -> ProjectedUniformInductionV1 {
    let function = uniform_induction_function(SemanticLocalRoleV1::Argument(0));
    let (mut inductions, _, _) = project_test_inductions(&function).unwrap();
    let mut induction = inductions.remove(0);
    induction.initializer_block = 6;
    induction.header = 3;
    induction.body_entry = 4;
    induction.latch = 4;
    induction.exit = 5;
    induction.loop_blocks = vec![3, 4];
    induction
}
