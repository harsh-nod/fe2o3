use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LANE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn reference_types(mutability: SemanticMutabilityV1) -> Vec<SemanticTypeDeclV1> {
    let old = neutral_semantic_types_v1();
    let lane = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(224)),
        SemanticLayoutIdentityV1::from_sha256(bytes(224)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    );
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(225)),
        SemanticLayoutIdentityV1::from_sha256(bytes(225)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                LANE,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    match mutability {
                        SemanticMutabilityV1::Immutable => {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        }
                        SemanticMutabilityV1::Mutable => {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        }
                    },
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    vec![old[0].clone(), old[3].clone(), lane, reference]
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn dereference(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, LANE).unwrap()],
        LANE,
    )
    .unwrap()
}

fn borrow_kind(mutability: SemanticMutabilityV1) -> SemanticBorrowKindV1 {
    match mutability {
        SemanticMutabilityV1::Immutable => SemanticBorrowKindV1::Shared,
        SemanticMutabilityV1::Mutable => SemanticBorrowKindV1::Mutable,
    }
}

fn borrow(
    destination: u32,
    source: SemanticPlaceV1,
    kind: SemanticBorrowKindV1,
) -> SemanticAssignmentV1 {
    SemanticAssignmentV1::new(
        place(destination, REFERENCE),
        SemanticRvalueV1::new(
            REFERENCE,
            SemanticRvalueKindV1::Borrow {
                kind,
                place: source,
            },
        ),
    )
}

fn admitted_source(mutability: SemanticMutabilityV1) -> ProductionSemanticSsaOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let layout = SemanticLayoutIdentityV1::from_sha256(bytes(250));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(203)),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let issue = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![],
        Some(SemanticCallDestinationV1::new(
            place(1, LANE),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let locals = [UNIT, LANE, REFERENCE, REFERENCE]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            local(
                210 + index as u8,
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
            )
        })
        .collect();
    let kind = borrow_kind(mutability);
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(203)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(203)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(203)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(203)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(203)),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            block(204, vec![], SemanticTerminatorKindV1::Call(issue)),
            block(
                205,
                vec![
                    statement(SemanticStatementKindV1::Assign(borrow(
                        2,
                        place(1, LANE),
                        kind,
                    ))),
                    statement(SemanticStatementKindV1::Assign(borrow(
                        3,
                        dereference(2),
                        kind,
                    ))),
                ],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(2),
                )),
            ),
            block(206, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"capability_reference_reborrow".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(207)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let lane_result = SemanticAbiValueV1::new(
        LANE,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    let intrinsic_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(208)),
        layout,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        lane_result,
    )
    .unwrap();
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(208)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(208)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(208)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(208)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(208)),
            source,
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: LANE,
            wave_width: 64,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(209)),
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout),
        reference_types(mutability),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let mir = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        mir,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn admitted_lane_reborrow_survives_first_propagation_and_replay() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        let owner = admitted_source(mutability);
        owner.verify_replay().unwrap();
        let semantic = owner.source_semantic();
        let function = &semantic.functions()[0];
        let dominance =
            SemanticEnumPayloadDominanceV1::analyze(function, semantic.types()).unwrap();
        let locals = vec![None; function.locals().len()];
        let mut work = 0;
        let entries = propagate_capability_dataflow_v1(
            semantic.types(),
            semantic.callables(),
            function,
            &dominance,
            &locals,
            &vec![None; function.locals().len()],
            &vec![None; function.locals().len()],
            &HashMap::new(),
            0,
            &mut work,
        )
        .unwrap();
        let expected = ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
            root: 1,
            wave_width: 64,
        });
        let entry = entries[2].as_ref().unwrap();
        assert_eq!(entry.get(&1), Some(&expected));
        assert_eq!(entry.get(&2), Some(&expected));
        assert_eq!(entry.get(&3), Some(&expected));
        let effects = project_authenticated_capabilities_v1(
            semantic.types(),
            semantic.callables(),
            function,
            &dominance,
            &locals,
            &vec![None; function.locals().len()],
        )
        .unwrap();
        assert!(effects.global_reads.iter().all(Option::is_none));
        owner.verify_replay().unwrap();
    }
}

