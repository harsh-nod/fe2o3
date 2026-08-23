use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_ir::*;

const MATRIX_V5_GOLDEN_HEX: &str = include_str!("fixtures/matrix_v5.hex");

fn from_hex(text: &str) -> Vec<u8> {
    let compact = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(compact.len() % 2, 0);
    compact
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("invalid golden hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn defs(first: u32, ty: Type) -> Vec<ValueDef> {
    (first..first + 4)
        .map(|id| ValueDef::new(ValueId(id), ty.clone()))
        .collect()
}

fn matrix_fixture() -> Module {
    let load = MatrixOperation {
        kind: MatrixOperationKind::LdsLoad {
            base: ValueId(11),
            profile: MatrixLdsProfile::tile_16x16_xor4_wave64(MatrixElement::Bf16),
        },
        active_lanes: 64,
        convergence: Convergence::uniform(SynchronizationScope::Subgroup),
        frontend_binding: None,
        tensor_layout: None,
    };
    let store = MatrixOperation {
        kind: MatrixOperationKind::LdsStore {
            base: ValueId(12),
            values: [ValueId(21), ValueId(22), ValueId(23), ValueId(24)],
            profile: MatrixLdsProfile::tile_16x16_xor4_wave64(MatrixElement::F32),
        },
        active_lanes: 32,
        convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        frontend_binding: None,
        tensor_layout: None,
    };
    let multiply = MatrixOperation {
        kind: MatrixOperationKind::MultiplyAccumulate {
            lhs: [ValueId(31), ValueId(32), ValueId(33), ValueId(34)],
            rhs: [ValueId(41), ValueId(42), ValueId(43), ValueId(44)],
            accumulator: [ValueId(51), ValueId(52), ValueId(53), ValueId(54)],
            profile: MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64(),
        },
        active_lanes: 64,
        convergence: Convergence::uniform(SynchronizationScope::Subgroup),
        frontend_binding: None,
        tensor_layout: None,
    };

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            defs(1_001, Type::Scalar(ScalarType::Bf16)),
            OperationKind::Matrix(load),
        ),
        Operation::new(vec![], OperationKind::Matrix(store)),
        Operation::new(defs(2_001, Type::F32), OperationKind::Matrix(multiply)),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("matrix-wire-v5");
    module.functions.push(Function::definition(
        "matrix",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn matrix_mut(module: &mut Module, index: usize) -> &mut MatrixOperation {
    match &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[index].kind {
        OperationKind::Matrix(matrix) => matrix,
        _ => panic!("fixture operation is not matrix"),
    }
}

fn matrix_offset(bytes: &[u8], active_lanes: u32, scope: u8, kind: u8) -> usize {
    let mut marker = vec![22];
    marker.extend_from_slice(&active_lanes.to_le_bytes());
    marker.extend_from_slice(&[1, scope, kind]);
    let offsets = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "matrix marker must be unique");
    offsets[0]
}

fn assert_distinct_round_trip(baseline: &[u8], module: Module) {
    let bytes = encode_module_v5(&module).expect("mutated V5 matrix module encodes");
    assert_ne!(bytes, baseline);
    assert_eq!(decode_module_v5(&bytes).unwrap(), module);
    assert_eq!(
        encode_module_v5(&decode_module_v5(&bytes).unwrap()).unwrap(),
        bytes
    );
}

fn assert_unknown_tag(mut bytes: Vec<u8>, offset: usize, tag: u8, kind: &'static str) {
    bytes[offset] = tag;
    assert_eq!(
        decode_module_v5(&bytes),
        Err(KernelIrDecodeError::UnknownTag { kind, tag })
    );
}

#[test]
fn v5_exports_domain_and_round_trips_every_matrix_variant_deterministically() {
    assert_eq!(KERNEL_IR_VERSION_V5, 5);
    assert_eq!(KERNEL_IR_DOMAIN_V5, b"FE2O3/KERNEL-IR/V5\0");

    let module = matrix_fixture();
    let first = encode_module_v5(&module).unwrap();
    let second = encode_module_v5(&module).unwrap();
    let golden = from_hex(MATRIX_V5_GOLDEN_HEX);
    assert_eq!(first, second);
    assert_eq!(first, golden);
    assert_eq!(first[8..10], KERNEL_IR_VERSION_V5.to_le_bytes());

    let decoded = decode_module_v5(&first).unwrap();
    assert_eq!(decoded, module);
    assert_eq!(decode_module_v5(&golden).unwrap(), module);
    assert_eq!(encode_module_v5(&decoded).unwrap(), first);
}

#[test]
fn exact_tiled_gemm_lds_v1_requires_v7_for_its_layout_contract() {
    let module = tiled_gemm_lds_v1_module();
    assert_eq!(
        encode_module_v5(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V5,
            feature: "tensor layout contract",
        })
    );
}

#[test]
fn matrix_operands_metadata_and_profiles_have_distinct_canonical_bytes() {
    let baseline_module = matrix_fixture();
    let baseline = encode_module_v5(&baseline_module).unwrap();

    let mut mutations = Vec::new();

    let mut module = baseline_module.clone();
    let MatrixOperationKind::LdsLoad { base, .. } = &mut matrix_mut(&mut module, 0).kind else {
        panic!()
    };
    *base = ValueId(101);
    mutations.push(module);

    let mut module = baseline_module.clone();
    let MatrixOperationKind::LdsStore { base, .. } = &mut matrix_mut(&mut module, 1).kind else {
        panic!()
    };
    *base = ValueId(102);
    mutations.push(module);

    let mut module = baseline_module.clone();
    let MatrixOperationKind::LdsStore { values, .. } = &mut matrix_mut(&mut module, 1).kind else {
        panic!()
    };
    values[3] = ValueId(103);
    mutations.push(module);

    for (field, value) in [(0, ValueId(104)), (1, ValueId(105)), (2, ValueId(106))] {
        let mut module = baseline_module.clone();
        let MatrixOperationKind::MultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            ..
        } = &mut matrix_mut(&mut module, 2).kind
        else {
            panic!()
        };
        match field {
            0 => lhs[1] = value,
            1 => rhs[2] = value,
            2 => accumulator[3] = value,
            _ => unreachable!(),
        }
        mutations.push(module);
    }

    for mutation in 0..6 {
        let mut module = baseline_module.clone();
        let MatrixOperationKind::MultiplyAccumulate { profile, .. } =
            &mut matrix_mut(&mut module, 2).kind
        else {
            panic!()
        };
        match mutation {
            0 => profile.m = 17,
            1 => profile.n = 18,
            2 => profile.k = 19,
            3 => profile.input = MatrixElement::F32,
            4 => profile.accumulator = MatrixElement::Bf16,
            5 => profile.wave_width = WaveWidth::Wave32,
            _ => unreachable!(),
        }
        mutations.push(module);
    }

    for mutation in 0..5 {
        let mut module = baseline_module.clone();
        let MatrixOperationKind::LdsLoad { profile, .. } = &mut matrix_mut(&mut module, 0).kind
        else {
            panic!()
        };
        match mutation {
            0 => profile.rows = 17,
            1 => profile.columns = 18,
            2 => profile.element = MatrixElement::F32,
            3 => profile.fragment_elements = 8,
            4 => profile.wave_width = WaveWidth::Wave32,
            _ => unreachable!(),
        }
        mutations.push(module);
    }

    let mut module = baseline_module.clone();
    matrix_mut(&mut module, 0).active_lanes = 63;
    mutations.push(module);

    let mut module = baseline_module;
    matrix_mut(&mut module, 0).convergence = Convergence::uniform(SynchronizationScope::Invocation);
    mutations.push(module);

    for module in mutations {
        assert_distinct_round_trip(&baseline, module);
    }
}

