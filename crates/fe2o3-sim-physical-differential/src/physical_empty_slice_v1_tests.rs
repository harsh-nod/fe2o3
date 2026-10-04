#[test]
fn generated_empty_pointers_follow_element_types_not_buffer_overalignment() {
    use fe2o3_kir_sim::BufferArgumentV1;
    for (element, expected) in [
        (ScalarType::I8, 1),
        (ScalarType::U8, 1),
        (ScalarType::I16, 2),
        (ScalarType::U16, 2),
        (ScalarType::I32, 4),
        (ScalarType::U32, 4),
        (ScalarType::F32, 4),
        (ScalarType::I64, 8),
        (ScalarType::U64, 8),
        (ScalarType::F64, 8),
    ] {
        for alignment in [expected as u32, 64] {
            let buffer = BufferArgumentV1::new(
                element,
                AccessMode::ReadOnly,
                alignment,
                Vec::new(),
                Vec::new(),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap();
            assert_eq!(
                buffer
                    .element_count(SimulationTargetV1::amdgpu_64())
                    .unwrap(),
                0
            );
            assert_eq!(
                generated_empty_slice_pointer(buffer.element()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn unsupported_generated_empty_element_types_are_not_reinterpreted() {
    for element in [
        ScalarType::Bool,
        ScalarType::Index,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::I128,
        ScalarType::U128,
    ] {
        assert!(matches!(
            generated_empty_slice_pointer(element),
            Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)
        ));
    }
}
