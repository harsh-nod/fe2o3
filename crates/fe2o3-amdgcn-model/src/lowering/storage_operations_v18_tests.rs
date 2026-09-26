use super::super::storage_native_v18::tests::*;
use super::*;

#[test]
fn private_fields_share_one_backing_and_use_exact_packed_offsets() {
    let rows = vec![
        scalar(),
        Row {
            size: 8,
            alignment: 1,
            kind: Kind::Record(
                vec![Field {
                    offset: 1,
                    layout: Id(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    let module = module(
        rows,
        vec![
            allocate(0, 1, Space::Private, 1),
            constant(1, 11),
            project(2, 0, 0, Projection::Field(0), Space::Private),
            write(2, 1, Space::Private, 1),
            read(3, 2, Type::Scalar(ScalarType::U32), Space::Private, 1),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert_eq!(text.matches(" = alloca ").count(), 1);
    assert!(text.contains("%v2 = getelementptr i8, ptr addrspace(5) %v0, i64 1"));
    assert!(text.contains("store i32 11, ptr addrspace(5) %v2, align 1"));
    assert!(text.contains("%v3 = load i32, ptr addrspace(5) %v2, align 1"));
}

#[test]
fn global_object_is_an_actual_parameter_not_an_allocation() {
    let mut module = module(
        vec![scalar()],
        vec![constant(1, 19), write(0, 1, Space::Global, 4)],
        Profile::Gfx950,
    );
    module.functions[0]
        .signature
        .parameters
        .push(address(0, Space::Global));
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(0));
    let text = emit(&module, Profile::Gfx950);
    assert!(text.contains("ptr addrspace(1) %arg0"));
    assert!(text.contains("store i32 19, ptr addrspace(1) %arg0, align 4"));
    assert!(!text.contains(" = alloca "));
}

#[test]
fn static_and_dynamic_lds_use_the_existing_kernel_declaration_path() {
    for dynamic in [false, true] {
        let mut allocation = allocate(0, 0, Space::Workgroup, 4);
        if dynamic {
            let OperationKind::WorkgroupMemory(memory) = &mut allocation.kind else {
                unreachable!()
            };
            memory.extent = WorkgroupMemoryExtent::Dynamic;
        }
        let module = module(vec![scalar()], vec![allocation], Profile::Gfx942);
        let text = emit(&module, Profile::Gfx942);
        if dynamic {
            assert!(text.contains("external addrspace(3) global [0 x i8], align 4"));
        } else {
            assert!(text.contains("internal addrspace(3) global [4 x i8] undef, align 4"));
        }
        assert!(text.contains("%v0 = getelementptr i8, ptr addrspace(3) @"));
    }
}

#[test]
fn dynamic_index_check_updates_following_phi_predecessor() {
    let rows = vec![
        scalar(),
        Row {
            size: 8,
            alignment: 4,
            kind: Kind::Array {
                element: Id(0),
                length: 2,
                stride: 4,
            },
        },
    ];
    let mut module = module(
        rows,
        vec![
            allocate(0, 1, Space::Private, 4),
            value(
                1,
                Type::Scalar(ScalarType::Index),
                OperationKind::Constant(Constant::Index(1)),
            ),
            project(2, 0, 0, Projection::ArrayIndex(ValueId(1)), Space::Private),
        ],
        Profile::Gfx942,
    );
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters
        .push(ValueDef::new(ValueId(3), address(0, Space::Private)));
    next.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(next);
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("icmp ult i64 1, 2"));
    assert!(text.contains("phi ptr addrspace(5) [ %v2, %storage_bb0_op2_continue ]"));
    assert_eq!(text.matches("call void @llvm.trap()").count(), 1);
    assert!(!text.contains("getelementptr inbounds i8, ptr addrspace(5) %v0"));
}

#[test]
fn internal_helper_mutates_the_callers_same_storage_pointer() {
    let mut module = module(
        vec![scalar()],
        vec![
            allocate(0, 0, Space::Private, 4),
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("mutate"),
                    arguments: vec![ValueId(0)],
                },
            ),
            read(1, 0, Type::Scalar(ScalarType::U32), Space::Private, 4),
        ],
        Profile::Gfx942,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![constant(1, 31), write(0, 1, Space::Private, 4)];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut helper = Function::definition(
        "mutate",
        Signature::new(vec![address(0, Space::Private)], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    helper.role = FunctionRole::InternalHelper;
    module.functions.push(helper);
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("call void @mutate(ptr addrspace(5) %v0)"));
    assert!(text.contains("store i32 31, ptr addrspace(5) %arg0, align 4"));
    assert_eq!(text.matches(" = alloca ").count(), 1);
}

#[test]
fn storage_loop_uses_shared_cfg_phis_and_keeps_the_single_backing() {
    let mut module = module(
        vec![scalar()],
        vec![
            allocate(0, 0, Space::Private, 4),
            constant(1, 7),
            write(0, 1, Space::Private, 4),
        ],
        Profile::Gfx942,
    );
    let function = &mut module.functions[0];
    function
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::Bool));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(10));
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut repeat = BasicBlock::new(BlockId(1));
    repeat
        .parameters
        .push(ValueDef::new(ValueId(2), address(0, Space::Private)));
    repeat.operations = vec![
        read(3, 2, Type::Scalar(ScalarType::U32), Space::Private, 4),
        write(2, 1, Space::Private, 4),
    ];
    repeat.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(10),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(2)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(2)],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.parameters
        .push(ValueDef::new(ValueId(4), address(0, Space::Private)));
    exit.operations
        .push(read(5, 4, Type::Scalar(ScalarType::U32), Space::Private, 4));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([repeat, exit]);
    let text = emit(&module, Profile::Gfx942);
    assert_eq!(text.matches(" = alloca ").count(), 1);
    assert!(text.contains("%v2 = phi ptr addrspace(5) [ %v0, %bb0 ], [ %v2, %edge_bb1_0_bb1 ]"));
    assert!(text.contains("edge_bb1_0_bb1:\n  br label %bb1"));
    assert!(text.contains("%v4 = phi ptr addrspace(5) [ %v2, %bb1 ]"));
    assert!(text.contains("%v5 = load i32, ptr addrspace(5) %v4, align 4"));
}

#[test]
fn counted_backing_checks_nonempty_and_segment_limit_without_fabricated_bytes() {
    let mut module = module(
        vec![scalar()],
        vec![value(
            1,
            address(0, Space::Private),
            OperationKind::Alloca {
                element: object(0),
                count: Some(ValueId(0)),
                address_space: Space::Private,
                alignment: 4,
            },
        )],
        Profile::Gfx950,
    );
    module.functions[0]
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::Index));
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(0));
    let text = emit(&module, Profile::Gfx950);
    assert!(text.contains("icmp ule i64 %arg0, 1073741823"));
    assert!(text.contains("icmp ne i64 %arg0, 0"));
    assert!(text.contains(".bytes = mul i64 %arg0, 4"));
    assert!(text.contains("%v1 = alloca i8, i64 %storage.bb0.op0.bytes, align 4, addrspace(5)"));
    assert_eq!(text.matches("call void @llvm.trap()").count(), 1);
}

