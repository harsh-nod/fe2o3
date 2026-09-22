use super::*;
use crate::*;

const FLOOR: usize = 31 + size_of::<TensorLayoutContractV1>();

fn literal() -> (TensorLayoutContractV1, Vec<u8>) {
    let fragment = |role| TensorFragmentLayoutV1 {
        role,
        shape: [0x0101, 0x0202],
        element: MatrixElement::Bf16,
        fragment_elements: 7,
        mapping: TensorSymbolicMapV1::Opaque(0x5566_7788),
        multiplicity: TensorMultiplicityV1::Broadcast { factor: 17 },
        packing: TensorElementPackingV1::Unsupported(0x33),
        lds_swizzle: TensorLdsSwizzleV1::Unsupported(0x44),
    };
    let contract = TensorLayoutContractV1 {
        profile: TensorInstructionProfileV1::Opaque(0x1122_3344),
        subgroup_width: 0x5566,
        a: fragment(TensorOperandRoleV1::A),
        b: fragment(TensorOperandRoleV1::B),
        accumulator: fragment(TensorOperandRoleV1::Accumulator),
        tail_mask: TensorTailMaskV1::Unsupported(0xaa),
    };
    let mut bytes = vec![3, 0x44, 0x33, 0x22, 0x11, 0x66, 0x55];
    for role in [1, 2, 3] {
        bytes.extend_from_slice(&[
            role, 1, 1, 2, 2, 1, 7, 2, 0x88, 0x77, 0x66, 0x55, 2, 17, 3, 0x33, 3, 0x44,
        ]);
    }
    bytes.extend_from_slice(&[5, 0xaa]);
    (contract, bytes)
}

#[test]
fn raw_tensor_payload_preserves_literal_bytes_including_module_length_offsets() {
    let (contract, expected) = literal();
    assert_ne!(&expected[12..16], &(expected.len() as u32).to_le_bytes());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, 10_000);
    budget.reserve_storage(FLOOR).unwrap();
    let (encoded, receipt) = encode_tensor_layout_payload_v12(contract, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(encoded.canonical_bytes(), expected);
    let encoded_floor = budget.storage();
    let (decoded, decoded_storage) =
        decode_tensor_layout_payload_v12(encoded.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(budget.storage(), encoded_floor);
    budget
        .reserve_storage(decoded_storage.retained_storage())
        .unwrap();
    assert_eq!(decoded, contract);
    budget
        .release_storage(decoded_storage.retained_storage())
        .unwrap();
    drop(encoded);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn tensor_payload_uses_v12_tags_and_rejects_incomplete_or_trailing_input() {
    let mut contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    for (profile, expected_tag) in [
        (
            TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64,
            1,
        ),
        (TensorInstructionProfileV1::IncompatibleWave32, 2),
        (TensorInstructionProfileV1::Opaque(19), 3),
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64,
            4,
        ),
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64,
            5,
        ),
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64,
            6,
        ),
    ] {
        contract.profile = profile;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(FLOOR).unwrap();
        let (encoded, receipt) = encode_tensor_layout_payload_v12(contract, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(encoded.canonical_bytes()[0], expected_tag);
        let (decoded, decoded_storage) =
            decode_tensor_layout_payload_v12(encoded.canonical_bytes(), &mut budget).unwrap();
        budget
            .reserve_storage(decoded_storage.retained_storage())
            .unwrap();
        assert_eq!(decoded, contract);
        budget
            .release_storage(decoded_storage.retained_storage())
            .unwrap();
        for end in 0..encoded.canonical_bytes().len() {
            assert!(
                decode_tensor_layout_payload_v12(&encoded.canonical_bytes()[..end], &mut budget)
                    .is_err()
            );
        }
    }
    let (_, mut bytes) = literal();
    bytes.push(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, 10_000);
    budget
        .reserve_storage(size_of::<Vec<u8>>() + bytes.capacity())
        .unwrap();
    assert!(matches!(
        decode_tensor_layout_payload_v12(&bytes, &mut budget),
        Err(E::Decode(KernelIrDecodeError::TrailingBytes))
    ));
    bytes.pop();
    bytes[0] = 0;
    assert!(matches!(
        decode_tensor_layout_payload_v12(&bytes, &mut budget),
        Err(E::Decode(KernelIrDecodeError::UnknownTag {
            kind: "tensor instruction profile",
            tag: 0
        }))
    ));
}

