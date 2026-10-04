//! Synthetic canonical-shape controls, not actual source qualification.
use super::*;
use fe2o3_kernel_ir::*;
fn u32_ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn fixture() -> Module {
    let mut words = [0; 16];
    words[..3].copy_from_slice(&[133, 307, 413]);
    let program = Gfx942OrderedProgramV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(1), ValueId(2), ValueId(3)],
        Gfx942U32ProgramV1::from_descriptors(3, words).unwrap(),
    )
    .unwrap();
    let op = Operation::effect_free(
        ValueDef::new(ValueId(4), u32_ty()),
        OperationKind::Gfx942OrderedProgram(program),
    );
    let caps = op.required_capabilities();
    let pointer = Type::pointer(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op,
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(8),
                offset: ValueId(5),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U64)),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(7),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(10),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(2),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(1),
                arguments: vec![],
            },
        ],
        default_target: BlockId(3),
        default_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(1));
    yes.operations = vec![Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(9),
            value: ValueId(4),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )];
    yes.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });
    let mut impossible = BasicBlock::new(BlockId(3));
    impossible.terminator = Some(Terminator::Unreachable);
    let mut done = BasicBlock::new(BlockId(4));
    done.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "shape_not_a_source_name",
        Signature::new(
            vec![
                Type::slice(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite),
                u32_ty(),
                u32_ty(),
                u32_ty(),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, yes, no, impossible, done],
    );
    function.required_capabilities = caps.clone();
    let mut kernel = Kernel::new(
        "different_export_name",
        "shape_not_a_source_name",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = caps.clone();
    let mut module = Module::new("synthetic_complete_shape");
    module.required_capabilities = caps;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}
fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV17 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1 << 26);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 26);
    VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn body(module: &mut Module) -> &mut FunctionBody {
    module.functions[0].body.as_mut().unwrap()
}
#[test]
fn boolean_discriminant_zero_extension_preserves_exact_zero_one_values() {
    for scalar in [ScalarType::I64, ScalarType::U64] {
        let mut module = fixture();
        let operation = &mut body(&mut module).blocks[0].operations[6];
        let ty = Type::Scalar(scalar);
        operation.results[0].ty = ty.clone();
        let OperationKind::Cast { to, .. } = &mut operation.kind else {
            panic!()
        };
        *to = ty;
        assert!(profile::inspect(&admit(&module)).is_ok());
    }
}
#[test]
fn narrower_boolean_selector_does_not_expand_the_complete_shape() {
    let mut module = fixture();
    let operation = &mut body(&mut module).blocks[0].operations[6];
    operation.results[0].ty = u32_ty();
    let OperationKind::Cast { to, .. } = &mut operation.kind else {
        panic!()
    };
    *to = u32_ty();
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("unsupported operation or memory effect")
    );
}
#[test]
fn signed_discriminant_must_extend_the_exact_boolean_guard() {
    let mut module = fixture();
    let operation = &mut body(&mut module).blocks[0].operations[6];
    operation.results[0].ty = Type::Scalar(ScalarType::I64);
    let OperationKind::Cast { value, to, .. } = &mut operation.kind else {
        panic!()
    };
    *value = ValueId(1);
    *to = Type::Scalar(ScalarType::I64);
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("changed dataflow or store value/address")
    );
}
#[test]
fn signed_discriminant_cannot_relabel_the_true_case() {
    let mut module = fixture();
    let operation = &mut body(&mut module).blocks[0].operations[6];
    operation.results[0].ty = Type::Scalar(ScalarType::I64);
    let OperationKind::Cast { to, .. } = &mut operation.kind else {
        panic!()
    };
    *to = Type::Scalar(ScalarType::I64);
    let Some(Terminator::Switch { cases, .. }) = &mut body(&mut module).blocks[0].terminator else {
        panic!()
    };
    cases[1].value = 2;
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("missing true case")
    );
}
#[test]
fn boolean_sign_extension_is_rejected_by_canonical_admission() {
    let mut module = fixture();
    let operation = &mut body(&mut module).blocks[0].operations[6];
    operation.results[0].ty = Type::Scalar(ScalarType::I64);
    let OperationKind::Cast { kind, to, .. } = &mut operation.kind else {
        panic!()
    };
    *kind = CastKind::SignExtend;
    *to = Type::Scalar(ScalarType::I64);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1 << 26);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 26);
    assert!(
        VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
            &module,
            &mut budget
        )
        .is_err()
    );
}
#[test]
fn complete_shape_and_literal_clobber_contract() {
    let owner = admit(&fixture());
    let bytes = owner.canonical_bytes().to_vec();
    let profile = profile::inspect(&owner).unwrap();
    let llvm = emit::emit(&profile).unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(llvm.matches("call void asm sideeffect").count(), 1);
    for required in [
        "{s16},{v4},{s18},{s19}",
        "~{vcc}",
        "~{scc}",
        "~{memory}",
        "~{s20}",
        "~{s21}",
        "~{v2}",
        "~{v3}",
        "~{v32}",
        "~{v33}",
        "s_waitcnt vmcnt(0)",
        "s_and_saveexec_b64 s[20:21], vcc",
        "s_mov_b64 exec, s[20:21]",
        "v_addc_co_u32_e32 v3, vcc, v4, v3, vcc",
        "v_cmp_gt_u64_e32",
        "global_store_dword v[2:3], v33, off",
    ] {
        assert!(llvm.contains(required), "{required}");
    }
    assert!(!llvm.contains(" naked "));
    assert!(!llvm.contains("~{exec}"));
    assert!(!llvm.contains("~{v4}"));
    assert!(!llvm.contains("flat_store"));
    assert!(!llvm.contains("amdgpu-implicitarg-num-bytes"));
}
#[test]
fn changed_output_or_pointer_dataflow_is_not_source_shape() {
    let mut module = fixture();
    let OperationKind::Store { value, .. } = &mut body(&mut module).blocks[1].operations[0].kind
    else {
        panic!()
    };
    *value = ValueId(1);
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("changed dataflow or store value/address")
    );
}
#[test]
fn reversed_guard_paths_are_rejected() {
    let mut module = fixture();
    let Some(Terminator::Switch { cases, .. }) = &mut body(&mut module).blocks[0].terminator else {
        panic!()
    };
    cases[0].target = BlockId(1);
    cases[1].target = BlockId(2);
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("wrong terminal store count or return")
    );
}
#[test]
fn missing_guard_does_not_inherit_source_contract() {
    let mut module = fixture();
    body(&mut module).blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("unguarded store")
    );
}
#[test]
fn volatile_memory_effect_is_refused() {
    let mut module = fixture();
    let OperationKind::Store { access, .. } = &mut body(&mut module).blocks[1].operations[0].kind
    else {
        panic!()
    };
    access.volatile = true;
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("unsupported operation or memory effect")
    );
}
#[test]
fn extra_pure_or_effectful_operation_is_not_ignored() {
    let mut module = fixture();
    body(&mut module).blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(11), u32_ty()),
            OperationKind::Constant(Constant::U32(0)),
        ));
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("complete block/operation bound")
    );
}
#[test]
fn all_five_register_roles_are_reserved_against_the_abi_scratch() {
    for plan in [
        [0, 33, 34, 35, 36],
        [32, 3, 34, 35, 36],
        [32, 33, 4, 35, 36],
        [32, 33, 34, 6, 36],
        [32, 33, 34, 35, 7],
    ] {
        let mut module = fixture();
        let operation = &mut body(&mut module).blocks[0].operations[0];
        let OperationKind::Gfx942OrderedProgram(old) = &operation.kind else {
            panic!()
        };
        let next = Gfx942OrderedProgramV1::new(
            old.source(),
            Gfx942OrderedProgramRegistersV1::new(plan[0], plan[1], [plan[2], plan[3], plan[4]])
                .unwrap(),
            *old.inputs(),
            *old.program(),
        )
        .unwrap();
        operation.kind = OperationKind::Gfx942OrderedProgram(next);
        assert_eq!(
            profile::inspect(&admit(&module)).err(),
            Some("program collides with diagnostic ABI scratch")
        );
    }
}
#[test]
fn extra_argument_is_an_abi_change_even_if_unused() {
    let mut module = fixture();
    module.functions[0].signature.parameters.push(u32_ty());
    body(&mut module).parameters.push(ValueId(11));
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("expected writable global u32 slice and three u32 arguments")
    );
}
#[test]
fn indexed_address_must_use_actual_global_index() {
    let mut module = fixture();
    let OperationKind::GetElementPointer { offset, .. } =
        &mut body(&mut module).blocks[0].operations[5].kind
    else {
        panic!()
    };
    *offset = ValueId(6);
    assert_eq!(
        profile::inspect(&admit(&module)).err(),
        Some("changed dataflow or store value/address")
    );
}
#[test]
fn distinct_export_name_is_not_authority() {
    let mut module = fixture();
    module.kernels[0].id = KernelId::from("another_export");
    assert_eq!(
        profile::inspect(&admit(&module)).unwrap().symbol,
        "another_export"
    );
}