fn component_function(
    source_type: SemanticTypeIdV1,
    destination_type: SemanticTypeIdV1,
) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        vec![block(230, vec![], SemanticTerminatorKindV1::Return)],
        vec![
            local(230, UNIT, SemanticLocalRoleV1::Return),
            local(231, source_type, SemanticLocalRoleV1::Temporary),
            local(232, destination_type, SemanticLocalRoleV1::Temporary),
        ],
    )
}

fn component_origin() -> ProjectedCapabilityValueV1 {
    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
        root: 0x200000007,
        wave_width: 64,
    })
}

fn component_call(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    assignment: &SemanticAssignmentV1,
    value: ProjectedCapabilityValueV1,
) -> Option<ProjectedCapabilityValueV1> {
    capability_reference_reborrow_origin_v1(
        types,
        function,
        assignment,
        &HashMap::from([(1, value)]),
        &mut 0,
    )
    .unwrap()
}

#[test]
fn exact_reference_components_preserve_only_existing_borrowable_payloads() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        let types = reference_types(mutability);
        let function = component_function(REFERENCE, REFERENCE);
        let assignment = borrow(2, dereference(1), borrow_kind(mutability));
        let view = *authenticated_tensor_load_state().get(&0).unwrap();
        let read = ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::ReadView(
            strided_read_view(),
        ));
        for value in [component_origin(), view, read] {
            assert_eq!(
                component_call(&types, &function, &assignment, value),
                Some(value)
            );
        }
        for value in [
            ProjectedCapabilityValueV1::Invalid,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::MatrixContext {
                root: 7,
            }),
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::ReadViewResult(
                strided_read_view(),
            )),
            wrap_capability_enum_value_v1(component_origin(), 1).unwrap(),
        ] {
            assert_eq!(component_call(&types, &function, &assignment, value), None);
        }
        assert_eq!(
            capability_reference_reborrow_origin_v1(
                &types,
                &function,
                &assignment,
                &HashMap::new(),
                &mut 0
            )
            .unwrap(),
            None
        );
    }
}

#[test]
fn reference_components_reject_foreign_types_access_metadata_and_projection_shapes() {
    let types = reference_types(SemanticMutabilityV1::Immutable);
    let function = component_function(REFERENCE, REFERENCE);
    let assignment = borrow(2, dereference(1), SemanticBorrowKindV1::Shared);
    for (kind, mutability, address_space, width, metadata, pointee) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            LANE,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            LANE,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            1,
            64,
            SemanticPointerMetadataV1::None,
            LANE,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            32,
            SemanticPointerMetadataV1::None,
            LANE,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
            LANE,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            U32,
        ),
    ] {
        // Descriptor-only negatives, not admitted owners or fabricated issuance.
        let mut changed = types.clone();
        changed[3] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(240)),
            SemanticLayoutIdentityV1::from_sha256(bytes(240)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    kind,
                    mutability,
                    address_space,
                    width,
                    metadata,
                )
                .unwrap(),
            ),
        );
        assert_eq!(
            component_call(&changed, &function, &assignment, component_origin()),
            None
        );
    }
    for (source, destination) in [
        (LANE, REFERENCE),
        (REFERENCE, LANE),
        (SemanticTypeIdV1::from_index(99), REFERENCE),
    ] {
        assert_eq!(
            component_call(
                &types,
                &component_function(source, destination),
                &assignment,
                component_origin()
            ),
            None
        );
    }
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Fake] {
        assert_eq!(
            component_call(
                &types,
                &function,
                &borrow(2, dereference(1), kind),
                component_origin()
            ),
            None
        );
    }
    for source in [
        place(1, LANE),
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), LANE).unwrap()],
            LANE,
        )
        .unwrap(),
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, LANE).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap(),
            ],
            U32,
        )
        .unwrap(),
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap(),
        dereference(99),
    ] {
        assert_eq!(
            component_call(
                &types,
                &function,
                &borrow(2, source, SemanticBorrowKindV1::Shared),
                component_origin()
            ),
            None
        );
    }
    let mismatched_result = SemanticAssignmentV1::new(
        place(2, REFERENCE),
        SemanticRvalueV1::new(LANE, assignment.value().kind().clone()),
    );
    let projected_destination =
        SemanticAssignmentV1::new(dereference(2), assignment.value().clone());
    let raw_result = SemanticAssignmentV1::new(
        place(2, REFERENCE),
        SemanticRvalueV1::new(
            REFERENCE,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: dereference(1),
            },
        ),
    );
    for rejected in [
        mismatched_result,
        projected_destination,
        raw_result,
        borrow(99, dereference(1), SemanticBorrowKindV1::Shared),
    ] {
        assert_eq!(
            component_call(&types, &function, &rejected, component_origin()),
            None
        );
    }
    assert_eq!(
        component_call(&types[..3], &function, &assignment, component_origin()),
        None
    );
    let mut non_pointer = types.clone();
    non_pointer[3] = types[2].clone();
    assert_eq!(
        component_call(&non_pointer, &function, &assignment, component_origin()),
        None
    );
    let wrong_destination = SemanticAssignmentV1::new(place(2, LANE), assignment.value().clone());
    assert_eq!(
        component_call(&types, &function, &wrong_destination, component_origin()),
        None
    );
    let mutable_types = reference_types(SemanticMutabilityV1::Mutable);
    assert_eq!(
        component_call(&mutable_types, &function, &assignment, component_origin()),
        None
    );
}

