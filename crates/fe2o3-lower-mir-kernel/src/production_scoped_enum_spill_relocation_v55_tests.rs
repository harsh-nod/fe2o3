fn enum_spill_helper_owner_v55() -> ProductionSemanticSsaOwnerV1 {
    let owner = suffix_owner(SuffixCase::Ordinary);
    let semantic = owner.source_semantic();
    let mut types = semantic.types().to_vec();
    let enumeration = SemanticTypeIdV1::from_index(types.len() as u32);
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let variants = [vec![], vec![4]]
        .into_iter()
        .enumerate()
        .map(|(variant, offsets)| {
            SemanticEnumVariantLayoutV1::from_rustc(
                variant as u32,
                8,
                4,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                900 + variant as u64,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([239; 32]),
        SemanticLayoutIdentityV1::from_sha256([239; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            4,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
            ],
        )
        .unwrap(),
    ));
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    assert_eq!(original.locals().len(), 4);
    let mut locals = original.locals().to_vec();
    locals.push(local(140, enumeration, SemanticLocalRoleV1::Temporary));
    locals.push(local(141, U32, SemanticLocalRoleV1::Temporary));
    let constructor = |variant, values| {
        assign(
            place(4, enumeration),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(variant),
                    values,
                )
                .unwrap(),
            ),
        )
    };
    let join = || {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(3),
        ))
    };
    functions[3] = function(
        130,
        original.role(),
        original.abi().clone(),
        locals,
        vec![
            block(
                150,
                original.blocks()[0].statements().to_vec(),
                switch(SemanticOperandV1::Copy(place(3, U32)), 1, 2),
            ),
            block(151, vec![constructor(0, vec![])], join()),
            block(152, vec![constructor(1, vec![literal(17)])], join()),
            block(
                153,
                vec![assign(
                    place(5, U32),
                    SemanticRvalueKindV1::Discriminant(place(4, enumeration)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn observe_compiler_spill_relocation_v55(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    capture(source, instances, emitted, slots, budget)?;
    scoped_slot_relocation_v29::check_compiler_spill_test_permits_v55(
        instances, emitted, slots, budget,
    )
}

fn run_compiler_spill_relocation_v55(
    work: usize,
    storage: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    run_suffix_owner(
        enum_spill_helper_owner_v55,
        observe_compiler_spill_relocation_v55,
        work,
        storage,
        |output, _, _| {
            verify(output, SuffixCase::Ordinary);
            let pending = &output.pending;
            let entry = &pending.function.body.as_ref().unwrap().blocks[0];
            let spills: Vec<_> = pending
                .coordinates
                .spans
                .rows
                .iter()
                .filter(|row| {
                    matches!(row.source, InstanceSpanSourceV1::Synthetic(source)
                if source.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
                })
                .collect();
            assert_eq!(
                spills.len(),
                2,
                "two calls have distinct helper spill instances"
            );
            assert_ne!(spills[0].instance, spills[1].instance);
            for row in spills {
                assert_eq!(row.segments[1], None);
                let span = row.segments[0].unwrap();
                assert_eq!((span.block, span.count), (entry.id, 1));
                assert!(matches!(
                    entry.operations[span.first as usize].kind,
                    OperationKind::Alloca {
                        element: Type::Scalar(ScalarType::U32),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    }
                ));
            }
            assert_eq!(
                entry
                    .operations
                    .iter()
                    .filter(|operation| matches!(operation.kind, OperationKind::Alloca { .. }))
                    .count(),
                output.source_slots.slots.len() + 2
            );
            Ok(())
        },
    )
}

#[test]
fn compiler_spill_relocation_preserves_repeated_helper_initializers_and_exact_spans() {
    run_compiler_spill_relocation_v55(LIMIT, LIMIT).0.unwrap();
    assert!(REACHED.get() > 0);
}

#[test]
fn compiler_spill_relocation_repeated_helpers_have_exact_and_one_short_resources() {
    let measured = run_compiler_spill_relocation_v55(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = run_compiler_spill_relocation_v55(measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for work_short in [false, true] {
        let result = run_compiler_spill_relocation_v55(
            measured.1 - usize::from(work_short),
            measured.2 - usize::from(!work_short),
        );
        let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource)) = result.0
        else {
            panic!("one-short relocation must refuse its exact resource");
        };
        match resource {
            ArgumentResourceV1::Work(error) if work_short => {
                assert_eq!(error.actual(), measured.1);
                assert_eq!(error.limit(), measured.1 - 1);
            }
            ArgumentResourceV1::Storage(error) if !work_short => {
                assert_eq!(error.actual(), measured.2);
                assert_eq!(error.limit(), measured.2 - 1);
            }
            other => panic!("wrong relocation resource: {other:?}"),
        }
    }
}
