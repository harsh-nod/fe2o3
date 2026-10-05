use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1, SemanticDirectEnumEncodingV1,
    SemanticEnumLayoutV1, SemanticEnumVariantLayoutV1,
};

include!("production_execution_cfg_carrier_v29_tests.rs");
include!("production_invocation_representation_v29_tests.rs");

fn visitor_scalar(value: u32) -> SemanticValueBindingV1 {
    SemanticValueBindingV1::Value {
        id: ValueId(value),
        ty: Type::Scalar(ScalarType::U32),
    }
}

fn check_transport_visitors(
    transport: SemanticPromotedTransportV1,
    binding: &SemanticValueBindingV1,
    expected: &[Type],
) {
    let legacy = transport.transport_values(binding, expected);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget.reserve_storage(73).unwrap();
    let mut collector = InvocationTransportCollectorV1 {
        values: vec![],
        nodes: 0,
        budget: &mut budget,
    };
    let bounded = transport.visit_transport_values(binding, expected, &mut collector);
    match (legacy, bounded) {
        (Ok(values), Ok(())) => assert_eq!(
            collector.values,
            values
                .into_iter()
                .map(|(value, ty)| ValueDef::new(value, ty))
                .collect::<Vec<_>>()
        ),
        (Err(expected), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })) => {
            assert_eq!(detail, expected)
        }
        (legacy, bounded) => panic!("collector disagreement: {legacy:?} / {bounded:?}"),
    }
    drop(collector);
    let retained = budget.storage() - 73;
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 73);
}