#[test]
fn reborrow_preserves_existing_move_invalidation_and_exact_join_rules() {
    let types = reference_types(SemanticMutabilityV1::Immutable);
    let function = component_function(REFERENCE, REFERENCE);
    let assignment = borrow(2, dereference(1), SemanticBorrowKindV1::Shared);
    for operand in [
        SemanticOperandV1::Copy(place(1, REFERENCE)),
        SemanticOperandV1::Move(place(1, REFERENCE)),
    ] {
        let mut state = HashMap::from([(1, component_origin())]);
        consume_capability_operand_v1(&mut state, &operand);
        assert_eq!(state[&1], ProjectedCapabilityValueV1::Invalid);
        assert_eq!(
            capability_reference_reborrow_origin_v1(&types, &function, &assignment, &state, &mut 0)
                .unwrap(),
            None
        );
    }
    for incoming in [
        HashMap::new(),
        HashMap::from([(
            1,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
                root: 9,
                wave_width: 64,
            }),
        )]),
    ] {
        let mut state = HashMap::from([(1, component_origin())]);
        merge_capability_states_v1(&mut state, &incoming).unwrap();
        assert_eq!(
            capability_reference_reborrow_origin_v1(&types, &function, &assignment, &state, &mut 0)
                .unwrap(),
            None
        );
    }
    let mut state = HashMap::from([(1, component_origin())]);
    let same = state.clone();
    assert!(!merge_capability_states_v1(&mut state, &same).unwrap());
    assert_eq!(
        capability_reference_reborrow_origin_v1(&types, &function, &assignment, &state, &mut 0)
            .unwrap(),
        Some(component_origin())
    );
}

