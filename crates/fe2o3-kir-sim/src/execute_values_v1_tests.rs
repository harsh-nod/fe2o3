use super::*;
use fe2o3_kernel_ir::{Signature, TargetCapability};
use std::collections::BTreeSet;

fn function(name: &str, ids: &[u32], types: Vec<Type>) -> Function {
    Function {
        id: name.into(),
        signature: Signature::new(types, vec![]),
        role: FunctionRole::InternalHelper,
        body: Some(fe2o3_kernel_ir::FunctionBody {
            parameters: ids.iter().copied().map(ValueId).collect(),
            blocks: vec![BasicBlock {
                id: BlockId(0),
                parameters: vec![],
                operations: vec![],
                terminator: Some(Terminator::Return { values: vec![] }),
            }],
        }),
        required_capabilities: BTreeSet::<TargetCapability>::new(),
    }
}
fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("column-values");
    module.functions = functions;
    module
}
fn target() -> SimulationTargetV1 {
    SimulationTargetV1::amdgpu_64()
}
fn columns<'a>(
    layout: &'a ValueLayout<'a>,
    plan: FrameValuePlan,
    target: SimulationTargetV1,
) -> RuntimeValues<'a> {
    RuntimeValues::prepared(Some(layout), plan, target, 0).unwrap()
}
fn payload_bytes(values: &RuntimeValues<'_>) -> usize {
    let RuntimeValues::Columns {
        scalars,
        present,
        others,
        ..
    } = values
    else {
        panic!("columns")
    };
    scalars.capacity() * size_of::<u128>()
        + present.capacity() * size_of::<bool>()
        + others.capacity() * size_of::<Option<RuntimeValue>>()
}

