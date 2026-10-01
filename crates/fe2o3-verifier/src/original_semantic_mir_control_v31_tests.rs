use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_with_module_v1};

#[test]
fn original_mir_control_edge_census_pays_dead_and_return_only_block_rows() {
    let mut blocks: Vec<Option<SourceBlock>> = (0..32).map(|_| None).collect();
    blocks[7] = Some(SourceBlock {
        program: SourceProgramV30 {
            nodes: vec![],
            assignments: vec![],
            arguments: 0,
            returned: None,
            statements: 0,
            locals: vec![],
        },
        live: vec![],
        branch: Branch::Return,
        changed: vec![],
    });
    let control = SourceControl {
        blocks,
        initial: vec![],
        entry: Block::new(7),
        locals: 0,
        arguments: 0,
        types: vec![],
    };
    for limit in [32, 31] {
        let mut work = Work::new(limit);
        let floor = super::super::super::super::SOURCE_LIMIT;
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        let result = {
            let mut writer = Writer::new(&mut budget).unwrap();
            control.check_edges(None, &mut writer)
        };
        if limit == 32 {
            result.unwrap();
            assert_eq!(budget.work(), 32);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
        assert_eq!(budget.storage(), floor);
    }
}

pub(crate) fn fixture(cyclic: bool, move_selector: bool) -> (Vec<Type>, Function) {
    let (types, original) = super::super::tests::fixture(SemanticBinaryOpV1::BitXor, false);
    let source = original.source();
    let word = SemanticTypeIdV1::from_index(0);
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap();
    let copy = |local| SemanticOperandV1::Copy(place(local));
    let edge =
        |target, role| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let jump = |target| {
        SemanticTerminatorV1::new(
            source,
            Terminator::Goto(edge(target, SemanticEdgeRoleV1::Goto)),
        )
    };
    let statement = |destination, operation, left, right| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination),
                SemanticRvalueV1::new(
                    word,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: copy(left),
                        right: copy(right),
                    },
                ),
            )),
        )
    };
    let switch = |zero, otherwise, moved| {
        SemanticTerminatorV1::new(
            source,
            Terminator::SwitchInt {
                discriminant: if moved {
                    SemanticOperandV1::Move(place(1))
                } else {
                    copy(1)
                },
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(zero, SemanticEdgeRoleV1::SwitchValue),
                    )],
                    edge(otherwise, SemanticEdgeRoleV1::SwitchOtherwise),
                )
                .unwrap(),
            },
        )
    };
    let block = |ordinal, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([40 + ordinal; 32]),
            source,
            statements,
            terminator,
        )
        .unwrap()
    };
    let mut blocks = vec![
        block(0, vec![], switch(1, 2, move_selector)),
        block(
            1,
            vec![statement(3, SemanticBinaryOpV1::BitXor, 1, 2)],
            jump(3),
        ),
        block(
            2,
            vec![statement(3, SemanticBinaryOpV1::BitOr, 1, 2)],
            jump(3),
        ),
    ];
    let mut finish = vec![statement(0, SemanticBinaryOpV1::BitOr, 3, 1)];
    if cyclic {
        finish.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(1),
                SemanticRvalueV1::new(word, SemanticRvalueKindV1::Use(copy(3))),
            )),
        ));
        blocks.push(block(3, finish, switch(4, 0, false)));
        blocks.push(block(
            4,
            vec![],
            SemanticTerminatorV1::new(source, Terminator::Return),
        ));
    } else {
        blocks.push(block(
            3,
            finish,
            SemanticTerminatorV1::new(source, Terminator::Return),
        ));
    }
    let function = Function::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        source,
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn run(cyclic: bool, moved: bool, work: usize, storage: usize) -> (Result<()>, usize, usize) {
    let (types, function) = fixture(cyclic, moved);
    let (_, planned_function) = fixture(cyclic, false);
    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &planned_function,
        &types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        budget.reserve_storage(super::super::super::super::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut budget)?;
        let control = SourceControl::derive(&types, &function, plan.plan(), &mut writer)?;
        assert_eq!(control.blocks.len(), if cyclic { 5 } else { 4 });
        assert_eq!(control.entry, Block::new(0));
        assert_eq!(control.locals, 4);
        assert_eq!(control.arguments, 2);
        assert_eq!(control.initial, vec![(1, 0), (2, 1)]);
        assert_eq!(control.blocks[0].as_ref().unwrap().changed, [false; 4]);
        assert_eq!(
            control.blocks[1].as_ref().unwrap().changed,
            [false, false, false, true]
        );
        assert_eq!(
            control.blocks[2].as_ref().unwrap().changed,
            [false, false, false, true]
        );
        assert_eq!(
            control
                .blocks
                .iter()
                .flatten()
                .map(|block| block.program.assignments.len())
                .sum::<usize>(),
            if cyclic { 4 } else { 3 }
        );
        assert!(matches!(
            control.blocks[0].as_ref().unwrap().branch,
            Branch::Switch { .. }
        ));
        let merge = control.blocks[3].as_ref().unwrap();
        assert!(merge.live.iter().any(|row| row.local == 3
            && row.value
                == Value::BlockArgument {
                    block: Block::new(3),
                    variable: Variable::new(3)
                }));
        if cyclic {
            let entry = control.blocks[0].as_ref().unwrap();
            assert!(entry.live.iter().any(|row| row.local == 1
                && row.value
                    == Value::BlockArgument {
                        block: Block::new(0),
                        variable: Variable::new(1)
                    }));
        }
        Ok(())
    })();
    let work = budget.work();
    let peak = budget.peak_storage();
    budget.release_storage(budget.storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, work, peak)
}

