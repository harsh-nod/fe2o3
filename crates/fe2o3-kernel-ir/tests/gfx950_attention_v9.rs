use fe2o3_kernel_ir::*;

fn storage_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    )
}

fn attention_module() -> Module {
    let parameters = vec![
        Type::slice(
            Type::Scalar(ScalarType::U8),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::Scalar(ScalarType::F32),
    ];
    let parameter_ids = (0..parameters.len() as u32)
        .map(ValueId)
        .collect::<Vec<_>>();
    let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
    let transpose = |result: u32, kind| {
        Operation::new(
            vec![ValueDef::new(ValueId(result), storage_type())],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(kind)),
        )
    };
    let operations = vec![
        transpose(8, Gfx950LdsTransposeOperationKindV1::Current { format }),
        transpose(
            9,
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage: ValueId(8),
                source_slice: ValueId(0),
                offset: ValueId(1),
                rows: ValueId(2),
                columns: ValueId(3),
                stride: ValueId(4),
                token_base: ValueId(5),
                reduction_base: ValueId(6),
            },
        ),
        transpose(
            10,
            Gfx950LdsTransposeOperationKindV1::Publish {
                format,
                storage: ValueId(9),
            },
        ),
        Operation::new(
            (11..19)
                .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
                .collect(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Read {
                    format,
                    storage: ValueId(10),
                },
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(19), Type::Scalar(ScalarType::F32)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(7),
                    tile_width: 16,
                    kind: WaveF32ReductionKindV1::Maximum,
                },
                WaveWidth::Wave64,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::Scalar(ScalarType::F32)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::BroadcastF32 {
                    value: ValueId(19),
                    source_lane: ValueId(20),
                    tile_width: 16,
                },
                WaveWidth::Wave64,
            )),
        ),
    ];
    let mut function = Function::kernel_entry(
        "gfx950_attention_impl",
        Signature::new(parameters, vec![]),
        parameter_ids,
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "gfx950_attention",
        "gfx950_attention_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tests::gfx950_attention_v9");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn exact_attention_surface_round_trips_only_as_v9() {
    let module = attention_module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v9(&module).unwrap();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V9.to_le_bytes());
    assert_eq!(decode_module_v9(&bytes).unwrap(), module);
    assert!(encode_module_v8(&module).is_err());
    let owner = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
}

#[test]
fn wrong_workgroup_and_bypassed_publish_are_rejected() {
    let mut wrong_workgroup = attention_module();
    wrong_workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    assert!(
        verify_module(&wrong_workgroup)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGfx950LdsTranspose)
    );

    let mut bypassed = attention_module();
    let OperationKind::Gfx950LdsTranspose(read) =
        &mut bypassed.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        panic!("expected transpose read")
    };
    read.kind = Gfx950LdsTransposeOperationKindV1::Read {
        format: Gfx950LdsTransposeFormatV1::Fp8E4M3,
        storage: ValueId(9),
    };
    assert!(
        verify_module(&bypassed)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGfx950LdsTranspose)
    );
}

#[test]
fn broadcast_rejects_an_out_of_tile_or_dynamic_source_lane() {
    let mut out_of_tile = attention_module();
    out_of_tile.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind =
        OperationKind::Constant(Constant::U32(16));
    assert!(
        verify_module(&out_of_tile)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidWaveOperation)
    );

    let mut dynamic = attention_module();
    let OperationKind::Wave(wave) =
        &mut dynamic.functions[0].body.as_mut().unwrap().blocks[0].operations[6].kind
    else {
        panic!("expected broadcast")
    };
    let WaveOperationKind::BroadcastF32 { source_lane, .. } = &mut wave.kind else {
        panic!("expected f32 broadcast")
    };
    *source_lane = ValueId(6);
    assert!(
        verify_module(&dynamic)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidWaveOperation)
    );
}