fn check_paid_compiler_carrier_construction(
    descriptor: SemanticPromotedBindingV1,
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    values: &[ValueDef],
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(37).unwrap();
    let mut allocation = CompilerCarrierAllocationV29::paid(&mut budget).unwrap();
    let paid_types = descriptor
        .transport_types_with_allocation_v29(types, ty, &mut allocation)
        .unwrap();
    assert_eq!(
        paid_types,
        values
            .iter()
            .map(|value| value.ty.clone())
            .collect::<Vec<_>>()
    );
    let paid = descriptor
        .binding_from_transport_with_allocation_v29(types, ty, values, &mut allocation)
        .unwrap();
    let expected = descriptor
        .binding_from_transport(types, ty, values)
        .unwrap();
    assert_eq!(format!("{paid:?}"), format!("{expected:?}"));
    assert_eq!(
        descriptor.transport_values(&paid).unwrap(),
        values
            .iter()
            .map(|value| (value.id, value.ty.clone()))
            .collect::<Vec<_>>()
    );
    drop((paid, expected, paid_types, allocation));
    let storage = budget.storage() - 37;
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 37);

    // This is descriptor-equation coverage, not source-plan admission. The
    // source table has separate owner, instance and Plain-node checks.
    let carrier = ExecutionCfgCarrierV29 {
        source_type: ty,
        transport_type: ty,
        binding: descriptor,
        kernel_types: values.iter().map(|value| value.ty.clone()).collect(),
    };
    let original = descriptor
        .binding_from_transport(types, ty, values)
        .unwrap();
    with_execution_cfg_carrier_values_v29(&carrier, &original, &mut budget, |actual, budget| {
        assert_eq!(actual, values);
        let rebuilt = carrier.rebuild(types, actual, budget)?;
        merge_execution_cfg_carrier_v29(&carrier, &original, &rebuilt, budget)?;
        assert_eq!(format!("{original:?}"), format!("{rebuilt:?}"));
        Ok(())
    })
    .unwrap();
    let retained = budget.storage() - 37;
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn shared_borrowed_transport_preserves_every_compiler_issued_variant() {
    use SemanticPromotedBindingV1 as P;
    let optional = test_option_availability_v1();
    let available = SemanticCapabilityAvailabilityV1::Option(optional);
    let ty = SemanticTypeIdV1::from_index(0);
    let types = [plain_bit_scalar_type(
        10,
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        },
    )];
    let operand = SemanticMfmaOperandContractV1 {
        role: SemanticMfmaOperandRoleV1::A,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    };
    let accumulator = SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    };
    let descriptors = [
        P::Ordinary,
        P::MathContext,
        P::CollectiveContext,
        P::WorkgroupLdsScope,
        P::MatrixContext,
        P::WaveLane { wave_width: 64 },
        P::WorkgroupPipeline {
            pipeline: ty,
            element: ty,
            payload_binding: SemanticPipelinePayloadBindingV1::Ordinary,
            buffers: 2,
            elements: 32,
            prefetch_distance: 1,
            packed_bits: 32,
            alignment: 4,
        },
        P::WorkgroupPipeline {
            pipeline: ty,
            element: ty,
            payload_binding: SemanticPipelinePayloadBindingV1::MatrixFragment {
                contract: operand,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
            buffers: 2,
            elements: 32,
            prefetch_distance: 1,
            packed_bits: 32,
            alignment: 4,
        },
        P::WorkgroupPipeline {
            pipeline: ty,
            element: ty,
            payload_binding: SemanticPipelinePayloadBindingV1::AccumulatorFragment {
                contract: accumulator,
            },
            buffers: 2,
            elements: 32,
            prefetch_distance: 1,
            packed_bits: 32,
            alignment: 4,
        },
        P::DynamicLds {
            dynamic_lds: ty,
            element_storage: ty,
            elements: 32,
            byte_extent: 128,
            alignment: 4,
            producer_function: SemanticFunctionIdV1::from_index(7),
            producer_block: SemanticBlockIdV1::from_index(9),
        },
        P::MatrixFragment {
            contract: operand,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        },
        P::AccumulatorFragment {
            contract: accumulator,
        },
        P::IndexWitness {
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            disjoint: true,
            availability: Some(available),
        },
        P::OptionIndexWitness {
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            disjoint: false,
            availability: optional,
        },
        P::GridLeader {
            availability: available,
        },
        P::OptionGridLeader {
            availability: optional,
        },
        P::ComponentWitness {
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            availability: available,
        },
        P::OptionComponentWitness {
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            availability: optional,
        },
        P::OptionPointer {
            element: ScalarType::U32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
            availability: optional,
        },
        P::Gfx950LdsTransposeTile {
            format: SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
            state: SemanticGfx950LdsTransposeStateV1::Published,
        },
    ];
    for descriptor in descriptors {
        let expected = descriptor.transport_types(&types, ty).unwrap();
        let values: Vec<_> = expected
            .iter()
            .enumerate()
            .map(|(index, ty)| ValueDef::new(ValueId(100 + index as u32), ty.clone()))
            .collect();
        let binding = descriptor
            .binding_from_transport(&types, ty, &values)
            .unwrap();
        check_paid_compiler_carrier_construction(descriptor, &types, ty, &values);
        assert_eq!(
            descriptor.transport_values(&binding).unwrap(),
            values
                .iter()
                .map(|value| (value.id, value.ty.clone()))
                .collect::<Vec<_>>()
        );
        let transport = SemanticPromotedTransportV1::Semantic(descriptor);
        check_transport_visitors(transport, &binding, &expected);
        check_transport_visitors(
            transport,
            &SemanticValueBindingV1::Unmaterialized,
            &expected,
        );
    }
    let aggregate = SemanticValueBindingV1::Aggregate(vec![
        visitor_scalar(7),
        SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit, visitor_scalar(9)]),
    ]);
    for descriptor in [P::Ordinary, P::WorkgroupCollectiveScratch { element: ty }] {
        let transport = SemanticPromotedTransportV1::Semantic(descriptor);
        check_transport_visitors(
            transport,
            &aggregate,
            &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        );
    }
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let direct = SemanticPromotedTransportV1::DirectParameter { parameter_local: 3 };
    check_transport_visitors(
        direct,
        &SemanticValueBindingV1::Value {
            id: ValueId(12),
            ty: slice.clone(),
        },
        std::slice::from_ref(&slice),
    );
    check_transport_visitors(direct, &aggregate, std::slice::from_ref(&slice));
    check_transport_visitors(direct, &visitor_scalar(12), std::slice::from_ref(&slice));
    check_transport_visitors(SemanticPromotedTransportV1::Execution, &aggregate, &[]);

    let transport = SemanticPromotedTransportV1::Semantic(P::Ordinary);
    let enum_binding = |payload| SemanticValueBindingV1::Enum {
        discriminant: ValueId(19),
        discriminant_ty: Type::Scalar(ScalarType::U32),
        semantic_type: ty,
        variant: None,
        payloads: BTreeMap::from([(0, vec![payload])]),
    };
    let plain = enum_binding(aggregate);
    assert_eq!(
        transport.transport_values(&plain, &[]).unwrap(),
        vec![(ValueId(19), Type::Scalar(ScalarType::U32))]
    );
    check_transport_visitors(transport, &plain, &[]);
    for payload in [
        SemanticValueBindingV1::MovedExecution,
        SemanticValueBindingV1::Value {
            id: ValueId(20),
            ty: Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context),
        },
    ] {
        let nested = enum_binding(SemanticValueBindingV1::Aggregate(vec![enum_binding(
            payload,
        )]));
        assert_eq!(
            transport.transport_values(&nested, &[]),
            Err("execution bindings cannot be flattened through enum payloads")
        );
        check_transport_visitors(transport, &nested, &[]);
    }
}

fn independent_collector_header() -> usize {
    use std::mem::{align_of, size_of};
    let fields = 3 * size_of::<usize>() + size_of::<usize>() + 2 * size_of::<usize>();
    let align = align_of::<usize>();
    fields.div_ceil(align) * align
}

