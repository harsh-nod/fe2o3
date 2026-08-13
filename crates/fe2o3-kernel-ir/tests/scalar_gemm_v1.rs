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
    reassociated.functions[0].body.as_mut().unwrap().blocks[3]
        .operations
        .swap(9, 10);
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