#[test]
fn reborrow_fixed_work_is_prepaid_without_state_publication() {
    let types = reference_types(SemanticMutabilityV1::Immutable);
    let function = component_function(REFERENCE, REFERENCE);
    let assignment = borrow(2, dereference(1), SemanticBorrowKindV1::Shared);
    let state = HashMap::from([(1, component_origin())]);
    let mut work = 0;
    assert_eq!(
        capability_reference_reborrow_origin_v1(&types, &function, &assignment, &state, &mut work)
            .unwrap(),
        Some(component_origin())
    );
    assert_eq!(work, 4 + 5 + 10 + 2);
    let mut exact = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - 21;
    assert!(
        capability_reference_reborrow_origin_v1(&types, &function, &assignment, &state, &mut exact)
            .unwrap()
            .is_some()
    );
    assert_eq!(exact, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    for (allowance, attempted) in [(3, 4), (8, 9), (18, 19), (20, 21)] {
        let mut limited = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - allowance;
        assert!(matches!(
            capability_reference_reborrow_origin_v1(
                &types,
                &function,
                &assignment,
                &state,
                &mut limited
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability dataflow exceeds the charged projection limit"
            ))
        ));
        assert_eq!(
            limited,
            MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - allowance + attempted
        );
        assert_eq!(state, HashMap::from([(1, component_origin())]));
    }
    let mut overflow = usize::MAX;
    assert!(matches!(
        capability_reference_reborrow_origin_v1(
            &types,
            &function,
            &assignment,
            &state,
            &mut overflow
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow work overflow"
        ))
    ));
    assert_eq!(overflow, usize::MAX);
    let active = projection_function_with_locals(
        vec![block(
            234,
            vec![statement(SemanticStatementKindV1::Assign(assignment))],
            SemanticTerminatorKindV1::Return,
        )],
        function.locals().to_vec(),
    );
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&active, &types).unwrap();
    let mut state = HashMap::from([
        (1, component_origin()),
        (2, ProjectedCapabilityValueV1::Invalid),
    ]);
    let before = state.clone();
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - 20;
    assert!(
        transfer_capability_statements_v1(&types, &active, 0, &mut state, &dominance, &mut work)
            .is_err()
    );
    assert_eq!(
        state, before,
        "the actual statement driver must not publish a denied reborrow"
    );
}

#[test]
fn reborrowed_view_and_lane_still_require_exact_typed_load_provenance() {
    // This is the existing private load component, not an admitted BF16 owner.
    // The ordinary Rust loaded-source test supplies the end-to-end gate.
    let types = reference_types(SemanticMutabilityV1::Immutable);
    let function = component_function(REFERENCE, REFERENCE);
    let assignment = borrow(2, dereference(1), SemanticBorrowKindV1::Shared);
    let original = authenticated_tensor_load_state();
    let mut state = HashMap::new();
    for key in [0, 1] {
        let value = original[&key];
        state.insert(
            key,
            component_call(&types, &function, &assignment, value).unwrap(),
        );
    }
    assert_eq!(state, original);
    let load = tensor_load_function(Some(place(4, SCALAR_TYPE)));
    let SemanticTerminatorKindV1::Call(call) = load.blocks()[0].terminator().kind() else {
        panic!("load fixture")
    };
    let contract = mfma_operand_contract(SemanticMfmaOperandRoleV1::A);
    let expected = project_tensor_load_origin_v1(
        call,
        &original,
        SCALAR_TYPE,
        SCALAR_TYPE,
        contract,
        SemanticMfmaStorageLayoutV1::RowMajor,
    );
    assert!(matches!(
        expected,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(_))
    ));
    assert_eq!(
        project_tensor_load_origin_v1(
            call,
            &state,
            SCALAR_TYPE,
            SCALAR_TYPE,
            contract,
            SemanticMfmaStorageLayoutV1::RowMajor
        ),
        expected
    );
    for missing in [0, 1] {
        let mut changed = state.clone();
        changed.remove(&missing);
        assert_eq!(
            project_tensor_load_origin_v1(
                call,
                &changed,
                SCALAR_TYPE,
                SCALAR_TYPE,
                contract,
                SemanticMfmaStorageLayoutV1::RowMajor
            ),
            ProjectedCapabilityValueV1::Invalid
        );
    }
    for (result, contract, layout) in [
        (ARRAY_TYPE, contract, SemanticMfmaStorageLayoutV1::RowMajor),
        (
            SCALAR_TYPE,
            mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
            SemanticMfmaStorageLayoutV1::RowMajor,
        ),
        (SCALAR_TYPE, contract, SemanticMfmaStorageLayoutV1::LdsXor4),
        (
            SCALAR_TYPE,
            SemanticMfmaOperandContractV1 {
                wave_width: 32,
                ..contract
            },
            SemanticMfmaStorageLayoutV1::RowMajor,
        ),
    ] {
        assert_eq!(
            project_tensor_load_origin_v1(call, &state, result, SCALAR_TYPE, contract, layout),
            ProjectedCapabilityValueV1::Invalid
        );
    }
}