fn restriction_module(source: Type, target: Type, profile: Profile) -> Module {
    let mut module = module(vec![scalar(), scalar()], vec![], profile);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(value(
        1,
        target.clone(),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(0),
            to: target.clone(),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    module.functions.push(Function::definition(
        "restrict_storage",
        Signature::new(vec![source], vec![target]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn access_restriction_emits_generic_and_ordinary_storage_pointer_identity() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for (space, llvm_space) in [
            (Space::Generic, 0),
            (Space::Global, 1),
            (Space::Workgroup, 3),
            (Space::Private, 5),
        ] {
            let module = restriction_module(
                address(0, space),
                Type::pointer(object(0), space, AccessMode::ReadOnly),
                profile,
            );
            let text = emit(&module, profile);
            assert!(text.contains(&format!(
                "%v1 = select i1 true, ptr addrspace({llvm_space}) %arg0, ptr addrspace({llvm_space}) %arg0"
            )));
            assert!(text.contains(&format!("ret ptr addrspace({llvm_space}) %v1")));
            for forbidden in ["addrspacecast", "ptrtoint", "inttoptr", "load ", "store "] {
                assert!(!text.contains(forbidden), "{forbidden}: {text}");
            }
        }
    }
}

#[test]
fn access_restriction_owner_refuses_widening_row_space_and_pointee_changes() {
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as Verification,
        CanonicalKernelIrReplayAdmissionErrorV18 as Admission,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget, DiagnosticCode,
        VerifiedCanonicalKernelIrModuleV18 as Owner,
    };
    for (source, target) in [
        (
            Type::pointer(object(0), Space::Generic, AccessMode::ReadOnly),
            address(0, Space::Generic),
        ),
        (
            address(0, Space::Generic),
            Type::pointer(object(1), Space::Generic, AccessMode::ReadOnly),
        ),
        (
            address(0, Space::Generic),
            Type::pointer(object(0), Space::Global, AccessMode::ReadOnly),
        ),
        (
            address(0, Space::Generic),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                Space::Generic,
                AccessMode::ReadOnly,
            ),
        ),
    ] {
        let module = restriction_module(source, target, Profile::Gfx942);
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.reserve_storage(17).unwrap();
        let result = Owner::from_module_ref_with_verification_budget_v18(
            &module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        );
        let error = match result {
            Err(Admission::Verification(Verification::Verification(error))) => error,
            other => panic!("expected actual InvalidCast admission refusal: {other:?}"),
        };
        assert!(error.contains(DiagnosticCode::InvalidCast), "{error:?}");
        assert_eq!(budget.storage(), 17);
    }
}