#[test]
fn original_mir_control_frames_have_independent_field_and_envelope_oracles() {
    type ProgramFields = (
        Vec<NodeV30>,
        Vec<AssignmentV30>,
        usize,
        Option<usize>,
        usize,
        Vec<Option<usize>>,
    );
    type SourceFields = (
        Vec<Option<SourceBlock>>,
        Vec<(u32, u32)>,
        Block,
        usize,
        usize,
        Vec<ScalarV30>,
    );
    type BlockFields = (SourceProgramV30, Vec<LiveIn>, Branch, Vec<bool>);
    assert_eq!(size_of::<SourceProgramV30>(), size_of::<ProgramFields>());
    assert_eq!(size_of::<SourceControl>(), size_of::<SourceFields>());
    assert_eq!(size_of::<SourceBlock>(), size_of::<BlockFields>());
    type StatementFrame = (
        &'static mut SourceProgramV30,
        &'static [Type],
        &'static Function,
        usize,
        &'static Statement,
        &'static mut Writer<'static, 'static>,
        &'static super::super::Place,
        ScalarV30,
        usize,
        AssignmentV30,
        Result<()>,
    );
    type ReturnFrame = (
        &'static mut SourceProgramV30,
        &'static [Type],
        &'static Function,
        &'static mut Writer<'static, 'static>,
        usize,
        Option<usize>,
        Result<()>,
    );
    let interpreter = size_of::<StatementFrame>() + size_of::<ReturnFrame>();
    assert_eq!(interpreter_headers_v31(), interpreter);
    let expected = interpreter
        + size_of::<SourceFields>()
        + size_of::<Result<SourceControl>>()
        + size_of::<BlockFields>()
        + size_of::<ProgramFields>()
        + size_of::<Boundaries<'_>>()
        + size_of::<Result<Boundaries<'_>>>()
        + size_of::<Vec<Vec<Block>>>()
        + size_of::<Vec<Block>>()
        + 2 * size_of::<Vec<bool>>()
        + size_of::<Vec<ScalarV30>>()
        + size_of::<Vec<LiveIn>>()
        + size_of::<Vec<(u32, u32)>>()
        + size_of::<Vec<(u128, Block)>>()
        + size_of::<Branch>()
        + size_of::<LiveIn>()
        + size_of::<AssignmentV30>()
        + size_of::<&[Type]>()
        + size_of::<&Function>()
        + size_of::<&Plan>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Result<()>>()
        + size_of::<Variable>()
        + size_of::<(&Statement, &mut [bool], &mut Writer<'_, '_>, Result<()>)>()
        + size_of::<(&Operand, &mut [bool], Result<()>)>()
        + size_of::<Option<&CallContext<'_, '_, '_, '_>>>()
        + size_of::<(
            &[Type],
            &Function,
            &Plan,
            &CallContext<'_, '_, '_, '_>,
            &mut Writer<'_, '_>,
            Result<SourceControl>,
        )>();
    assert_eq!(headers(), expected);
}

#[test]
fn original_mir_control_interprets_each_diamond_assignment_and_merge() {
    run(false, false, 10_000_000, 64 * 1024 * 1024).0.unwrap();
}

#[test]
fn original_mir_control_interprets_loop_entry_and_each_recurrence_assignment() {
    run(true, false, 10_000_000, 64 * 1024 * 1024).0.unwrap();
}

#[test]
fn original_mir_control_rejects_moved_values_still_needed_by_a_successor() {
    for cyclic in [false, true] {
        assert!(matches!(
            run(cyclic, true, 10_000_000, 64 * 1024 * 1024).0,
            Err(Error::Statement(
                "original MIR control edge local is undefined"
            ))
        ));
    }
}

#[test]
fn original_mir_control_has_exact_and_one_short_complete_model_resources() {
    for cyclic in [false, true] {
        let measured = run(cyclic, false, 10_000_000, 64 * 1024 * 1024);
        measured.0.unwrap();
        let exact = run(cyclic, false, measured.1, measured.2);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(run(cyclic, false, measured.1 - 1, measured.2).0,
            Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(cyclic, false, measured.1, measured.2 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.2 && error.limit() == measured.2 - 1));
    }
}