#[test]
fn tensor_payload_exact_new_work_and_storage_prefixes_are_independent() {
    let (contract, bytes) = literal();
    // Profile tag/id, subgroup, 13 fields per fragment, and tail tag/code.
    let tokens = 2 + 1 + 3 * 13 + 2;
    let encode_work = 2 + tokens + bytes.len();
    let decode_work = 2 + bytes.len() + bytes.len() + tokens + 1;
    let header =
        size_of::<InertTensorLayoutPayloadV12>() + size_of::<TensorLayoutPayloadStorageV12>();
    let prefix = FLOOR + header + size_of::<Writer<'_>>();
    let requested_peak = prefix + bytes.len();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(encode_work);
    let mut budget = Budget::new(&mut work, 10_000);
    budget.reserve_storage(FLOOR).unwrap();
    let (encoded, storage) = encode_tensor_layout_payload_v12(contract, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.work(), encode_work);
    assert_eq!(budget.peak_storage(), prefix + encoded.bytes.capacity());
    assert_eq!(
        storage.retained_storage(),
        header + encoded.bytes.capacity()
    );
    drop(encoded);
    budget.release_storage(storage.retained_storage()).unwrap();

    let mut work = CanonicalKernelIrWorkBudgetV1::new(encode_work);
    let mut budget = Budget::new(&mut work, requested_peak - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let Err(E::Resource(Resource::Storage(error))) =
        encode_tensor_layout_payload_v12(contract, &mut budget)
    else {
        panic!("exact byte reservation must refuse");
    };
    assert_eq!(
        (error.actual(), error.limit()),
        (requested_peak, requested_peak - 1)
    );
    assert_eq!(budget.work(), 2 + tokens);
    assert_eq!(budget.peak_storage(), prefix);
    assert_eq!(budget.failed_storage(), Some(requested_peak));
    assert_eq!(budget.storage(), FLOOR);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(decode_work);
    let mut budget = Budget::new(&mut work, 10_000);
    let backing = size_of::<Vec<u8>>() + bytes.capacity();
    budget.reserve_storage(backing).unwrap();
    let (decoded, decoded_storage) = decode_tensor_layout_payload_v12(&bytes, &mut budget).unwrap();
    budget
        .reserve_storage(decoded_storage.retained_storage())
        .unwrap();
    assert_eq!(decoded, contract);
    assert_eq!(budget.work(), decode_work);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(decode_work - 1);
    let mut budget = Budget::new(&mut work, 10_000);
    budget.reserve_storage(backing).unwrap();
    assert!(matches!(
        decode_tensor_layout_payload_v12(&bytes, &mut budget),
        Err(E::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), backing);
    assert_eq!(budget.work(), decode_work - 1);
}

#[test]
fn tensor_payload_errors_borrow_the_real_typed_child() {
    let error = E::Resource(Resource::Arithmetic);
    let source = error.source().unwrap().downcast_ref::<Resource>().unwrap();
    let E::Resource(child) = &error else {
        unreachable!()
    };
    assert!(std::ptr::eq(source, child));
    assert!(E::Length.source().is_none());
    assert!(E::NonCanonical.source().is_none());
    assert!(E::Panicked.source().is_none());
}

fn tensor_layout_fixture() -> Module {
    let parameters = vec![Type::F32, Type::F32, Type::F32, Type::F32];
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let allocation = |id: u32, element: Type| {
        Operation::new(
            vec![ValueDef::new(
                ValueId(id),
                Type::pointer(
                    element.clone(),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element,
                extent: WorkgroupMemoryExtent::Static(256),
                alignment: 16,
            }),
        )
    };
    let load_a = MatrixOperation::lds_load(ValueId(4), MatrixElement::Bf16);
    let load_b = MatrixOperation::lds_load(ValueId(5), MatrixElement::Bf16);
    let mma = MatrixOperation::multiply_accumulate(
        [ValueId(7), ValueId(8), ValueId(9), ValueId(10)],
        [ValueId(11), ValueId(12), ValueId(13), ValueId(14)],
        [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
    )
    .with_declared_tensor_layout(TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64());
    let store = MatrixOperation::lds_store(
        ValueId(6),
        [ValueId(15), ValueId(16), ValueId(17), ValueId(18)],
        MatrixElement::F32,
    );
    let mut next = 7;
    let mut matrix_op = |matrix: MatrixOperation| {
        let results = matrix
            .result_types()
            .into_iter()
            .map(|ty| {
                let result = ValueDef::new(ValueId(next), ty);
                next += 1;
                result
            })
            .collect();
        Operation::new(results, OperationKind::Matrix(matrix))
    };
    let operations = vec![
        allocation(4, Type::Scalar(ScalarType::Bf16)),
        allocation(5, Type::Scalar(ScalarType::Bf16)),
        allocation(6, Type::F32),
        matrix_op(load_a),
        matrix_op(load_b),
        matrix_op(mma),
        matrix_op(store),
    ];
    let mut function = Function::kernel_entry(
        "layout_impl",
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
        "layout",
        "layout_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tensor-layout-wire-v7");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn raw_payload_agrees_with_independent_fields_inside_a_real_matrix_module() {
    use crate::*;
    let module = tensor_layout_fixture();
    let encoded_module = encode_module_v12(&module).unwrap();
    let decoded_module = decode_module_v12(&encoded_module).unwrap();
    assert_eq!(decoded_module, module);
    // Complete manually specified GFX942 payload, including both affine axes.
    let mut expected = vec![1, 64, 0];
    for (role, element, packing, axes) in [
        (1u8, 1u8, 1u8, [[0u16, 1, 0, 0], [0, 0, 4, 1]]),
        (2, 1, 1, [[0, 0, 4, 1], [0, 1, 0, 0]]),
        (3, 2, 2, [[0, 0, 4, 1], [0, 1, 0, 0]]),
    ] {
        expected.extend_from_slice(&[role, 16, 0, 16, 0, element, 4, 1, 16, 0, 16, 0]);
        for axis in axes {
            for coefficient in axis {
                expected.extend_from_slice(&coefficient.to_le_bytes());
            }
            expected.push(1);
        }
        expected.extend_from_slice(&[1, packing, 1]);
    }
    expected.push(1);
    let locations = encoded_module
        .windows(expected.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == expected).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        locations.len(),
        1,
        "one declared tensor payload in this exact module"
    );
    let body = decoded_module.functions[0]
        .body
        .as_ref()
        .expect("decoded fixture body");
    let OperationKind::Matrix(matrix) = &body.blocks[0].operations[5].kind else {
        panic!("matrix slot");
    };
    let contract = matrix.tensor_layout.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let (raw, receipt) = encode_tensor_layout_payload_v12(contract, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(raw.canonical_bytes(), expected);
    assert_eq!(
        &encoded_module[locations[0]..locations[0] + expected.len()],
        raw.canonical_bytes()
    );
}

#[test]
fn every_tensor_inner_alternative_and_exact_maximum_remain_closed_syntax() {
    let mut cases = vec![
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
        literal().0,
    ];
    for tail_mask in [
        TensorTailMaskV1::ExactPhysicalTile,
        TensorTailMaskV1::ZeroFilledPredicateInputs,
        TensorTailMaskV1::PredicateMask,
        TensorTailMaskV1::Missing,
        TensorTailMaskV1::Unsupported(255),
    ] {
        let mut value = cases[0];
        value.tail_mask = tail_mask;
        cases.push(value);
    }
    for lds_swizzle in [
        TensorLdsSwizzleV1::None,
        TensorLdsSwizzleV1::Xor4,
        TensorLdsSwizzleV1::Unsupported(255),
    ] {
        let mut value = cases[0];
        value.a.lds_swizzle = lds_swizzle;
        cases.push(value);
    }
    let mut largest = literal().0;
    let mapping = TensorSymbolicMapV1::LaneComponentAffine {
        lane_modulus: u16::MAX,
        lane_divisor: u16::MAX,
        axes: [TensorCoordinateExprV1 {
            constant: u16::MAX,
            lane_mod_scale: u16::MAX,
            lane_div_scale: u16::MAX,
            component_scale: u16::MAX,
            tile_origin: true,
        }; 2],
    };
    largest.a.mapping = mapping;
    largest.b.mapping = mapping;
    largest.accumulator.mapping = mapping;
    cases.push(largest);
    for contract in cases {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let (bytes, receipt) = encode_tensor_layout_payload_v12(contract, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (decoded, storage) =
            decode_tensor_layout_payload_v12(bytes.canonical_bytes(), &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(decoded, contract);
        if contract == largest {
            assert_eq!(bytes.canonical_bytes().len(), 117);
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert!(matches!(
        decode_tensor_layout_payload_v12(&[0; 118], &mut budget),
        Err(E::Length)
    ));
}
