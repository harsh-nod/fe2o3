//! Synthetic structural tests only. No real rustc token, source/schema
//! admission, optimizer, simulator, LLVM or hardware evidence is claimed.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942CompleteBodyBuilderV1 as Builder,
    Gfx942CompleteBodyLabelV1 as Label, Gfx942ProgramBinaryOpcodeV1 as Opcode,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    OperationKind, Terminator,
};

fn mov(destination: Destination, source: Role) -> Instruction {
    Instruction::Move {
        destination,
        source,
    }
}
fn input(packed: Gfx942CompleteBodyPackedV1) -> CompleteBodyCanonicalInputVNext<'static> {
    CompleteBodyCanonicalInputVNext {
        origin: Gfx942CompleteBodyOriginVNext {
            root_axes: [[1; 32]; 5],
            mir_body: [2; 32],
            semantic_block: [3; 32],
            source_signature: [4; 32],
            rustc_fn_abi: [5; 32],
            frontend_bytes_sha256: [6; 32],
            raw_block: 8,
        },
        export_name: "actual_export_fixture",
        registers: Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 63]).unwrap(),
        packed,
    }
}
fn packed(blocks: &[SourceBlock<'_>]) -> Gfx942CompleteBodyPackedV1 {
    let mut builder = Builder::new();
    for block in blocks {
        builder.push_block(*block).unwrap();
    }
    builder.finish().unwrap()
}
fn one(instructions: &[Instruction]) -> Gfx942CompleteBodyPackedV1 {
    packed(&[SourceBlock {
        label: Label(251),
        instructions,
        terminator: End::GuardedStoreOutputAndEnd,
    }])
}
fn diamond(
    zero: &[Instruction],
    nonzero: &[Instruction],
    merge: &[Instruction],
) -> Gfx942CompleteBodyPackedV1 {
    packed(&[
        SourceBlock {
            label: Label(250),
            instructions: &[],
            terminator: End::BranchSelectorZero {
                zero: Label(4),
                nonzero: Label(0),
            },
        },
        SourceBlock {
            label: Label(4),
            instructions: zero,
            terminator: End::Jump(Label(7)),
        },
        SourceBlock {
            label: Label(0),
            instructions: nonzero,
            terminator: End::Jump(Label(7)),
        },
        SourceBlock {
            label: Label(7),
            instructions: merge,
            terminator: End::GuardedStoreOutputAndEnd,
        },
    ])
}
fn build(input: &CompleteBodyCanonicalInputVNext<'_>) -> PendingCompleteBodyMaterializationVNext {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(73).unwrap();
    let value = materialize_complete_body_function_vnext(input, &mut budget).unwrap();
    assert_eq!(budget.storage(), 73);
    assert_eq!(budget.work(), MATERIALIZATION_WORK);
    value
}
fn blocks(owner: &PendingCompleteBodyMaterializationVNext) -> &[BasicBlock] {
    &owner.function().body.as_ref().unwrap().blocks
}
fn failure(input: &CompleteBodyCanonicalInputVNext<'_>) -> Error {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(91).unwrap();
    let error = materialize_complete_body_function_vnext(input, &mut budget)
        .err()
        .unwrap();
    assert_eq!(budget.storage(), 91);
    error
}

#[test]
fn one_block_has_one_ssa_body_and_real_guarded_memory_effect() {
    let owner = build(&input(one(&[mov(Destination::Output, Role::Input0)])));
    assert!(!owner.grants_artifact_or_launch_authority());
    assert_eq!(owner.source_labels(), &[251]);
    assert_eq!(owner.instruction_count(), 1);
    assert_eq!(owner.function().role, FunctionRole::KernelEntry);
    assert_eq!(owner.function().signature.parameters.len(), 5);
    assert!(owner.function().signature.results.is_empty());
    let [block] = blocks(&owner) else {
        panic!("one body block required");
    };
    assert_eq!(block.id, BlockId(0));
    assert!(block.parameters.is_empty());
    assert_eq!(block.operations.len(), 8);
    assert!(matches!(
        block.operations[0].kind,
        OperationKind::Gfx942CompleteBodyDeclaration(_)
    ));
    let OperationKind::Gfx942CompleteBodyStep(step) = block.operations[1].kind else {
        panic!("step missing");
    };
    assert_eq!(step.operands, [Some(ValueId(1)), None]);
    assert_eq!(step.authored_instruction, 0);
    assert!(
        matches!(block.operations[7].kind, OperationKind::GuardedStore { access, .. }
        if access.alignment == 4 && access.address_space == fe2o3_kernel_ir::AddressSpace::Global)
    );
    assert_eq!(
        block.terminator,
        Some(Terminator::Return { values: vec![] })
    );
}

#[test]
fn diamond_joins_scratch_with_actual_block_parameters_and_edge_arguments() {
    let packed = diamond(
        &[mov(Destination::Scratch, Role::Input0)],
        &[mov(Destination::Scratch, Role::Input1)],
        &[Instruction::Binary {
            opcode: Opcode::Xor,
            destination: Destination::Output,
            left: Role::Scratch,
            right: Role::Input2,
        }],
    );
    let owner = build(&input(packed));
    let body = blocks(&owner);
    assert_eq!(body.len(), 4);
    assert_eq!(owner.source_labels(), &[250, 4, 0, 7]);
    assert_eq!(body[3].parameters.len(), 1);
    let merge_value = body[3].parameters[0].id;
    for arm in [1, 2] {
        let result = body[arm].operations[0].results[0].id;
        assert_eq!(
            body[arm].terminator,
            Some(Terminator::Branch {
                target: BlockId(3),
                arguments: vec![result],
            })
        );
    }
    let OperationKind::Gfx942CompleteBodyStep(step) = body[3].operations[0].kind else {
        panic!("merge step");
    };
    assert_eq!(step.operands, [Some(merge_value), Some(ValueId(3))]);
    assert_eq!(step.authored_block, 3);
    assert_eq!(step.authored_instruction, 2);
    let OperationKind::Compare {
        predicate,
        lhs,
        rhs,
    } = body[0].operations[2].kind
    else {
        panic!("selector");
    };
    assert_eq!(predicate, fe2o3_kernel_ir::ComparePredicate::Equal);
    assert_eq!(lhs, ValueId(4));
    assert_eq!(rhs, body[0].operations[1].results[0].id);
    assert!(
        matches!(body[0].terminator, Some(Terminator::ConditionalBranch {
        then_target: BlockId(1), else_target: BlockId(2), ref then_arguments, ref else_arguments, ..
    }) if then_arguments.is_empty() && else_arguments.is_empty())
    );
}

#[test]
fn both_output_definitions_join_before_the_one_guarded_store() {
    let owner = build(&input(diamond(
        &[mov(Destination::Output, Role::Input0)],
        &[mov(Destination::Output, Role::Input1)],
        &[],
    )));
    let body = blocks(&owner);
    assert_eq!(body[3].parameters.len(), 1);
    let output = body[3].parameters[0].id;
    assert!(
        matches!(body[3].operations[5].kind, OperationKind::GuardedStore { value, .. } if value == output)
    );
    assert_eq!(
        body.iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::GuardedStore { .. }))
            .count(),
        1
    );
}

