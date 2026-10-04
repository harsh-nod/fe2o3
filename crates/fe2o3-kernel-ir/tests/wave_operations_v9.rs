use fe2o3_kernel_ir::*;

fn storage_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    )
}

fn wave_operation_module() -> Module {
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
        "wave_operations_impl",
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
        "wave_operations",
        "wave_operations_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tests::wave_operations_v9");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn masked_broadcast_module(mask: Option<u32>, mask_on_left: bool) -> Module {
    let mut module = wave_operation_module();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let mut broadcast = block.operations.pop().expect("broadcast operation");
    let mask_value = ValueId(22);
    let masked_value = ValueId(23);
    if let Some(mask) = mask {
        block.operations.push(Operation::effect_free(
            ValueDef::new(mask_value, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(mask)),
        ));
    }
    let dynamic = ValueId(11);
    let other = mask.map_or(ValueId(12), |_| mask_value);
    let (lhs, rhs) = if mask_on_left {
        (other, dynamic)
    } else {
        (dynamic, other)
    };
    block.operations.push(Operation::effect_free(
        ValueDef::new(masked_value, Type::Scalar(ScalarType::U32)),
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        },
    ));
    let OperationKind::Wave(wave) = &mut broadcast.kind else {
        panic!("expected broadcast")
    };
    let WaveOperationKind::BroadcastF32 {
        source_lane,
        tile_width,
        ..
    } = &mut wave.kind
    else {
        panic!("expected f32 broadcast")
    };
    *source_lane = masked_value;
    *tile_width = 64;
    block.operations.push(broadcast);
    module
}

#[test]
fn generic_wave_operation_surface_round_trips_only_as_v9() {
    let module = wave_operation_module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v9(&module).unwrap();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V9.to_le_bytes());
    assert_eq!(decode_module_v9(&bytes).unwrap(), module);
    assert!(encode_module_v8(&module).is_err());
    let owner = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
}

#[test]
fn transpose_rejects_wrong_workgroup_and_bypassed_publish() {
    let mut wrong_workgroup = wave_operation_module();
    wrong_workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    assert!(
        verify_module(&wrong_workgroup)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGfx950LdsTranspose)
    );

    let mut bypassed = wave_operation_module();
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
    let mut out_of_tile = wave_operation_module();
    out_of_tile.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind =
        OperationKind::Constant(Constant::U32(16));
    assert!(
        verify_module(&out_of_tile)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidWaveOperation)
    );

    let mut dynamic = wave_operation_module();
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

#[test]
fn broadcast_accepts_direct_mask63_and_rejects_mask64_or_nonconstant_mask() {
    verify_module(&masked_broadcast_module(Some(63), false)).unwrap();
    verify_module(&masked_broadcast_module(Some(63), true)).unwrap();
    for hostile in [
        masked_broadcast_module(Some(64), false),
        masked_broadcast_module(None, false),
    ] {
        assert!(
            verify_module(&hostile)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidWaveOperation)
        );
    }
}
