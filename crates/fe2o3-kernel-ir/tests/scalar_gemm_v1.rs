use fe2o3_kernel_ir::*;

fn requirements() -> ScalarGemmTargetRequirementsV1 {
    ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6()
}

#[test]
fn canonical_graph_has_one_cyclic_loop_and_exact_memory_effects() {
    let module = scalar_gemm_v1_module();
    verify_scalar_gemm_v1_module(&module, requirements()).expect("canonical GEMM");
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels[0].id.as_str(), "scalar_gemm_v1");
    assert_eq!(
        module.kernels[0].entry.as_str(),
        "__fe2o3_scalar_gemm_v1_impl"
    );
    assert_eq!(
        module.functions[0].signature.parameters,
        [
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
            Type::Scalar(ScalarType::U32),
            Type::Scalar(ScalarType::U32),
            Type::Scalar(ScalarType::U32),
        ]
    );
    let body = module.functions[0].body.as_ref().unwrap();

    assert!(matches!(
        body.blocks[3].terminator,
        Some(Terminator::Branch {
            target: BlockId(2),
            ref arguments,
        }) if arguments == &[ValueId(34), ValueId(32)]
    ));
    assert_eq!(body.blocks[2].parameters.len(), 2);

    let effects = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .flat_map(Operation::memory_effects)
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            MemoryEffect::Read(AddressSpace::Global),
            MemoryEffect::Read(AddressSpace::Global),
            MemoryEffect::Write(AddressSpace::Global),
        ]
    );
}