#[test]
fn merge_drops_an_arm_only_definition_and_never_fabricates_zero() {
    let left = [
        mov(Destination::Scratch, Role::Input0),
        mov(Destination::Output, Role::Input0),
    ];
    let right = [mov(Destination::Output, Role::Input1)];
    let owner = build(&input(diamond(&left, &right, &[])));
    assert_eq!(blocks(&owner)[3].parameters.len(), 1);
    let reading = [mov(Destination::Output, Role::Scratch)];
    assert_eq!(
        failure(&input(diamond(&left, &right, &reading))),
        Error::UndefinedRole
    );
}

#[test]
fn maximum_sixteen_authored_steps_keep_dead_and_self_writes_and_ordinal() {
    let mut instructions = [mov(Destination::Output, Role::Input0); 16];
    instructions[3] = mov(Destination::Scratch, Role::Input2); // unused physical role write
    instructions[8] = mov(Destination::Output, Role::Output);
    let owner = build(&input(one(&instructions)));
    let steps: Vec<_> = blocks(&owner)[0]
        .operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Gfx942CompleteBodyStep(step) => Some(step),
            _ => None,
        })
        .collect();
    assert_eq!(steps.len(), 16);
    for (ordinal, step) in steps.iter().enumerate() {
        assert_eq!(step.authored_instruction, ordinal as u8);
        assert_eq!(step.instruction, instructions[ordinal]);
    }
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) =
        blocks(&owner)[0].operations[0].kind
    else {
        panic!("declaration");
    };
    assert_eq!(declaration.registers.inputs(), [34, 35, 63]);
    assert_eq!(declaration.instruction_count, 16);
}