fn independent_value_definition_bytes() -> usize {
    use std::mem::{align_of, size_of};
    let fields = size_of::<ValueId>() + size_of::<Type>();
    let align = align_of::<ValueId>().max(align_of::<Type>());
    fields.div_ceil(align) * align
}

#[test]
fn invocation_collector_has_independent_exact_and_one_short_limits() {
    let header = independent_collector_header();
    let value_bytes = independent_value_definition_bytes();
    assert_eq!(
        header,
        std::mem::size_of::<InvocationTransportCollectorV1<'_>>()
    );
    assert_eq!(value_bytes, std::mem::size_of::<ValueDef>());
    let transport = SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary);
    // Transport wrapper + semantic dispatch + binding visit; each scalar
    // adds one component visit, one type node and a two-work vector push.
    // The first component reserves four rows with three setup work units.
    for (binding, components, required, first_growth_work) in [
        (SemanticValueBindingV1::Unit, 0, 3, 0),
        (visitor_scalar(1), 1, 10, 10),
        (
            SemanticValueBindingV1::Aggregate(vec![visitor_scalar(1), visitor_scalar(2)]),
            2,
            16,
            11,
        ),
    ] {
        let bytes = header + if components == 0 { 0 } else { 4 * value_bytes };
        for floor in [0, 73] {
            for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
                let prior = 11;
                let mut work =
                    CanonicalKernelIrWorkBudgetV1::new(prior + required - usize::from(work_short));
                let mut budget =
                    ArgumentBudgetV1::new(&mut work, floor + bytes - usize::from(storage_short));
                budget.charge_work(prior).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = scoped_slot_attempt_v29(&mut budget, |budget| {
                    budget.reserve_storage(header)?;
                    let mut collector = InvocationTransportCollectorV1 {
                        values: vec![],
                        nodes: 0,
                        budget,
                    };
                    transport.visit_transport_values(&binding, &[], &mut collector)?;
                    assert_eq!(collector.values.len(), components);
                    assert_eq!(
                        collector.values.capacity(),
                        if components == 0 { 0 } else { 4 }
                    );
                    drop(collector);
                    budget.release_storage(bytes)?;
                    Ok(())
                });
                assert_eq!(result.is_ok(), !work_short && !storage_short);
                assert_eq!(budget.storage(), floor);
                if storage_short {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ));
                    assert_eq!(budget.failed_storage(), Some(floor + bytes));
                    assert_eq!(budget.work(), prior + first_growth_work);
                    assert_eq!(
                        budget.peak_storage(),
                        floor + if components == 0 { 0 } else { header }
                    );
                } else {
                    assert_eq!(budget.failed_storage(), None);
                    if !work_short {
                        assert_eq!(budget.work(), prior + required);
                    }
                }
                drop(budget);
                assert_eq!(work.failed_work(), work_short.then_some(prior + required));
            }
        }
    }
}

#[test]
fn borrowed_transport_stops_at_first_refused_visit_and_preserves_prefix_order() {
    struct Stop {
        remaining: usize,
        calls: usize,
        values: Vec<ValueId>,
    }
    impl SemanticTransportVisitorV1 for Stop {
        type Error = &'static str;
        fn node(&mut self) -> Result<(), Self::Error> {
            self.calls += 1;
            if self.remaining == 0 {
                return Err("stop");
            }
            self.remaining -= 1;
            Ok(())
        }
        fn component(
            &mut self,
            value: ValueId,
            _: BorrowedTransportTypeV1<'_>,
        ) -> Result<(), Self::Error> {
            self.node()?;
            self.values.push(value);
            Ok(())
        }
        fn invalid(detail: &'static str) -> Self::Error {
            detail
        }
    }
    let binding = SemanticValueBindingV1::Aggregate(vec![visitor_scalar(7), visitor_scalar(9)]);
    let transport = SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary);
    // Three dispatch/root visits, then a binding and component visit per leaf.
    for allowed in 0..7 {
        let mut visitor = Stop {
            remaining: allowed,
            calls: 0,
            values: vec![],
        };
        assert_eq!(
            transport.visit_transport_values(&binding, &[], &mut visitor),
            Err("stop")
        );
        assert_eq!(visitor.calls, allowed + 1);
        assert_eq!(
            visitor.values,
            if allowed >= 5 {
                vec![ValueId(7)]
            } else {
                vec![]
            }
        );
    }
    let mut visitor = Stop {
        remaining: 7,
        calls: 0,
        values: vec![],
    };
    transport
        .visit_transport_values(&binding, &[], &mut visitor)
        .unwrap();
    assert_eq!(visitor.calls, 7);
    assert_eq!(visitor.values, vec![ValueId(7), ValueId(9)]);
}