#[test]
fn malformed_matrix_and_profile_tags_are_rejected() {
    let bytes = encode_module_v5(&matrix_fixture()).unwrap();
    let load = matrix_offset(&bytes, 64, 2, 2);
    let multiply = matrix_offset(&bytes, 64, 2, 1);

    assert_unknown_tag(bytes.clone(), load, 23, "operation");
    assert_unknown_tag(bytes.clone(), load + 5, 0, "convergence");
    assert_unknown_tag(bytes.clone(), load + 6, 0, "synchronization scope");
    assert_unknown_tag(bytes.clone(), load + 7, 0, "matrix operation");
    assert_unknown_tag(bytes.clone(), load + 7, 4, "matrix operation");
    assert_unknown_tag(bytes.clone(), load + 16, 0, "matrix element");
    assert_unknown_tag(bytes.clone(), load + 17, 2, "matrix layout");
    assert_unknown_tag(bytes.clone(), load + 19, 3, "wave width");
    assert_unknown_tag(bytes.clone(), multiply + 62, 3, "matrix element");
    assert_unknown_tag(bytes.clone(), multiply + 63, 0, "matrix element");
    assert_unknown_tag(bytes, multiply + 64, 0, "wave width");
}

#[test]
fn v5_rejects_unsupported_matrix_fields_and_excessive_counts() {
    let mut bound = matrix_fixture();
    matrix_mut(&mut bound, 2).frontend_binding = Some(MatrixFrontendBindingV2 {
        observed_source: MatrixSourceAbiObservationV2 {
            provider: MatrixProviderIdentityV2 {
                crate_name: String::new(),
                stable_crate_id: 0,
                crate_hash: [0; 16],
                cargo_metadata_build_observation: [0; 32],
                source_identity: [0; 32],
                definition_identities: vec![],
            },
            canonical_record: vec![],
            digest: [0; 32],
        },
        projected_kernarg: MatrixProjectedKernargPolicyV1::canonical(),
    });
    assert_eq!(
        encode_module_v5(&bound),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V5,
            feature: "matrix frontend binding",
        })
    );

    let mut excessive = encode_module_v5(&matrix_fixture()).unwrap();
    let load = matrix_offset(&excessive, 64, 2, 2);
    let result_count = load - (4 * (4 + 2)) - 4;
    assert_eq!(
        &excessive[result_count..result_count + 4],
        &4_u32.to_le_bytes()
    );
    excessive[result_count..result_count + 4]
        .copy_from_slice(&(MAX_OPERATION_RESULTS_V1 as u32 + 1).to_le_bytes());
    assert_eq!(
        decode_module_v5(&excessive),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "operation results",
            actual: MAX_OPERATION_RESULTS_V1 + 1,
            max: MAX_OPERATION_RESULTS_V1,
        })
    );

    assert_eq!(
        decode_module_v5(&vec![0; MAX_MODULE_BYTES_V1 + 1]),
        Err(KernelIrDecodeError::TooLarge {
            max: MAX_MODULE_BYTES_V1,
        })
    );
}