#[test]
fn rejects_malformed_loop_noninjective_output_and_index_truncation() {
    let mut malformed = scalar_gemm_v1_module();
    malformed.functions[0].body.as_mut().unwrap().blocks[3].terminator =
        Some(Terminator::Return { values: vec![] });
    assert_eq!(
        verify_scalar_gemm_v1_module(&malformed, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );

    let mut noninjective = scalar_gemm_v1_module();
    let c_pointer = &mut noninjective.functions[0].body.as_mut().unwrap().blocks[4].operations[0];
    let OperationKind::GetElementPointer { offset, .. } = &mut c_pointer.kind else {
        panic!("canonical C pointer");
    };
    *offset = ValueId(15);
    assert_eq!(
        verify_scalar_gemm_v1_module(&noninjective, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );

    let mut truncated = scalar_gemm_v1_module();
    truncated.functions[0].body.as_mut().unwrap().blocks[3].operations[0].kind =
        OperationKind::Cast {
            kind: CastKind::Truncate,
            value: ValueId(19),
            to: Type::INDEX,
        };
    assert_eq!(
        verify_scalar_gemm_v1_module(&truncated, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );
}

#[test]
fn rejects_reassociated_arithmetic_calls_extra_roots_and_wrong_profile() {
    let mut reassociated = scalar_gemm_v1_module();
    let OperationKind::Binary { lhs, rhs, .. } =
        &mut reassociated.functions[0].body.as_mut().unwrap().blocks[3].operations[10].kind
    else {
        panic!("canonical accumulation");
    };
    std::mem::swap(lhs, rhs);
    assert_eq!(
        verify_scalar_gemm_v1_module(&reassociated, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );

    let mut call = scalar_gemm_v1_module();
    call.functions.push(Function::external_import(
        "hidden_fma",
        Signature::new(vec![Type::F32, Type::F32, Type::F32], vec![Type::F32]),
    ));
    assert_eq!(
        verify_scalar_gemm_v1_module(&call, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );

    let wrong_target = ScalarGemmTargetRequirementsV1 {
        xnack: ScalarGemmXnackV1::Enabled,
        ..requirements()
    };
    assert_eq!(
        verify_scalar_gemm_v1_module(&scalar_gemm_v1_module(), wrong_target),
        Err(ScalarGemmV1Error::UnsupportedTargetRequirements)
    );
}

#[test]
fn validates_structure_before_exact_graph_comparison() {
    let mut invalid = scalar_gemm_v1_module();
    let duplicate_entry = invalid.functions[0].body.as_ref().unwrap().blocks[0].clone();
    invalid.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(duplicate_entry);
    assert!(matches!(
        verify_scalar_gemm_v1_module(&invalid, requirements()),
        Err(ScalarGemmV1Error::InvalidKernelIr(_))
    ));

    let mut valid_but_noncanonical = scalar_gemm_v1_module();
    valid_but_noncanonical.id = ModuleId::new("attacker-controlled-but-valid");
    assert_eq!(
        verify_scalar_gemm_v1_module(&valid_but_noncanonical, requirements()),
        Err(ScalarGemmV1Error::NonCanonicalKernelIr)
    );
}

#[test]
fn rejects_every_target_abi_symbol_and_execution_profile_mutation() {
    for wrong_target in [
        ScalarGemmTargetRequirementsV1 {
            architecture: ScalarGemmArchitectureV1::Other,
            ..requirements()
        },
        ScalarGemmTargetRequirementsV1 {
            xnack: ScalarGemmXnackV1::Enabled,
            ..requirements()
        },
        ScalarGemmTargetRequirementsV1 {
            code_object: ScalarGemmCodeObjectV1::V5,
            ..requirements()
        },
    ] {
        assert_eq!(
            verify_scalar_gemm_v1_module(&scalar_gemm_v1_module(), wrong_target),
            Err(ScalarGemmV1Error::UnsupportedTargetRequirements)
        );
    }

    let mut mutations = Vec::new();

    let mut exported_symbol = scalar_gemm_v1_module();
    exported_symbol.kernels[0].id = KernelId::new("scalar_gemm_v1_alias");
    mutations.push(exported_symbol);

    let mut internal_symbol = scalar_gemm_v1_module();
    internal_symbol.functions[0].id = FunctionId::new("scalar_gemm_v1");
    internal_symbol.kernels[0].entry = FunctionId::new("scalar_gemm_v1");
    mutations.push(internal_symbol);

    let mut abi = scalar_gemm_v1_module();
    abi.functions[0].signature.parameters.swap(2, 3);
    mutations.push(abi);

    let mut workgroup = scalar_gemm_v1_module();
    workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    mutations.push(workgroup);

    let mut wave = scalar_gemm_v1_module();
    wave.functions[0]
        .required_capabilities
        .remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
    wave.functions[0]
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
    mutations.push(wave);

    for mutation in mutations {
        assert!(
            verify_scalar_gemm_v1_module(&mutation, requirements()).is_err(),
            "profile mutation must fail closed"
        );
    }
}

#[test]
fn active_guard_dominates_division_and_u32_shapes_fit_u64_indices() {
    let module = scalar_gemm_v1_module();
    let function = &module.functions[0];
    let control_flow = analyze_control_flow(function).unwrap();
    assert!(control_flow.dominates(BlockId(0), BlockId(1)));
    assert_eq!(
        control_flow
            .predecessor_blocks(BlockId(1))
            .unwrap()
            .collect::<Vec<_>>(),
        [BlockId(0)]
    );

    let body = function.body.as_ref().unwrap();
    assert!(matches!(
        body.blocks[0].terminator,
        Some(Terminator::ConditionalBranch {
            condition: ValueId(11),
            then_target: BlockId(1),
            else_target: BlockId(5),
            ..
        })
    ));
    for (operation, expected) in body.blocks[1].operations[..2]
        .iter()
        .zip([BinaryOp::Divide, BinaryOp::Remainder])
    {
        assert!(matches!(
            operation.kind,
            OperationKind::Binary {
                op,
                lhs: ValueId(6),
                rhs: ValueId(8),
            } if op == expected
        ));
    }

    for (m, n, k) in [
        (0_u32, 0_u32, 0_u32),
        (1, 1, 0),
        (1, u32::MAX, u32::MAX),
        (u32::MAX, 1, u32::MAX),
        (u32::MAX, u32::MAX, u32::MAX),
    ] {
        let mn = u64::from(m) * u64::from(n);
        let mk = u64::from(m) * u64::from(k);
        let kn = u64::from(k) * u64::from(n);
        assert_eq!(mn as u128, u128::from(m) * u128::from(n));
        assert_eq!(mk as u128, u128::from(m) * u128::from(k));
        assert_eq!(kn as u128, u128::from(k) * u128::from(n));

        if mn > 0 {
            let p = mn - 1;
            assert_ne!(n, 0, "the active p < m*n path makes division safe");
            let row = p / u64::from(n);
            let column = p % u64::from(n);
            assert!(row < u64::from(m));
            assert!(column < u64::from(n));
            assert_eq!(row * u64::from(n) + column, p);
            if k > 0 {
                let t = u64::from(k - 1);
                assert!(row * u64::from(k) + t < mk);
                assert!(t * u64::from(n) + column < kn);
            }
        }
    }
}