#[test]
fn invocation_collector_cleanup_preserves_prior_denials_on_error_and_panic() {
    let header = independent_collector_header();
    let bytes = header + 4 * independent_value_definition_bytes();
    let transport = SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary);
    for behavior in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 73 + bytes);
        budget.reserve_storage(73).unwrap();
        assert!(budget.reserve_storage(10_000).is_err());
        assert!(budget.charge_work(20_000).is_err());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(&mut budget, |budget| {
                budget.reserve_storage(header)?;
                let mut collector = InvocationTransportCollectorV1 {
                    values: vec![],
                    nodes: 0,
                    budget,
                };
                transport.visit_transport_values(&visitor_scalar(17), &[], &mut collector)?;
                assert_eq!(
                    collector.values,
                    vec![ValueDef::new(ValueId(17), Type::Scalar(ScalarType::U32))]
                );
                match behavior {
                    1 => return Err(invocation_entry_error_v1()),
                    2 => panic!("invocation collector unwind"),
                    _ => {}
                }
                drop(collector);
                budget.release_storage(bytes)?;
                Ok(())
            })
        }));
        match behavior {
            0 => assert!(result.unwrap().is_ok()),
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 73);
        assert_eq!(budget.peak_storage(), 73 + bytes);
        assert_eq!(budget.failed_storage(), Some(10_073));
        assert_eq!(budget.work(), 10);
        drop(budget);
        assert_eq!(work.failed_work(), Some(20_000));
    }
}

fn cyclic_enum_analysis_owner_v1() -> ProductionSemanticSsaOwnerV1 {
    let original = noop_semantic_owner(&["cyclic_enum_analysis"]);
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let enum_ty = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let variant = |index| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            4,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticBackendReprV1::scalar(tag),
            None,
            false,
            None,
            4,
            0,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap()
    };
    let types = vec![
        unit_type(),
        plain_bit_scalar_type(
            10,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([12; 32]),
            SemanticLayoutIdentityV1::from_sha256([13; 32]),
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                4,
                4,
                SemanticBackendReprV1::scalar(tag),
                false,
                SemanticEnumLayoutV1::new(
                    vec![variant(0), variant(1)],
                    SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::enum_type(
                u32_ty,
                vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                ],
            )
            .unwrap(),
        ),
    ];
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            enum_ty,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let blocks = vec![
        block(
            40,
            vec![assign(
                2,
                u32_ty,
                SemanticRvalueKindV1::Discriminant(place(1, enum_ty)),
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, u32_ty)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            41,
            vec![assign(
                1,
                enum_ty,
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(1), vec![])
                    .unwrap(),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0)),
        ),
        block(42, vec![], SemanticTerminatorKindV1::Return),
    ];
    let locals = [unit, enum_ty, u32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([30 + index as u8; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source,
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([21; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([22; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([23; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([24; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([25; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(
        original.semantic().functions()[0]
            .kernel_entry()
            .unwrap()
            .clone(),
    );
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn admitted_cyclic_enum_analysis_keeps_invocation_definition_distinct_from_header_phi() {
    let owner = cyclic_enum_analysis_owner_v1();
    let semantic = owner.source_semantic();
    let function = &semantic.functions()[0];
    let source = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let producers = semantic_option_producers_v1(function, semantic.callables()).unwrap();
    let option = SemanticOptionDominanceV1::analyze(function, &producers).unwrap();
    let mut plan = SemanticControlFlowSsaPlanV1::analyze(
        SemanticSsaTransportInputV1 {
            types: semantic.types(),
            callables: semantic.callables(),
            function,
            semantic_function: SemanticFunctionIdV1::from_index(0),
        },
        source,
        &option,
        &BTreeMap::new(),
        100_000,
        100_000,
    )
    .unwrap();
    let phi = SsaValueV1::BlockArgument {
        block: SsaBlockIdV1::new(0),
        variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
    };
    assert_eq!(plan.live_in(0), &[1]);
    assert_ne!(plan.entry_definitions[&1], phi);
    assert_eq!(plan.entry_value(function, 0, 1), Some(phi));
    let original = source.plan().entry_arguments();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].value(), plan.entry_definitions[&1]);
    let facts =
        analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000)
            .unwrap();
    assert!(!facts.contains_key(&(0, phi)));
    assert_eq!(facts.get(&(1, phi)), Some(&0));
    let saved = plan
        .block_entry_values
        .insert((0, 1), plan.entry_definitions[&1]);
    assert!(matches!(
        analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    match saved {
        Some(value) => {
            plan.block_entry_values.insert((0, 1), value);
        }
        None => {
            plan.block_entry_values.remove(&(0, 1));
        }
    }
    assert_eq!(
        analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000)
            .unwrap(),
        facts
    );
    plan.live_in.get_mut(&0).unwrap().clear();
    assert!(matches!(
        analyze_promoted_enum_variants_v1(semantic.types(), function, &plan, 100_000, 100_000),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}