#[test]
fn old_versions_reject_matrix_bytes_and_v5_accepts_legacy_bytes() {
    let module = matrix_fixture();
    for (version, encoded) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
        (KERNEL_IR_VERSION_V3, encode_module_v3(&module)),
        (KERNEL_IR_VERSION_V4, encode_module_v4(&module)),
    ] {
        assert_eq!(
            encoded,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "matrix operation",
            })
        );
    }

    let v5 = encode_module_v5(&module).unwrap();
    for decoded in [
        decode_module_v1(&v5),
        decode_module_v2(&v5),
        decode_module_v3(&v5),
        decode_module_v4(&v5),
    ] {
        assert_eq!(decoded, Err(KernelIrDecodeError::UnknownVersion(5)));
    }

    let mut forged_v4 = v5;
    forged_v4[8..10].copy_from_slice(&KERNEL_IR_VERSION_V4.to_le_bytes());
    assert_eq!(
        decode_module_v4(&forged_v4),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 22,
        })
    );

    let legacy = Module::new("legacy");
    for encoded in [
        encode_module_v1(&legacy).unwrap(),
        encode_module_v2(&legacy).unwrap(),
        encode_module_v3(&legacy).unwrap(),
        encode_module_v4(&legacy).unwrap(),
    ] {
        assert_eq!(decode_module_v5(&encoded).unwrap(), legacy);
    }
}

#[test]
fn v5_decoder_is_total_on_truncation_and_single_byte_mutations() {
    let bytes = encode_module_v5(&matrix_fixture()).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_module_v5(&bytes[..end]).is_err(), "prefix {end}");
    }

    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0x80;
        let decoded = catch_unwind(AssertUnwindSafe(|| decode_module_v5(&mutated)));
        assert!(decoded.is_ok(), "decoder panicked at byte {index}");
        if let Ok(module) = decoded.unwrap() {
            assert_eq!(encode_module_v5(&module).unwrap(), mutated);
        }
    }
}
