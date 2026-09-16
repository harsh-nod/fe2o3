// Isolated capability-census tests. These do not grant a semantic module,
// pipeline lifetime, indexed effect, or executable artifact any authority.
fn scalar_pipeline_payload_census_v1(
    writes: &[(SemanticTypeIdV1, SemanticOperandV1)],
    has_owner: bool,
) -> Result<HashMap<usize, ProjectedMfmaOperandV1>, ProductionRankedProjectionErrorV1> {
    let types = optional_selector_types();
    let mut callables = Vec::new();
    let mut blocks = Vec::new();
    for (index, (element, value)) in writes.iter().enumerate() {
        callables.push(compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                pipeline: POINTER_TYPE,
                element: *element,
            },
        ));
        let receiver = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], POINTER_TYPE).unwrap(),
        );
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(u32::try_from(index).unwrap()),
            vec![
                receiver,
                typed_constant(U64_TYPE, 0, 8),
                typed_constant(U64_TYPE, 0, 8),
                value.clone(),
            ],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        blocks.push(block(
            u8::try_from(220 + index).unwrap(),
            vec![],
            SemanticTerminatorKindV1::Call(call),
        ));
    }
    let function = projection_function_with_locals(
        blocks,
        vec![
            local(0, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(1, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(2, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
        ],
    );
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let owners = vec![None, has_owner.then_some(0), None];
    let entries = vec![Some(HashMap::new()); writes.len()];
    collect_workgroup_pipeline_payloads_v1(
        &types, &callables, &function, &dominance, &owners, &entries, &mut 0,
    )
}

#[test]
fn scalar_pipeline_payload_does_not_require_or_fabricate_fragment_authority() {
    for (ty, bytes) in [
        (SCALAR_TYPE, 4),
        (U8_TYPE, 1),
        (U16_TYPE, 2),
        (I32_TYPE, 4),
        (U64_TYPE, 8),
        (F32_TYPE, 4),
        (F64_TYPE, 8),
    ] {
        let payloads =
            scalar_pipeline_payload_census_v1(&[(ty, typed_constant(ty, 0, bytes))], true).unwrap();
        assert!(payloads.is_empty());
    }
    let local_value = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE).unwrap(),
    );
    assert!(
        scalar_pipeline_payload_census_v1(&[(SCALAR_TYPE, local_value)], true)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn scalar_pipeline_payload_still_requires_exact_element_type_and_owner() {
    assert!(matches!(
        scalar_pipeline_payload_census_v1(&[(SCALAR_TYPE, typed_constant(U64_TYPE, 0, 8))], true)
            .unwrap_err(),
        ProductionRankedProjectionErrorV1::Incomplete(
            "a workgroup pipeline write payload changed semantic type"
        )
    ));
    assert!(matches!(
        scalar_pipeline_payload_census_v1(
            &[(SCALAR_TYPE, typed_constant(SCALAR_TYPE, 0, 4))],
            false
        )
        .unwrap_err(),
        ProductionRankedProjectionErrorV1::Incomplete(
            "a workgroup pipeline write lacks one compiler-owned origin"
        )
    ));
}

#[test]
fn scalar_pipeline_payload_does_not_admit_pointers_aggregates_or_unreviewed_scalars() {
    for ty in [
        POINTER_TYPE,
        ARRAY_TYPE,
        ENUM_TYPE,
        BOOL_TYPE,
        CHAR_TYPE,
        U128_TYPE,
    ] {
        let value = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], ty).unwrap(),
        );
        assert!(matches!(
            scalar_pipeline_payload_census_v1(&[(ty, value)], true).unwrap_err(),
            ProductionRankedProjectionErrorV1::Incomplete(
                "a workgroup pipeline write lacks one authenticated typed payload"
            )
        ));
    }
}

#[test]
fn scalar_pipeline_payload_rejects_equal_width_element_reinterpretation() {
    assert!(matches!(
        scalar_pipeline_payload_census_v1(
            &[
                (SCALAR_TYPE, typed_constant(SCALAR_TYPE, 0, 4)),
                (F32_TYPE, typed_constant(F32_TYPE, 0, 4)),
            ],
            true
        )
        .unwrap_err(),
        ProductionRankedProjectionErrorV1::Incomplete(
            "one workgroup pipeline receives inconsistent semantic payload types"
        )
    ));
}