#[test]
fn scalar_columns_round_trip_all_admitted_bits_including_nan_signed_zero_and_u128() {
    let cases = [
        (ScalarType::Bool, 1),
        (ScalarType::I8, 0xff),
        (ScalarType::U8, 0xa5),
        (ScalarType::I16, 0x8001),
        (ScalarType::U16, 0xffff),
        (ScalarType::I32, 0x8000_0001),
        (ScalarType::U32, 0xffff_ffff),
        (ScalarType::I64, 0x8000_0000_0000_0001),
        (ScalarType::U64, u64::MAX as u128),
        (ScalarType::I128, 1u128 << 127),
        (ScalarType::U128, u128::MAX),
        (ScalarType::F16, 0x8000),
        (ScalarType::F16, 0x7e35),
        (ScalarType::Bf16, 0x8000),
        (ScalarType::Bf16, 0x7fc5),
        (ScalarType::F32, 0x8000_0000),
        (ScalarType::F32, 0x7fc0_1234),
        (ScalarType::F64, 0x8000_0000_0000_0000),
        (ScalarType::F64, 0x7ff8_1234_5678_9abc),
        (ScalarType::Index, 0xfedc_ba98),
    ];
    let ids: Vec<_> = (0..cases.len())
        .map(|n| u32::try_from(n * 17 + 3).unwrap())
        .collect();
    let module = module(vec![function(
        "bits",
        &ids,
        cases.iter().map(|(ty, _)| Type::Scalar(*ty)).collect(),
    )]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    for (id, (ty, bits)) in ids.iter().zip(cases) {
        let value = RuntimeValue::Scalar(ScalarBitsV1::new(ty, bits, target()).unwrap());
        values.try_insert(ValueId(*id), value.clone()).unwrap();
        assert_eq!(values.get(&ValueId(*id)), Some(value));
    }
    assert_eq!(values.len(), cases.len());
    assert_eq!(
        values.ids().collect::<Vec<_>>(),
        ids.into_iter().map(ValueId).collect::<Vec<_>>()
    );
    assert_eq!(payload_bytes(&values), plan.frame_bytes().unwrap());
}

#[test]
fn scalar_type_or_index_width_substitution_is_refused_without_mutation() {
    let module = module(vec![function(
        "types",
        &[7, 9000],
        vec![Type::Scalar(ScalarType::U32), Type::INDEX],
    )]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    values
        .try_insert(ValueId(7), RuntimeValue::Scalar(ScalarBitsV1::u32(41)))
        .unwrap();
    let before = values.clone();
    assert!(
        values
            .try_insert(
                ValueId(7),
                RuntimeValue::Scalar(ScalarBitsV1::boolean(true))
            )
            .is_err()
    );
    assert!(
        values
            .try_insert(ValueId(8), RuntimeValue::Scalar(ScalarBitsV1::u32(1)))
            .is_err()
    );
    assert_eq!(values, before);
    assert_eq!(values.get(&ValueId(9000)), None);
    let narrow = SimulationTargetV1::little_endian(IndexWidthV1::Bits32);
    let narrow_value = ScalarBitsV1::new(ScalarType::Index, 17, narrow).unwrap();
    assert!(
        values
            .try_insert(ValueId(9000), RuntimeValue::Scalar(narrow_value))
            .is_err()
    );
    let mut narrow_values = columns(&layouts[0], plan, narrow);
    narrow_values
        .try_insert(ValueId(9000), RuntimeValue::Scalar(narrow_value))
        .unwrap();
    assert_eq!(
        narrow_values.get(&ValueId(9000)),
        Some(RuntimeValue::Scalar(narrow_value))
    );
    assert_eq!(values, before);
}

#[test]
fn overwrite_remove_and_reinsert_preserve_exact_initialized_census() {
    let module = module(vec![function(
        "reuse",
        &[1, 100_000],
        vec![Type::F32, Type::F32],
    )]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    for bits in [0x8000_0000, 0x7fc0_0021, 0x3f80_0000] {
        let value =
            RuntimeValue::Scalar(ScalarBitsV1::new(ScalarType::F32, bits, target()).unwrap());
        values.try_insert(ValueId(100_000), value.clone()).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values.remove(&ValueId(100_000)), Some(value));
        assert_eq!(values.remove(&ValueId(100_000)), None);
        assert_eq!(values.len(), 0);
        assert!(!values.contains_key(&ValueId(100_000)));
    }
    assert_eq!(values.ids().count(), 0);
    assert_eq!(values.capacity(), 2);
    assert_eq!(payload_bytes(&values), plan.frame_bytes().unwrap());
}

#[test]
fn reset_to_a_sibling_with_reused_ids_cannot_resurrect_old_values_or_types() {
    let module = module(vec![
        function(
            "first",
            &[5, 91],
            vec![Type::Scalar(ScalarType::U128), Type::F32],
        ),
        function("second", &[5], vec![Type::Scalar(ScalarType::U8)]),
    ]);
    let plan = frame_value_plan(&module, &[0, 1], false).unwrap();
    let layouts = build_layouts(&module, &[0, 1], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    values
        .try_insert(
            ValueId(5),
            RuntimeValue::Scalar(ScalarBitsV1::new(ScalarType::U128, u128::MAX, target()).unwrap()),
        )
        .unwrap();
    values
        .try_insert(
            ValueId(91),
            RuntimeValue::Scalar(
                ScalarBitsV1::new(ScalarType::F32, 0x7fc0_0001, target()).unwrap(),
            ),
        )
        .unwrap();
    let bytes = payload_bytes(&values);
    for _ in 0..64 {
        values.reset(Some(&layouts[1]), 0).unwrap();
        assert_eq!(values.get(&ValueId(5)), None);
        assert_eq!(values.get(&ValueId(91)), None);
        values
            .try_insert(
                ValueId(5),
                RuntimeValue::Scalar(ScalarBitsV1::new(ScalarType::U8, 255, target()).unwrap()),
            )
            .unwrap();
        values.reset(Some(&layouts[0]), 0).unwrap();
        assert_eq!(values.len(), 0);
        assert_eq!(values.get(&ValueId(5)), None);
        assert_eq!(payload_bytes(&values), bytes);
    }
}

#[test]
fn nested_and_recursive_frames_share_only_layout_not_runtime_cells() {
    let module = module(vec![function(
        "recursive",
        &[11],
        vec![Type::Scalar(ScalarType::U64)],
    )]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut frames: Vec<_> = (0..7)
        .map(|_| columns(&layouts[0], plan, target()))
        .collect();
    for (index, frame) in frames.iter_mut().enumerate() {
        frame
            .try_insert(
                ValueId(11),
                RuntimeValue::Scalar(
                    ScalarBitsV1::new(ScalarType::U64, index as u128, target()).unwrap(),
                ),
            )
            .unwrap();
    }
    frames[3].reset(Some(&layouts[0]), 0).unwrap();
    for (index, frame) in frames.iter().enumerate() {
        if index == 3 {
            assert_eq!(frame.get(&ValueId(11)), None);
        } else {
            assert_eq!(
                frame.get(&ValueId(11)),
                Some(RuntimeValue::Scalar(
                    ScalarBitsV1::new(ScalarType::U64, index as u128, target()).unwrap()
                ))
            );
        }
    }
}

fn pointer() -> PointerValue {
    PointerValue {
        exposed_generic: false,
        allocation: 987,
        byte_offset: 28,
        element: ScalarType::U32,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
        lower_bound: 12,
        upper_bound: 60,
        abi_argument_ordinal: 3,
    }
}

#[test]
fn non_scalar_column_retains_full_pointer_provenance_and_rejects_type_substitution() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let module = module(vec![function("pointer", &[17], vec![ty])]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    let value = RuntimeValue::Pointer(pointer());
    values.try_insert(ValueId(17), value.clone()).unwrap();
    assert_eq!(values.get(&ValueId(17)), Some(value.clone()));
    assert_eq!(values.get_ref(&ValueId(17)), Some(&value));
    let before = values.clone();
    let mut wrong = pointer();
    wrong.address_space = AddressSpace::Private;
    assert!(
        values
            .try_insert(ValueId(17), RuntimeValue::Pointer(wrong))
            .is_err()
    );
    assert!(
        values
            .try_insert(ValueId(17), RuntimeValue::Scalar(ScalarBitsV1::u32(7)))
            .is_err()
    );
    assert_eq!(values, before);
    values.reset(Some(&layouts[0]), 0).unwrap();
    assert_eq!(values.get_ref(&ValueId(17)), None);
}

#[test]
fn uniform_columns_account_independent_scalar_and_non_scalar_high_water() {
    let pointers = vec![
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite
        );
        5
    ];
    let module = module(vec![
        function(
            "scalar",
            &[0, 1, 2, 3, 4, 5, 6, 7, 8],
            vec![Type::Scalar(ScalarType::U64); 9],
        ),
        function("pointer", &[0, 1, 2, 3, 4], pointers),
        function(
            "unreachable",
            &(0..100).collect::<Vec<_>>(),
            vec![Type::F32; 100],
        ),
    ]);
    let plan = frame_value_plan(&module, &[0, 1], false).unwrap();
    assert_eq!((plan.scalars, plan.others), (9, 5));
    let layouts = build_layouts(&module, &[0, 1], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    let before = payload_bytes(&values);
    values.reset(Some(&layouts[1]), 0).unwrap();
    assert_eq!(before, payload_bytes(&values));
    assert_eq!(before, plan.frame_bytes().unwrap());
    let shared = size_of::<Vec<ValueLayout<'_>>>()
        + layouts.capacity() * size_of::<ValueLayout<'_>>()
        + layouts
            .iter()
            .map(|layout| layout.slots.capacity() * size_of::<Slot<'_>>())
            .sum::<usize>();
    assert_eq!(shared, plan.shared_bytes);
}

#[test]
fn layout_rejects_duplicate_ids_missing_function_and_inadequate_prepaid_capacity() {
    let duplicate = module(vec![function("duplicate", &[3, 3], vec![Type::F32; 2])]);
    let plan = frame_value_plan(&duplicate, &[0], false).unwrap();
    assert!(build_layouts(&duplicate, &[0], plan).is_err());
    assert!(frame_value_plan(&duplicate, &[1], false).is_none());
    let module = module(vec![function("one", &[3], vec![Type::F32])]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut short = plan;
    short.scalars = 0;
    assert!(RuntimeValues::prepared(Some(&layouts[0]), short, target(), 0).is_err());
    assert!(RuntimeValues::prepared(None, plan, target(), 0).is_err());
}

#[test]
fn physical_fallback_keeps_wide_scalar_typed_symbolic_cells_and_original_capacity() {
    let module = module(vec![function(
        "physical",
        &[0],
        vec![Type::Scalar(ScalarType::U32)],
    )]);
    let plan = frame_value_plan(&module, &[0], true).unwrap();
    assert!(build_layouts(&module, &[0], plan).unwrap().is_empty());
    assert_eq!(
        plan.frame_bytes(),
        reserved_hash_map_bytes::<ValueId, RuntimeValue>(1)
    );
    let mut values = RuntimeValues::prepared(None, plan, target(), 1).unwrap();
    let value = RuntimeValue::PhysicalEntry(physical_entry_state_v20::Value::ScaledOffset {
        half: physical_entry_state_v20::Half::High,
        generation: CompactSite {
            function: 0,
            block: BlockId(0),
            operation: Some(0),
        },
        displacement: 0x1234_5678_abcd_ef01,
    });
    values.try_insert(ValueId(0), value.clone()).unwrap();
    assert_eq!(values.get_ref(&ValueId(0)), Some(&value));
    assert!(values.get_mut(&ValueId(99)).is_none());
    let ready = RuntimeValue::Scalar(ScalarBitsV1::u32(17));
    *values.get_mut(&ValueId(0)).unwrap() = ready.clone();
    assert_eq!(values.get_ref(&ValueId(0)), Some(&ready));
    assert_eq!(values.len(), 1);
    values.reset(None, 1).unwrap();
    assert_eq!(values.get(&ValueId(0)), None);
}

#[test]
fn sparse_initialized_slots_keep_full_scan_charge_and_deterministic_value_order() {
    let module = module(vec![function(
        "sparse",
        &[100, 5, 1000, 1],
        vec![Type::F32; 4],
    )]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    for id in [1000, 1] {
        values
            .try_insert(
                ValueId(id),
                RuntimeValue::Scalar(ScalarBitsV1::new(ScalarType::F32, 0, target()).unwrap()),
            )
            .unwrap();
    }
    assert_eq!(
        values.ids().collect::<Vec<_>>(),
        vec![ValueId(1), ValueId(1000)]
    );
    assert_eq!(values.len(), 2);
    assert_eq!(values.capacity(), 4);
    assert!(values.lookup_work().unwrap() >= 2 * 3);
}

#[test]
fn measured_fp8_typed_census_is_below_the_unchanged_resident_limit_without_discounting_other_terms()
{
    // Original exported bundle census: 2,148 scalar +78 other definitions,
    // two retained frame depths and256 participants. No workload/cap rewrite.
    let plan = FrameValuePlan {
        scalars: 2148,
        others: 78,
        shared_bytes: 0,
        legacy: None,
    };
    let bytes = plan.frame_bytes().unwrap();
    assert!(bytes < reserved_hash_map_bytes::<ValueId, RuntimeValue>(2226).unwrap());
    let original_without_ssa = 605_233_171usize - 371_212_288;
    let columns = bytes.checked_mul(2 * 256).unwrap();
    assert!(original_without_ssa + columns < 256 * 1024 * 1024);
    // This arithmetic is not complete execution qualification: new headers and
    // shared layout capacities are independently included by the real calculator.
}

#[test]
fn legacy_storage_selector_is_identical_for_empty_and_physical_v20_modules() {
    let module = module(vec![function(
        "no-physical-operations",
        &[1],
        vec![Type::F32],
    )]);
    for version in 1..=25 {
        let legacy = uses_legacy_frame_values(version);
        assert_eq!(legacy, matches!(version, 20 | 21 | 22));
        let plan = frame_value_plan(&module, &[0], legacy).unwrap();
        let layouts = build_layouts(&module, &[0], plan).unwrap();
        assert_eq!(layouts.is_empty(), legacy);
        assert_eq!(plan.legacy.is_some(), legacy);
    }
}

#[test]
fn scalar_and_pointer_sibling_slots_clear_all_retained_columns_before_reuse() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let module = module(vec![
        function("pointer", &[4, 9], vec![ty.clone(), ty]),
        function("scalar", &[4], vec![Type::Scalar(ScalarType::U128)]),
    ]);
    let plan = frame_value_plan(&module, &[0, 1], false).unwrap();
    let layouts = build_layouts(&module, &[0, 1], plan).unwrap();
    let mut values = columns(&layouts[0], plan, target());
    for id in [4, 9] {
        values
            .try_insert(ValueId(id), RuntimeValue::Pointer(pointer()))
            .unwrap();
    }
    let capacity = payload_bytes(&values);
    let before = values.clone();
    assert!(values.get_mut(&ValueId(4)).is_none());
    assert!(values.get_mut(&ValueId(9)).is_none());
    assert!(values.get_mut(&ValueId(99)).is_none());
    assert_eq!(values, before);
    values.reset(Some(&layouts[1]), 0).unwrap();
    assert_eq!(values.get_ref(&ValueId(4)), None);
    values
        .try_insert(
            ValueId(4),
            RuntimeValue::Scalar(ScalarBitsV1::new(ScalarType::U128, u128::MAX, target()).unwrap()),
        )
        .unwrap();
    let before = values.clone();
    assert!(values.get_mut(&ValueId(4)).is_none());
    assert_eq!(values, before);
    values.reset(Some(&layouts[0]), 0).unwrap();
    assert_eq!(values.get(&ValueId(4)), None);
    assert_eq!(values.get(&ValueId(9)), None);
    assert_eq!(values.len(), 0);
    assert_eq!(payload_bytes(&values), capacity);
}

#[test]
fn column_accounting_is_checked_and_capacity_shortfall_is_not_a_larger_fallback() {
    let overflow = FrameValuePlan {
        scalars: usize::MAX,
        others: 0,
        shared_bytes: 0,
        legacy: None,
    };
    assert_eq!(overflow.frame_bytes(), None);
    let module = module(vec![function("scalar", &[4], vec![Type::F32])]);
    let plan = frame_value_plan(&module, &[0], false).unwrap();
    let layouts = build_layouts(&module, &[0], plan).unwrap();
    assert!(
        RuntimeValues::prepared(Some(&layouts[0]), FrameValuePlan::legacy(1), target(), 1).is_err()
    );
    assert!(RuntimeValues::prepared(None, plan, target(), 1).is_err());
    let mut value = columns(&layouts[0], plan, target());
    let before = value.clone();
    assert!(value.reset(None, 999).is_err());
    assert_eq!(value, before);
}
