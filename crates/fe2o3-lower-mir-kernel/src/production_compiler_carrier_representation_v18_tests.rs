use super::*;

#[test]
fn aggregate_value_reconstruction_keeps_selected_pointer_representation() {
    for (space, source_space, legacy_space) in [
        (0, AddressSpace::Generic, AddressSpace::Global),
        (1, AddressSpace::Global, AddressSpace::Global),
        (3, AddressSpace::Workgroup, AddressSpace::Workgroup),
        (4, AddressSpace::Constant, AddressSpace::Constant),
        (5, AddressSpace::Private, AddressSpace::Private),
    ] {
        let types = types(space);
        for (representation, expected) in [
            (ExecutionCfgRepresentationV29::OriginalSource, source_space),
            (ExecutionCfgRepresentationV29::LegacyAbi, legacy_space),
        ] {
            let actual = values(expected);
            let rebuilt = binding_from_value_defs_with_representation_v29(
                &types,
                PAIR,
                &actual,
                representation,
            )
            .unwrap();
            let SemanticValueBindingV1::Aggregate(fields) = rebuilt else {
                panic!("expected aggregate");
            };
            assert_eq!(fields.len(), 2);
            assert!(
                matches!(&fields[0], SemanticValueBindingV1::Value { id, ty }
                if *id == ValueId(7) && *ty == pointer(expected))
            );
            assert!(
                matches!(&fields[1], SemanticValueBindingV1::Value { id, ty }
                if *id == ValueId(8) && *ty == Type::Scalar(ScalarType::U32))
            );
            for wrong in [
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    expected,
                    AccessMode::ReadWrite,
                ),
                Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    expected,
                    AccessMode::ReadOnly,
                ),
                pointer(if expected == AddressSpace::Global {
                    AddressSpace::Generic
                } else {
                    AddressSpace::Global
                }),
            ] {
                let values = [
                    ValueDef::new(ValueId(7), wrong),
                    ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
                ];
                assert!(matches!(
                    binding_from_value_defs_with_representation_v29(
                        &types,
                        PAIR,
                        &values,
                        representation,
                    ),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "aggregate SSA pointer component type changed",
                        ..
                    })
                ));
            }
        }
        assert!(binding_from_value_defs(&types, PAIR, &values(legacy_space)).is_ok());
    }
    assert!(matches!(
        binding_from_value_defs(&types(0), PAIR, &values(AddressSpace::Generic)),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "aggregate SSA pointer component type changed",
            ..
        })
    ));
}

#[test]
fn ordinary_carrier_representation_survives_paid_and_unpaid_round_trip() {
    let types = types(0);
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        let expected = match representation {
            ExecutionCfgRepresentationV29::LegacyAbi => AddressSpace::Global,
            ExecutionCfgRepresentationV29::OriginalSource => AddressSpace::Generic,
        };
        for paid in [false, true] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let mut allocation = if paid {
                CompilerCarrierAllocationV29::paid_with_representation(&mut budget, representation)
                    .unwrap()
            } else {
                CompilerCarrierAllocationV29 {
                    budget: None,
                    representation,
                }
            };
            let actual = allocation.ordinary_types(&types, PAIR).unwrap();
            assert_eq!(actual, [pointer(expected), Type::Scalar(ScalarType::U32)]);
            let correct = values(expected);
            let result = allocation
                .ordinary_binding(&types, PAIR, &correct, true)
                .unwrap();
            let SemanticValueBindingV1::Aggregate(fields) = result else {
                panic!("aggregate binding");
            };
            assert!(
                matches!(&fields[0], SemanticValueBindingV1::Value { ty, .. } if *ty == pointer(expected))
            );
            let wrong = values(if expected == AddressSpace::Global {
                AddressSpace::Generic
            } else {
                AddressSpace::Global
            });
            assert!(matches!(
                allocation.ordinary_binding(&types, PAIR, &wrong, true),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "aggregate SSA pointer component type changed",
                    ..
                })
            ));
        }
    }
}

#[test]
fn nonnominal_transport_preserves_source_policy_and_authenticated_direct_types() {
    let types = types(0);
    let ordinary = SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary);
    let mut direct = BTreeMap::new();
    direct.insert(9, pointer(AddressSpace::Global));
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        let expected = match representation {
            ExecutionCfgRepresentationV29::LegacyAbi => AddressSpace::Global,
            ExecutionCfgRepresentationV29::OriginalSource => AddressSpace::Generic,
        };
        let physical = ordinary
            .transport_types_with_representation_v29(&types, PAIR, &direct, representation)
            .unwrap();
        assert_eq!(physical, [pointer(expected), Type::Scalar(ScalarType::U32)]);
        let actual = values(expected);
        assert!(
            ordinary
                .binding_from_transport_with_representation_v29(
                    &types,
                    PAIR,
                    &actual,
                    &physical,
                    representation
                )
                .is_ok()
        );
        let direct_transport = SemanticPromotedTransportV1::DirectParameter { parameter_local: 9 };
        assert_eq!(
            direct_transport
                .transport_types_with_representation_v29(&types, POINTER, &direct, representation)
                .unwrap(),
            [pointer(AddressSpace::Global)]
        );
        let global = [ValueDef::new(ValueId(11), pointer(AddressSpace::Global))];
        assert!(
            direct_transport
                .binding_from_transport_with_representation_v29(
                    &types,
                    POINTER,
                    &global,
                    &[pointer(AddressSpace::Global)],
                    representation
                )
                .is_ok()
        );
        let generic = [ValueDef::new(ValueId(11), pointer(AddressSpace::Generic))];
        assert!(matches!(
            direct_transport.binding_from_transport_with_representation_v29(
                &types,
                POINTER,
                &generic,
                &[pointer(AddressSpace::Global)],
                representation
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "promoted direct parameter changed its authenticated ABI carrier",
                ..
            })
        ));
    }
    assert_eq!(
        ordinary.transport_types(&types, PAIR, &direct).unwrap(),
        [pointer(AddressSpace::Global), Type::Scalar(ScalarType::U32)]
    );
}

#[test]
fn paid_carrier_representation_header_is_charged_before_any_query() {
    let floor = 19;
    let header = std::mem::size_of::<CompilerCarrierAllocationV29<'_>>();
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        for enough in [false, true] {
            let limit = floor + header - usize::from(!enough);
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let result =
                CompilerCarrierAllocationV29::paid_with_representation(&mut budget, representation);
            if enough {
                drop(result.unwrap());
                assert_eq!(budget.storage(), floor + header);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error))) if error.actual() == floor + header && error.limit() == limit)
                );
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(budget.work(), 0);
        }
    }
}