#[test]
fn unsupported_cfg_shapes_and_duplicate_labels_refuse_without_fallback() {
    let instructions = [mov(Destination::Output, Role::Input0)];
    let sequence = packed(&[
        SourceBlock {
            label: Label(1),
            instructions: &instructions,
            terminator: End::Jump(Label(2)),
        },
        SourceBlock {
            label: Label(2),
            instructions: &[],
            terminator: End::GuardedStoreOutputAndEnd,
        },
    ]);
    assert_eq!(failure(&input(sequence)), Error::UnsupportedShape);
    let duplicate = packed(&[
        SourceBlock {
            label: Label(1),
            instructions: &instructions,
            terminator: End::Jump(Label(1)),
        },
        SourceBlock {
            label: Label(1),
            instructions: &[],
            terminator: End::GuardedStoreOutputAndEnd,
        },
    ]);
    assert_eq!(failure(&input(duplicate)), Error::DuplicateLabel);
}

#[test]
fn absent_output_and_read_before_definition_refuse() {
    assert_eq!(
        failure(&input(one(&[mov(Destination::Scratch, Role::Input0)]))),
        Error::UndefinedRole
    );
    assert_eq!(
        failure(&input(one(&[mov(Destination::Output, Role::Scratch)]))),
        Error::UndefinedRole
    );
    assert_eq!(
        failure(&input(diamond(
            &[mov(Destination::Output, Role::Input0)],
            &[mov(Destination::Scratch, Role::Input1)],
            &[]
        ))),
        Error::UndefinedRole
    );
}

#[test]
fn unknown_source_identity_export_and_reserved_role_are_closed() {
    let mut value = input(one(&[mov(Destination::Output, Role::Input0)]));
    value.origin.root_axes[3] = [0; 32];
    assert_eq!(failure(&value), Error::IncompleteOrigin);
    value.origin.root_axes[3] = [1; 32];
    value.export_name = "bad symbol";
    assert_eq!(failure(&value), Error::ExportName);
    value.export_name = "entry";
    value.registers = Gfx942OrderedProgramRegistersV1::new(7, 33, [34, 35, 36]).unwrap();
    assert_eq!(failure(&value), Error::Registers);
}

#[test]
fn exact_reconstruction_rejects_changed_full_identity_not_only_digest_claims() {
    let mut input = input(one(&[mov(Destination::Output, Role::Input0)]));
    let owner = build(&input);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let floor = 37 + owner.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    owner.verify_reconstruction(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    input.origin.root_axes[4] = [99; 32];
    assert_eq!(
        owner.verify_reconstruction(&input, &mut budget).err(),
        Some(Error::InternalAccounting)
    );
    assert_eq!(budget.storage(), floor);
    input.origin.root_axes[4] = [1; 32];
    input.packed = one(&[mov(Destination::Output, Role::Input1)]);
    assert_eq!(
        owner.verify_reconstruction(&input, &mut budget).err(),
        Some(Error::InternalAccounting)
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn work_failure_precedes_allocation_and_preserves_nonzero_incoming_floor() {
    let input = input(one(&[mov(Destination::Output, Role::Input0)]));
    let mut work = Work::new(MATERIALIZATION_WORK - 1);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    assert!(matches!(
        materialize_complete_body_function_vnext(&input, &mut budget).err(),
        Some(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.peak_storage(), 19);
    assert_eq!(budget.work(), 0);
}

#[test]
fn exact_storage_boundary_and_one_byte_short_preserve_the_caller_floor() {
    let input = input(one(&[mov(Destination::Output, Role::Input0)]));
    let needed = build(&input).storage().retained_storage();
    for (limit, succeeds) in [(needed + 23, true), (needed + 22, false), (23, false)] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(23).unwrap();
        let actual = materialize_complete_body_function_vnext(&input, &mut budget);
        assert_eq!(actual.is_ok(), succeeds);
        assert_eq!(budget.storage(), 23);
        if !succeeds {
            assert!(matches!(
                actual.err(),
                Some(Error::Resource(Resource::Storage(_)))
            ));
        }
    }
}

#[test]
fn logical_scope_rolls_back_on_unwind_without_losing_parent_reservation() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(17).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut scope = Scope::new(&mut budget);
        let _temporary = scope.vec::<u64>(31).unwrap();
        panic!("test-only admitted allocation unwind");
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17 + 31 * 8);
}
