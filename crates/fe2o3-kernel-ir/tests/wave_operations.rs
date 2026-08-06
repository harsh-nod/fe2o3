use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

const GOLDEN_HEX: &str = include_str!("fixtures/wave_operations_v2.hex");

fn op(result: u32, ty: Type, kind: WaveOperationKind, width: WaveWidth) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), ty),
        OperationKind::Wave(WaveOperation::full(kind, width)),
    )
}

fn wave_module(width: WaveWidth) -> Module {
    let mask_ty = match width {
        WaveWidth::Wave32 => Type::Scalar(ScalarType::U32),
        WaveWidth::Wave64 => Type::Scalar(ScalarType::U64),
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            3,
            Type::Scalar(ScalarType::U32),
            WaveOperationKind::LaneId,
            width,
        ),
        op(
            4,
            mask_ty,
            WaveOperationKind::Ballot {
                predicate: ValueId(0),
            },
            width,
        ),
        op(
            5,
            Type::BOOL,
            WaveOperationKind::Any {
                predicate: ValueId(0),
            },
            width,
        ),
        op(
            6,
            Type::BOOL,
            WaveOperationKind::All {
                predicate: ValueId(0),
            },
            width,
        ),
        op(
            7,
            Type::Scalar(ScalarType::I32),
            WaveOperationKind::ShuffleIndex {
                value: ValueId(1),
                source_lane: ValueId(2),
                tile_width: width.lanes() / 2,
            },
            width,
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::definition(
        "wave_impl",
        Signature::new(
            vec![
                Type::BOOL,
                Type::Scalar(ScalarType::I32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    function
        .required_capabilities
        .insert(TargetCapability::WaveWidth(width));
    let mut module = Module::new("g4::wave");
    module.functions.push(function);
    module
}

fn wave_mut(module: &mut Module, operation: usize) -> &mut WaveOperation {
    let OperationKind::Wave(wave) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[operation].kind
    else {
        panic!("expected wave operation")
    };
    wave
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("unique wire marker")
}

#[test]
fn verifies_wave32_and_wave64_and_derives_exact_capabilities() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        let module = wave_module(width);
        verify_module(&module).expect("well-typed full-wave operations");
        let derived = module.derived_capabilities();
        assert_eq!(
            derived,
            BTreeSet::from([
                TargetCapability::Subgroups,
                TargetCapability::SubgroupSize(width.lanes()),
                TargetCapability::WaveWidth(width),
            ])
        );
        verify_module_with_capabilities(&module, &derived).unwrap();
    }
}

#[test]
fn operands_and_memory_effects_are_explicit_and_pure() {
    let module = wave_module(WaveWidth::Wave64);
    let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert_eq!(operations[0].kind.operands(), vec![]);
    assert_eq!(operations[1].kind.operands(), vec![ValueId(0)]);
    assert_eq!(operations[4].kind.operands(), vec![ValueId(1), ValueId(2)]);
    assert!(
        operations
            .iter()
            .all(|operation| operation.memory_effects().is_empty())
    );
}

#[test]
fn rejects_partial_lanes_and_non_subgroup_convergence_deterministically() {
    let mut module = wave_module(WaveWidth::Wave64);
    wave_mut(&mut module, 0).active_lanes = 32;
    wave_mut(&mut module, 1).convergence = Convergence::uniform(SynchronizationScope::Workgroup);
    let errors = verify_module(&module).unwrap_err();
    assert_eq!(
        errors
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .filter(|(code, _)| {
                matches!(
                    code,
                    DiagnosticCode::InvalidWaveOperation | DiagnosticCode::InvalidConvergence
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                DiagnosticCode::InvalidWaveOperation,
                "the first wave-operation subset requires all 64 lanes active, found 32",
            ),
            (
                DiagnosticCode::InvalidConvergence,
                "wave operation requires a uniform subgroup convergence claim",
            ),
        ]
    );
}

#[test]
fn rejects_wrong_predicate_mask_lane_and_shuffle_types() {
    let mut predicate = wave_module(WaveWidth::Wave32);
    if let WaveOperationKind::Ballot { predicate } = &mut wave_mut(&mut predicate, 1).kind {
        *predicate = ValueId(1);
    }
    assert!(
        verify_module(&predicate)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut mask = wave_module(WaveWidth::Wave64);
    mask.functions[0].body.as_mut().unwrap().blocks[0].operations[1].results[0].ty =
        Type::Scalar(ScalarType::U32);
    assert!(
        verify_module(&mask)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut lane = wave_module(WaveWidth::Wave32);
    if let WaveOperationKind::ShuffleIndex { source_lane, .. } = &mut wave_mut(&mut lane, 4).kind {
        *source_lane = ValueId(1);
    }
    assert!(
        verify_module(&lane)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut value = wave_module(WaveWidth::Wave32);
    if let WaveOperationKind::ShuffleIndex { value, .. } = &mut wave_mut(&mut value, 4).kind {
        *value = ValueId(0);
    }
    assert!(
        verify_module(&value)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );
}

#[test]
fn rejects_unsupported_static_tiles_and_has_no_dynamic_tile_form() {
    for tile_width in [0, 3, 128] {
        let mut module = wave_module(WaveWidth::Wave64);
        if let WaveOperationKind::ShuffleIndex {
            tile_width: tile, ..
        } = &mut wave_mut(&mut module, 4).kind
        {
            *tile = tile_width;
        }
        let errors = verify_module(&module).unwrap_err();
        assert_eq!(
            errors
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidWaveOperation)
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            vec![format!(
                "shuffle tile width {tile_width} must be a non-zero power of two no larger than 64"
            )]
        );
    }
}

#[test]
fn v2_wire_is_golden_bounded_and_v1_stays_frozen() {
    let module = wave_module(WaveWidth::Wave64);
    let bytes = encode_module_v2(&module).unwrap();
    assert_eq!(to_hex(&bytes), GOLDEN_HEX.trim());
    assert_eq!(decode_module_v2(&bytes).unwrap(), module);
    assert_eq!(encode_module_v2(&module).unwrap(), bytes);
    assert_eq!(
        encode_module_v1(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V1,
            feature: "physical wave operation",
        })
    );
    for length in 0..bytes.len() {
        assert!(
            decode_module_v2(&bytes[..length]).is_err(),
            "truncation {length}"
        );
    }
}

#[test]
fn v2_decoder_rejects_unknown_width_convergence_and_wave_tags() {
    let bytes = encode_module_v2(&wave_module(WaveWidth::Wave64)).unwrap();
    let marker = [20, 2, 64, 0, 0, 0, 1, 2, 1];
    let offset = find(&bytes, &marker);

    let mut width = bytes.clone();
    width[offset + 1] = 0xff;
    assert_eq!(
        decode_module_v2(&width),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "wave width",
            tag: 0xff,
        })
    );

    let mut convergence = bytes.clone();
    convergence[offset + 6] = 0xff;
    assert_eq!(
        decode_module_v2(&convergence),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "convergence",
            tag: 0xff,
        })
    );

    let mut operation = bytes;
    operation[offset + 8] = 0xff;
    assert_eq!(
        decode_module_v2(&operation),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "wave operation",
            tag: 0xff,
        })
    );
}
