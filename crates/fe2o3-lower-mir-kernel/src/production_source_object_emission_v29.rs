include!("production_source_object_aggregate_emission_v29.rs");
include!("production_source_object_entry_v29.rs");
include!("production_source_object_loans_v29.rs");

#[cfg(test)]
fn selected_pointer_test_owner_v29(
    base: ProductionSemanticSsaOwnerV1,
    immutable: bool,
    graph: u8,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let span = original.blocks()[0].source();
    let unit = SemanticTypeIdV1::from_index(0);
    let word = SemanticTypeIdV1::from_index(1);
    let raw = SemanticTypeIdV1::from_index(2);
    let indirect = SemanticTypeIdV1::from_index(3);
    let mut types = source.types()[..2].to_vec();
    let mutability = if immutable {
        SemanticMutabilityV1::Immutable
    } else {
        SemanticMutabilityV1::Mutable
    };
    for (tag, pointee, mutable) in [
        (171, word, mutability),
        (172, raw, SemanticMutabilityV1::Mutable),
    ] {
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Raw,
                        mutable,
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
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
            ),
        );
    }
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let deref = |local, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let assign = |destination: SemanticPlaceV1, value| {
        SemanticStatementV1::new(
            span,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let copy = |local, ty| SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(local, ty)));
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            span,
            statements,
            SemanticTerminatorV1::new(span, terminator),
        )
        .unwrap()
    };
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            span,
        )
    };
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    };
    let abi = |kernel, argument, output| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([if kernel { 241 } else { 242 }; 32]),
            original.abi().layout_identity(),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(direct(argument))],
            if output == unit {
                SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore)
            } else {
                direct(output)
            },
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap()
    };
    let function = |tag, role, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            span,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let call = |input, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Copy(place(input, raw))],
                Some(SemanticCallDestinationV1::new(
                    place(destination, raw),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let mut first = vec![
        assign(place(2, word), copy(1, word)),
        assign(
            place(3, raw),
            SemanticRvalueKindV1::AddressOf {
                place: place(2, word),
                mutability,
            },
        ),
    ];
    if graph != 3 {
        first.push(assign(
            place(4, indirect),
            SemanticRvalueKindV1::AddressOf {
                place: place(3, raw),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ));
        first.push(assign(place(5, raw), copy(3, raw)));
        first.push(assign(
            place(5, raw),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(deref(4, raw))),
        ));
        if !immutable {
            first.push(assign(deref(3, word), copy(1, word)));
        }
        first.push(assign(
            place(6, word),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(deref(3, word))),
        ));
    } else {
        first.push(assign(place(5, raw), copy(3, raw)));
    }
    let finish = || {
        block(
            250,
            vec![assign(
                place(0, unit),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    unit,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            )],
            SemanticTerminatorKindV1::Return,
        )
    };
    let switch = |yes, no| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(1, word)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
        )
        .unwrap(),
    };
    let blocks = match graph {
        0 => vec![
            block(243, first, call(5, 7, 1)),
            block(244, vec![], call(7, 8, 2)),
            finish(),
        ],
        1 => vec![
            block(243, first, switch(1, 2)),
            block(
                244,
                vec![assign(place(5, raw), copy(3, raw))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                245,
                vec![assign(
                    place(5, raw),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(5, raw))),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(246, vec![], call(5, 7, 4)),
            block(247, vec![], call(7, 8, 5)),
            finish(),
        ],
        2 => vec![
            block(
                243,
                first,
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(244, vec![], switch(2, 3)),
            block(
                245,
                vec![assign(place(5, raw), copy(3, raw))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(246, vec![], call(5, 7, 4)),
            block(247, vec![], call(7, 8, 5)),
            finish(),
        ],
        3 => vec![
            block(
                243,
                first,
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            finish(),
        ],
        _ => panic!("selected-pointer fixture graph"),
    };
    let root = function(
        241,
        SemanticFunctionRoleV1::KernelRoot,
        abi(true, word, unit),
        vec![
            local(230, unit, SemanticLocalRoleV1::Return),
            local(231, word, SemanticLocalRoleV1::Argument(0)),
            local(232, word, SemanticLocalRoleV1::Temporary),
            local(233, raw, SemanticLocalRoleV1::Temporary),
            local(234, indirect, SemanticLocalRoleV1::Temporary),
            local(235, raw, SemanticLocalRoleV1::Temporary),
            local(236, word, SemanticLocalRoleV1::Temporary),
            local(237, raw, SemanticLocalRoleV1::Temporary),
            local(238, raw, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper = function(
        242,
        SemanticFunctionRoleV1::InternalHelper,
        abi(false, raw, raw),
        vec![
            local(239, raw, SemanticLocalRoleV1::Return),
            local(240, raw, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            251,
            vec![assign(place(0, raw), copy(1, raw))],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let functions = if graph == 3 {
        vec![root]
    } else {
        vec![root, helper]
    };
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[cfg(test)]
fn with_selected_pointer_test_plan_v29(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_selected_pointer_test_root_plan_v29(owner, |plan, _, budget| consume(plan, budget))
}

#[cfg(test)]
fn with_selected_pointer_test_root_plan_v29(
    mut owner: ProductionSemanticSsaOwnerV1,
    consume: impl for<'owner, 'view, 'root, 'source, 'work> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        (),
        ProductionSemanticKirErrorV1,
    >,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(193)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let demands =
                source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, budget)
                    .unwrap();
            let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
                &owner,
                demands.types(&owner, budget).unwrap(),
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                budget,
            )
            .unwrap();
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let result = source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                &mut layouts,
                instances,
                None,
                lens,
                budget,
                |plan, root, budget| {
                    // The callback returns no owned values. Drop its query
                    // envelopes and test filler before the source-owner scope.
                    with_canonical_call_scratch_v1(budget, |budget| consume(plan, root, budget))
                        .map_err(Into::into)
                },
            );
            let cleanup = layouts.release(budget);
            let discarded = demands.discard(budget);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                result.and(cleanup).and(discarded),
            )
        },
    )
    .unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 193);
    result
}

#[cfg(test)]
fn projected_pointer_test_owner_v29(
    base: ProductionSemanticSsaOwnerV1,
    immutable: bool,
    mode: u8,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = selected_pointer_test_owner_v29(base, immutable, 3);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let span = original.blocks()[0].source();
    let unit = SemanticTypeIdV1::from_index(0);
    let word = SemanticTypeIdV1::from_index(1);
    let raw = SemanticTypeIdV1::from_index(2);
    let indirect = SemanticTypeIdV1::from_index(3);
    let record = SemanticTypeIdV1::from_index(4);
    let array = SemanticTypeIdV1::from_index(5);
    let record_pointer = SemanticTypeIdV1::from_index(6);
    let mut types = source.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([181; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![raw, word]).unwrap()),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([182; 32]),
        SemanticLayoutIdentityV1::from_sha256([182; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::array(8, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: raw,
            length: 2,
        },
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([183; 32]),
        SemanticLayoutIdentityV1::from_sha256([183; 32]),
        source.types()[indirect.index() as usize].layout().clone(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                record,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let array_holder = if mode >= 7 {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([184; 32]),
            SemanticLayoutIdentityV1::from_sha256([184; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![array]).unwrap()),
        ));
        ty
    } else {
        array
    };
    let plain =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let projected = |local, steps: Vec<(SemanticProjectionKindV1, SemanticTypeIdV1)>| {
        let ty = steps.last().unwrap().1;
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            steps
                .into_iter()
                .map(|(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
                .collect(),
            ty,
        )
        .unwrap()
    };
    let field = |index, ty| projected(11, vec![(SemanticProjectionKindV1::Field(index), ty)]);
    let element = |offset, from_end| {
        let mut steps = Vec::new();
        if array_holder != array {
            steps.push((SemanticProjectionKindV1::Field(0), array));
        }
        steps.push((
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length: 2,
                from_end,
            },
            raw,
        ));
        projected(13, steps)
    };
    let deref = |local, ty| projected(local, vec![(SemanticProjectionKindV1::Dereference, ty)]);
    let assign = |destination: SemanticPlaceV1, value| {
        SemanticStatementV1::new(
            span,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let copy = |source| SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source));
    let address = |place, mutability| SemanticRvalueKindV1::AddressOf { place, mutability };
    let permission = if immutable {
        SemanticMutabilityV1::Immutable
    } else {
        SemanticMutabilityV1::Mutable
    };
    let mut statements = original.blocks()[0].statements()[..2].to_vec();
    statements.extend([
        assign(plain(9, word), copy(plain(1, word))),
        assign(plain(10, raw), address(plain(9, word), permission)),
        assign(field(0, raw), copy(plain(3, raw))),
        assign(field(1, word), copy(plain(1, word))),
        assign(element(0, false), copy(plain(3, raw))),
        assign(element(1, false), copy(plain(10, raw))),
        assign(
            plain(12, record_pointer),
            address(plain(11, record), SemanticMutabilityV1::Mutable),
        ),
        assign(
            plain(14, indirect),
            address(element(1, false), SemanticMutabilityV1::Mutable),
        ),
    ]);
    let holder = if matches!(mode, 2 | 3 | 5..=8) {
        element(1, mode == 3)
    } else {
        field(0, raw)
    };
    statements.push(assign(
        plain(4, indirect),
        address(holder, SemanticMutabilityV1::Mutable),
    ));
    statements.push(assign(deref(4, raw), copy(plain(10, raw))));
    statements.push(SemanticStatementV1::new(
        span,
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            deref(4, raw),
            SemanticOperandV1::Copy(plain(3, raw)),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    statements.push(assign(plain(5, raw), copy(deref(4, raw))));
    let final_place = match mode {
        0 => field(1, word),
        1 => projected(
            12,
            vec![
                (SemanticProjectionKindV1::Dereference, record),
                (SemanticProjectionKindV1::Field(1), word),
            ],
        ),
        2 | 3 => deref(5, word),
        4 => projected(
            11,
            vec![
                (SemanticProjectionKindV1::Field(0), raw),
                (SemanticProjectionKindV1::Dereference, word),
            ],
        ),
        5..=8 => {
            let holder = element(1, matches!(mode, 6 | 8));
            let mut steps = holder.projections().to_vec();
            steps.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, word).unwrap(),
            );
            SemanticPlaceV1::new(holder.local(), steps, word).unwrap()
        }
        _ => panic!("projected pointer fixture mode"),
    };
    statements.push(assign(
        plain(7, raw),
        address(final_place.clone(), permission),
    ));
    if !immutable {
        statements.push(assign(final_place.clone(), copy(plain(1, word))));
    }
    statements.push(assign(plain(6, word), copy(final_place)));
    statements.push(assign(
        plain(0, unit),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            unit,
            SemanticConstantValueV1::ZeroSized,
        ))),
    ));
    let mut locals = original.locals().to_vec();
    for (tag, ty) in [
        (239, word),
        (240, raw),
        (241, record),
        (242, record_pointer),
        (243, array_holder),
        (244, indirect),
    ] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            span,
        ));
    }
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        span,
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([190; 32]),
                span,
                statements,
                SemanticTerminatorV1::new(span, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn source_reference_assignment_pointer_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(Type, fe2o3_kernel_ir::StorageLayoutIdV1)>, ProductionSemanticKirErrorV1> {
    source_reference_write_pointer_v29(
        plan,
        site,
        assignment.destination(),
        assignment.value().result_type(),
        budget,
    )
}

fn source_reference_write_pointer_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    destination: &SemanticPlaceV1,
    expected: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(Type, fe2o3_kernel_ir::StorageLayoutIdV1)>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        budget.source_reference_owner_v29(plan)?;
        source_reference_owned_prepay_v29::<Option<(Type, fe2o3_kernel_ir::StorageLayoutIdV1)>>(
            plan, budget,
        )?;
        budget.source_reference_charge_v29(plan, 6)?;
        let original = plan
            .instances
            .instance(site.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .blocks()
                    .get(site.block.index() as usize)
            })
            .and_then(|block| {
                site.statement
                    .and_then(|statement| block.statements().get(statement))
            })
            .ok_or(ArgumentResourceV1::Accounting)?;
        let (original, result_type) = match original.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                (assignment.destination(), assignment.value().result_type())
            }
            SemanticStatementKindV1::Store(store) => (store.destination(), store.value().ty()),
            _ => return Err(ArgumentResourceV1::Accounting.into()),
        };
        if !std::ptr::eq(original, destination)
            || expected != result_type
            || expected != destination.ty()
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let resolved = if destination.projections().is_empty() {
            None
        } else {
            Some(source_reference_access_at_v29(
                plan,
                site,
                destination,
                SourceReferenceAccessV29::Write,
                budget,
            )?)
        };
        let key =
            source_reference_access_key_v29(site, destination, SourceReferenceAccessV29::Write);
        charge_execution_cfg_lookup_v29(plan.representation_demand_sites.len(), budget)?;
        let rows = plan
            .representation_demand_sites
            .get(&key)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if rows.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut selected = None;
        let mut plain = false;
        for &index in rows {
            budget.source_reference_charge_v29(plan, 7)?;
            let demand = plan
                .representation_demands
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if demand.selector_source.is_some() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if let Some(resolved) = resolved {
                budget.source_reference_charge_v29(
                    plan,
                    argument_sum_v1(&[5, demand.projections.len()])?,
                )?;
                if demand.instance != resolved.instance
                    || demand.local != resolved.local
                    || demand.generation != resolved.generation
                    || resolved.shared_path
                    || plan.projections.get(demand.projections.clone())
                        != plan.projections.get(resolved.projections.clone())
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
            } else if demand.instance != site.instance
                || demand.local != destination.local()
                || !demand.projections.is_empty()
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let ty =
                budget.source_reference_selected_pointer_type_v29(plan, demand.node, expected)?;
            let Some(ty) = ty else {
                plain = true;
                continue;
            };
            let schema = plan
                .selected_storage
                .get(demand.node)
                .copied()
                .flatten()
                .ok_or(ArgumentResourceV1::Accounting)?;
            if let Some((previous, previous_schema)) = &selected {
                if *previous_schema != schema || !invocation_equal_types_v1(previous, &ty, budget)?
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
            } else {
                selected = Some((ty, schema));
            }
        }
        if plain && selected.is_some() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(selected)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_place_pointer_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    prefix: usize,
    access: SourceReferenceAccessV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(Type, fe2o3_kernel_ir::StorageLayoutIdV1)>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        budget.source_reference_owner_v29(plan)?;
        source_reference_owned_prepay_v29::<Option<(Type, fe2o3_kernel_ir::StorageLayoutIdV1)>>(
            plan, budget,
        )?;
        budget.source_reference_charge_v29(plan, 5)?;
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let source_block = function
            .blocks()
            .get(site.block.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let expected = if prefix == 0 {
            function
                .locals()
                .get(place.local().index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?
                .ty()
        } else {
            place
                .projections()
                .get(prefix - 1)
                .ok_or(ArgumentResourceV1::Accounting)?
                .result_type()
        };
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(expected.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(None);
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
        {
            return Ok(None);
        }
        let node = if prefix < place.projections().len() {
            let key = (source_reference_access_key_v29(site, place, access), prefix);
            charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
            let Some(raw) = plan.raw_accesses.get(&key) else {
                return Ok(None);
            };
            budget.source_reference_charge_v29(plan, 8)?;
            if raw.site != site
                || raw.source != place as *const SemanticPlaceV1 as usize
                || raw.access != access
                || raw.projection != prefix
                || raw.ty != place.ty()
                || place.projections()[prefix].kind() != SemanticProjectionKindV1::Dereference
                || raw.pointee != pointer.pointee()
                || !matches!(plan.nodes.get(raw.holder.node).map(|row| row.kind),
                    Some(SourceReferenceNodeKindV29::Address(set)) if set == raw.set)
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            raw.holder.node
        } else if let Some(statement) = site.statement {
            let (original, destination, result_type) = match source_block
                .statements()
                .get(statement)
                .ok_or(ArgumentResourceV1::Accounting)?
                .kind()
            {
                SemanticStatementKindV1::Assign(assignment) => {
                    let original = match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
                        ) => source,
                        SemanticRvalueKindV1::Load(load) => load.source(),
                        _ => return Err(ArgumentResourceV1::Accounting.into()),
                    };
                    (
                        original,
                        assignment.destination(),
                        assignment.value().result_type(),
                    )
                }
                SemanticStatementKindV1::Store(store) => {
                    let (SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source)) =
                        store.value()
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    (source, store.destination(), store.value().ty())
                }
                _ => return Err(ArgumentResourceV1::Accounting.into()),
            };
            if !std::ptr::eq(original, place) || expected != place.ty() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            return source_reference_write_pointer_v29(
                plan,
                site,
                destination,
                result_type,
                budget,
            );
        } else {
            let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            let mut ordinal = None;
            for (index, operand) in call.arguments().iter().enumerate() {
                budget.source_reference_charge_v29(plan, 1)?;
                if matches!(operand, SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source)
                    if std::ptr::eq(source, place))
                {
                    if ordinal.is_some() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    ordinal =
                        Some(u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?);
                }
            }
            let role = SourceReferenceBoundaryRoleV29::Argument(
                ordinal.ok_or(ArgumentResourceV1::Accounting)?,
            );
            let key = source_reference_boundary_key_v29(site, role);
            charge_execution_cfg_lookup_v29(plan.boundary_sites.len(), budget)?;
            let &index = plan
                .boundary_sites
                .get(&key)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let row = plan
                .boundary_values
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if row.site != site || row.role != role {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            row.node
        };
        let Some(ty) = budget.source_reference_selected_pointer_type_v29(plan, node, expected)?
        else {
            return Ok(None);
        };
        let schema = plan
            .selected_storage
            .get(node)
            .copied()
            .flatten()
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(Some((ty, schema)))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_object_projection_v29<'path>(
    plan: &SourceReferencePlanV29<'_, '_>,
    ty: SemanticTypeIdV1,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    path: &'path [SemanticProjectionV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<source_storage_v29::SourceSelectedComponentV29<'path>>, ProductionSemanticKirErrorV1>
{
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        plan.charge(path.len(), budget)?;
        if path.iter().any(|projection| {
            matches!(
                projection.kind(),
                SemanticProjectionKindV1::Dereference | SemanticProjectionKindV1::Index(_)
            )
        }) {
            return Err(source_reference_error_v29(
                "typed object projection requires an exact static original path",
            ));
        }
        let layouts = plan
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?
            .source_layouts(plan.instances, budget)?;
        let mut components = source_reference_owned_vec_v29(plan, path.len(), budget)?;
        let result = layouts.visit_selected_components(
            plan.instances.owner(),
            ty,
            schema,
            path,
            budget,
            |component, budget| {
                source_reference_owned_push_v29(plan, &mut components, component, budget)
            },
        )?;
        if !matches!(
            result,
            source_storage_v29::SourceSelectedProjectionV29::Physical { .. }
        ) {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(components)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn source_object_project_path_v29(
        &mut self,
        place: &SemanticPlaceV1,
        first: usize,
        end: usize,
        mut endpoint: ScopedObjectEndpointV29,
        mut address: ValueId,
        permission: AccessMode,
        mut memory: MemoryAccess,
        operations: &mut Vec<Operation>,
    ) -> Result<(ScopedObjectEndpointV29, ValueId, MemoryAccess), ProductionSemanticKirErrorV1>
    {
        let components = self.with_emission_budget_v1(|this, budget| {
            let plan = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or(ArgumentResourceV1::Accounting)?
                .plan;
            source_reference_owned_prepay_v29::<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>(
                plan, budget,
            )?;
            let path = place
                .projections()
                .get(first..end)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.source_object_projection_v29(
                plan,
                endpoint.projected_type,
                endpoint.projected_schema,
                path,
            )
        })?;
        for (ordinal, component) in components.iter().enumerate() {
            self.with_emission_budget_v1(|_, budget| {
                source_reference_emission_prepay_v29::<(
                    Option<ScopedObjectProjectionV29>,
                    Option<ScopedObjectViewProjectionV29>,
                )>(budget)?;
                budget.charge_work(8)
            })?;
            use source_storage_v29::SourceSelectedComponentKindV29 as Component;
            let (step, view) = match component.kind {
                Component::Field { physical, .. } => {
                    let field =
                        u32::try_from(physical).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    (
                        Some(ScopedObjectProjectionV29::Field(field)),
                        Some(ScopedObjectViewProjectionV29::Field(field)),
                    )
                }
                Component::Variant { original, physical } => (
                    physical.then_some(ScopedObjectProjectionV29::Variant {
                        index: original,
                        access: memory,
                    }),
                    physical.then_some(ScopedObjectViewProjectionV29::Variant(original)),
                ),
                Component::Index { length, .. } => {
                    let SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    } = component.projection.kind()
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    let index = if from_end {
                        length
                            .checked_sub(offset)
                            .ok_or(ArgumentResourceV1::Accounting)?
                    } else {
                        offset
                    };
                    if length < minimum_length || index >= length {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    let index = self.emit_index_constant(operations, index)?;
                    (
                        Some(ScopedObjectProjectionV29::ArrayIndex(index)),
                        Some(ScopedObjectViewProjectionV29::ArrayElement),
                    )
                }
                Component::OmittedNominal { .. } => {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
            };
            let projected = self.with_emission_budget_v1(|this, budget| {
                let plan = this
                    .execution
                    .as_ref()
                    .and_then(|cursor| cursor.references)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .plan;
                source_reference_owned_prepay_v29::<ScopedObjectEndpointV29>(plan, budget)?;
                budget.source_reference_charge_v29(plan, 6)?;
                if component.source_type != endpoint.projected_type
                    || component.source_schema != endpoint.projected_schema
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let prefix = argument_sum_v1(&[first, ordinal, 1])?;
                let mut original = source_reference_owned_vec_v29(plan, prefix, budget)?;
                for &projection in place
                    .projections()
                    .get(..prefix)
                    .ok_or(ArgumentResourceV1::Accounting)?
                {
                    budget.source_reference_charge_v29(plan, 1)?;
                    original.push(ScopedObjectComponentV29::Original {
                        projection,
                        selector: None,
                    });
                }
                let recorder = this
                    .scoped_memory
                    .as_mut()
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let source_path = recorder.anchors.append_object_path(&original, budget)?;
                let path = if let Some(projection) = view {
                    let previous = recorder.anchors.object_path(endpoint.path, budget)?;
                    let mut path = source_reference_owned_vec_v29(
                        plan,
                        argument_sum_v1(&[previous.len(), 1])?,
                        budget,
                    )?;
                    budget.source_reference_charge_v29(plan, previous.len())?;
                    path.extend_from_slice(previous);
                    path.push(ScopedObjectComponentV29::View {
                        projection,
                        ty: component.result_type,
                    });
                    recorder.anchors.append_object_path(&path, budget)?
                } else {
                    endpoint.path
                };
                let ScopedObjectSourceV29::Place {
                    site, role, local, ..
                } = endpoint.source
                else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                Ok(ScopedObjectEndpointV29 {
                    source: ScopedObjectSourceV29::Place {
                        site,
                        role,
                        local,
                        prefix: u32::try_from(prefix)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    },
                    projected_type: component.result_type,
                    projected_schema: component
                        .result_schema
                        .ok_or(ArgumentResourceV1::Accounting)?,
                    source_path,
                    path,
                    ..endpoint
                })
            })?;
            if let Some(step) = step {
                let ty = self.with_emission_budget_v1(|this, budget| {
                    let plan = this
                        .execution
                        .as_ref()
                        .and_then(|cursor| cursor.references)
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .plan;
                    source_reference_owned_prepay_v29::<Type>(plan, budget)?;
                    budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                    Ok(Type::pointer(
                        Type::StorageObject(projected.projected_schema),
                        memory.address_space,
                        permission,
                    ))
                })?;
                let base = address;
                let binding = self.with_scoped_object_role_v29(
                    ScopedObjectRoleV29::Project {
                        source: endpoint,
                        projected,
                    },
                    |this| {
                        this.emit(
                            operations,
                            ty,
                            OperationKind::Storage(ScopedObjectOperationV29::Project {
                                base,
                                step,
                            }),
                        )
                    },
                )?;
                address = binding
                    .value()
                    .map_err(|_| ArgumentResourceV1::Accounting)?
                    .0;
                memory = MemoryAccess::new(memory.address_space, 1);
            }
            endpoint = projected;
        }
        Ok((endpoint, address, memory))
    }

    fn source_object_projected_root_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        holder: Option<SourceReferenceRawHolderV29>,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        self.with_emission_budget_v1(|this, budget| {
            let local = place.local().index();
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let objects = ScopedAllocationIdentityV29::OriginalObject { local, generation: 0 }
                ..=ScopedAllocationIdentityV29::OriginalObject { local, generation: u32::MAX };
            if this.retained_local_slots.range(objects).next().is_none() { return Ok(None); }
            let cursor = this.execution.as_ref().ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, argument_sum_v1(&[12, place.projections().len()])?)?;
            let site = execution_site_v29(block, statement);
            let frame = this.scoped_memory.as_ref().and_then(|recorder| recorder.frame).ok_or(ArgumentResourceV1::Accounting)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else { return Err(ArgumentResourceV1::Accounting.into()); };
            if frame.site != site || !scoped_object_original_place_v29(this.function, site, role).is_some_and(|original| std::ptr::eq(original, place)) {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let generation = if let Some(holder) = holder { holder.generation } else {
                let source = source_reference_access_at_v29(plan, SourceReferenceSiteV29 {
                    instance: cursor.instance, block, statement: statement.map(|value| value as usize),
                }, place, access, budget)?;
                if source.instance != cursor.instance || source.local != place.local() || source.source_local != place.local()
                    || source.ty != place.ty() || source.loan.is_some() || !source.traversed.is_empty()
                    || source.shared_path || plan.projections.get(source.projections.clone()) != Some(place.projections())
                { return Err(ArgumentResourceV1::Accounting.into()); }
                source.generation
            };
            let mapping = plan.physical_object_generation(cursor.instance, place.local(), generation, budget)?;
            let physical_generation = mapping.map_or(generation, |(_, _, row)| row.generation);
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let slot = this.retained_local_slots.get(&ScopedAllocationIdentityV29::OriginalObject { local, generation: physical_generation })
                .ok_or(ArgumentResourceV1::Accounting)?;
            let SemanticRetainedStorageV29::Object { cell, schema, bytes, alignment } = slot.storage else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            let root_type = this.function.locals().get(local as usize).ok_or(ArgumentResourceV1::Accounting)?.ty();
            if slot.semantic_type != root_type { return Err(ArgumentResourceV1::Accounting.into()); }
            let logical_cell = if let Some((logical, representative, physical)) = mapping {
                if representative != cell || physical.ty != root_type || physical.kind != SourceBackingKindV29::Object(schema) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                logical
            } else { cell };
            let checked = budget.source_object_storage_v29(plan, logical_cell, cursor.instance, place.local(), generation, schema)?;
            if !matches!(checked, SemanticRetainedStorageV29::Object { cell: c, schema: s, bytes: b, alignment: a }
                if (c, s, b, a) == (logical_cell, schema, bytes, alignment)) { return Err(ArgumentResourceV1::Accounting.into()); }
            let path = ScopedObjectPathV29 { first: 0, count: 0 };
            Ok(Some((ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Local { instance: cursor.instance, local: place.local(), generation },
                source: ScopedObjectSourceV29::Place { site, role, local: place.local(), prefix: 0 },
                root_type, projected_type: root_type, root_schema: schema, projected_schema: schema,
                source_path: path, path,
            }, slot.pointer, MemoryAccess::new(AddressSpace::Private, alignment))))
        })
    }

    fn source_object_read_value_v29(
        &mut self,
        source: ScopedObjectEndpointV29,
        address: ValueId,
        access: MemoryAccess,
        ty: Type,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let read = self.with_emission_budget_v1(|this, budget| {
            source_reference_emission_prepay_v29::<ScopedMemoryReadV29>(budget)?;
            budget.charge_work(5)?;
            let ScopedObjectSourceV29::Place {
                site, role, prefix, ..
            } = source.source
            else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            let place = scoped_object_original_place_v29(this.function, site, role)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let cursor = this
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let occurrence = scoped_payload_occurrence_v29(cursor, site, role, place, budget)?
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(ScopedMemoryReadV29 {
                site,
                role,
                prefix,
                ty: source.projected_type,
                occurrence,
            })
        })?;
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::Original(read),
            },
            |this| {
                this.emit(
                    operations,
                    ty,
                    OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, access }),
                )
            },
        )
    }

    fn source_object_leaf_type_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    ) -> Result<Type, ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let cursor = this
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let plan = references.plan;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Type>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 2)?;
            match this
                .types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            {
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)) => {
                    lower_scalar_type(this.types, place.ty())
                }
                Some(SemanticTypeShapeV1::Pointer(_)) => {
                    let site = SourceReferenceSiteV29 {
                        instance: cursor.instance,
                        block,
                        statement: statement.map(|statement| statement as usize),
                    };
                    let selected = if access == SourceReferenceAccessV29::Write {
                        source_reference_write_pointer_v29(plan, site, place, place.ty(), budget)?
                    } else {
                        source_reference_place_pointer_v29(
                            plan,
                            site,
                            place,
                            place.projections().len(),
                            access,
                            budget,
                        )?
                    };
                    let Some((ty, selected)) = selected else {
                        return budget.source_object_original_leaf_type_v29(
                            plan,
                            place.ty(),
                            schema,
                        );
                    };
                    if selected != schema {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok(ty)
                }
                _ => Err(scoped_object_allocation_error_v29()),
            }
        })
    }

    fn source_object_leaf_address_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        operations: &mut Vec<Operation>,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        if self.scoped_memory.is_none() {
            return Ok(None);
        }
        if place.projections().is_empty() {
            return self.source_object_local_endpoint_v29(block, statement, place, access);
        }
        let (first_dereference, last_dereference) = self.with_emission_budget_v1(|_, budget| {
            source_reference_emission_prepay_v29::<(Option<usize>, Option<usize>)>(budget)?;
            budget.charge_work(argument_product_v1(place.projections().len(), 2)?)?;
            Ok((
                place.projections().iter().position(|projection| {
                    projection.kind() == SemanticProjectionKindV1::Dereference
                }),
                place.projections().iter().rposition(|projection| {
                    projection.kind() == SemanticProjectionKindV1::Dereference
                }),
            ))
        })?;
        let Some(first_dereference) = first_dereference else {
            let Some((endpoint, address, memory)) =
                self.source_object_projected_root_v29(block, statement, place, access, None)?
            else {
                return Ok(None);
            };
            return self
                .source_object_project_path_v29(
                    place,
                    0,
                    place.projections().len(),
                    endpoint,
                    address,
                    AccessMode::ReadWrite,
                    memory,
                    operations,
                )
                .map(Some);
        };
        let aggregate_loan = self.with_emission_budget_v1(|this, budget| {
            source_reference_emission_prepay_v29::<bool>(budget)?;
            source_reference_emission_prepay_v29::<&SemanticTypeShapeV1>(budget)?;
            source_reference_emission_prepay_v29::<&fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1>(budget)?;
            budget.charge_work(6)?;
            Ok(matches!(this.types[if first_dereference == 0 {
                this.function.locals()[place.local().index() as usize].ty().index() as usize
            } else { place.projections()[first_dereference - 1].result_type().index() as usize }].shape(),
                SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.metadata() == SemanticPointerMetadataV1::None
                    && matches!(this.types[pointer.pointee().index() as usize].shape(),
                        SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_))))
        })?;
        if aggregate_loan
            && let Some(endpoint) = self
                .source_aggregate_object_endpoint_v29(block, statement, place, access, operations)?
        {
            return Ok(Some(endpoint));
        }
        if first_dereference + 1 == place.projections().len()
            && matches!(self.types[if first_dereference == 0 {
                self.function.locals()[place.local().index() as usize].ty().index() as usize
            } else { place.projections()[first_dereference - 1].result_type().index() as usize }].shape(),
                SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.metadata() == SemanticPointerMetadataV1::None)
            && let Some(endpoint) =
                self.source_object_loan_endpoint_v29(block, statement, place, access)?
        {
            return Ok(Some(endpoint));
        }
        let selected = self.with_emission_budget_v1(|this, budget| {
            let Some(cursor) = this.execution.as_ref() else {
                return Ok(None);
            };
            let Some(references) = cursor.references else {
                return Ok(None);
            };
            references.check(budget)?;
            let site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|statement| statement as usize),
            };
            source_reference_place_pointer_v29(
                references.plan,
                site,
                place,
                first_dereference,
                access,
                budget,
            )
        })?;
        let Some((expected, holder_schema)) = selected else {
            return Ok(None);
        };
        let (site, role, holder) = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<(
                SourceReferenceSiteV29,
                ExecutionOperandV29,
                SourceReferenceRawHolderV29,
            )>(plan, budget)?;
            budget.source_reference_charge_v29(
                plan,
                argument_sum_v1(&[12, place.projections().len()])?,
            )?;
            let site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|statement| statement as usize),
            };
            let frame = this
                .scoped_memory
                .as_ref()
                .and_then(|recorder| recorder.frame)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if frame.site != execution_site_v29(block, statement)
                || !scoped_object_original_place_v29(this.function, frame.site, role)
                    .is_some_and(|source| std::ptr::eq(source, place))
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let key = (
                source_reference_access_key_v29(site, place, access),
                first_dereference,
            );
            charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
            let holder = plan
                .raw_accesses
                .get(&key)
                .ok_or(ArgumentResourceV1::Accounting)?
                .holder;
            if holder.instance != cursor.instance
                || holder.local != place.local()
                || holder.count != first_dereference
                || holder.parent.is_some()
                || holder.shared_path
                || holder.selector_source.is_some()
                || plan
                    .projections
                    .get(holder.first..argument_sum_v1(&[holder.first, holder.count])?)
                    != place.projections().get(..first_dereference)
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok((site, role, holder))
        })?;
        let original_site = execution_site_v29(block, statement);
        let mut value = if let Some((endpoint, address, memory)) =
            self.source_object_projected_root_v29(block, statement, place, access, Some(holder))?
        {
            let (endpoint, address, memory) = self.source_object_project_path_v29(
                place,
                0,
                first_dereference,
                endpoint,
                address,
                AccessMode::ReadWrite,
                memory,
                operations,
            )?;
            if endpoint.projected_schema != holder_schema {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.source_object_read_value_v29(endpoint, address, memory, expected, operations)?
        } else {
            self.with_emission_budget_v1(|this, budget| {
                let mut value = this
                    .locals
                    .get(place.local().index() as usize)
                    .and_then(Option::as_ref)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                source_reference_emission_prepay_v29::<Option<&[SemanticValueBindingV1]>>(budget)?;
                let mut variant_fields = None;
                for projection in &place.projections()[..first_dereference] {
                    budget.charge_work(4)?;
                    if let SemanticProjectionKindV1::Downcast(index) = projection.kind() {
                        let SemanticValueBindingV1::Enum {
                            variant: Some(actual),
                            payloads,
                            ..
                        } = value
                        else {
                            return Err(ArgumentResourceV1::Accounting.into());
                        };
                        if *actual != index || variant_fields.is_some() {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
                        variant_fields = Some(
                            payloads
                                .get(&index)
                                .ok_or(ArgumentResourceV1::Accounting)?
                                .as_slice(),
                        );
                        continue;
                    }
                    let fields = if let Some(fields) = variant_fields.take() {
                        fields
                    } else {
                        let SemanticValueBindingV1::Aggregate(fields) = value else {
                            return Err(ArgumentResourceV1::Accounting.into());
                        };
                        fields.as_slice()
                    };
                    let index = match projection.kind() {
                        SemanticProjectionKindV1::Field(index) => index as usize,
                        SemanticProjectionKindV1::ConstantIndex {
                            offset,
                            minimum_length,
                            from_end,
                        } => {
                            let length = u64::try_from(fields.len())
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                            let index = if from_end {
                                length
                                    .checked_sub(offset)
                                    .ok_or(ArgumentResourceV1::Accounting)?
                            } else {
                                offset
                            };
                            if length < minimum_length || index >= length {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            usize::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?
                        }
                        _ => return Err(ArgumentResourceV1::Accounting.into()),
                    };
                    value = fields.get(index).ok_or(ArgumentResourceV1::Accounting)?;
                }
                if variant_fields.is_some() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let SemanticValueBindingV1::Value { ty, .. } = value else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                if !invocation_equal_types_v1(ty, &expected, budget)? {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                emission_clone_binding_v1(value, budget)
            })?
        };
        let mut index = first_dereference;
        loop {
            let projection = &place.projections()[index];
            let (endpoint, address, memory, permission) =
                self.with_emission_budget_v1(|this, budget| {
                    let references = this
                        .execution
                        .as_ref()
                        .and_then(|cursor| cursor.references)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let plan = references.plan;
                    let (expected, _) = source_reference_place_pointer_v29(
                        plan, site, place, index, access, budget,
                    )?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                    let SemanticValueBindingV1::Value {
                        id,
                        ty: Type::Pointer(pointer),
                    } = &value
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    let SemanticValueBindingV1::Value { ty, .. } = &value else {
                        unreachable!()
                    };
                    if !invocation_equal_types_v1(ty, &expected, budget)?
                        || (Some(index) == last_dereference
                            && access == SourceReferenceAccessV29::Write
                            && pointer.access != AccessMode::ReadWrite)
                    {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    let Type::StorageObject(schema) = pointer.pointee.as_ref() else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    source_reference_owned_prepay_v29::<(
                        ScopedObjectEndpointV29,
                        ValueId,
                        MemoryAccess,
                        AccessMode,
                    )>(plan, budget)?;
                    let mut components = source_reference_owned_vec_v29(plan, index + 1, budget)?;
                    for &projection in &place.projections()[..index + 1] {
                        budget.source_reference_charge_v29(plan, 1)?;
                        components.push(ScopedObjectComponentV29::Original {
                            projection,
                            selector: None,
                        });
                    }
                    let source_path = this
                        .scoped_memory
                        .as_mut()
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .anchors
                        .append_object_path(&components, budget)?;
                    let prefix =
                        u32::try_from(index + 1).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    Ok((
                        ScopedObjectEndpointV29 {
                            object: ScopedObjectIdentityV29::Reference {
                                instance: site.instance,
                                site: original_site,
                                role,
                                dereference_prefix: prefix,
                            },
                            source: ScopedObjectSourceV29::Place {
                                site: original_site,
                                role,
                                local: place.local(),
                                prefix,
                            },
                            root_type: projection.result_type(),
                            projected_type: projection.result_type(),
                            root_schema: *schema,
                            projected_schema: *schema,
                            source_path,
                            path: ScopedObjectPathV29 { first: 0, count: 0 },
                        },
                        *id,
                        MemoryAccess::new(pointer.address_space, 1),
                        pointer.access,
                    ))
                })?;
            let next = self.with_emission_budget_v1(|_, budget| {
                source_reference_emission_prepay_v29::<usize>(budget)?;
                budget.charge_work(place.projections().len() - index - 1)?;
                Ok(place.projections()[index + 1..]
                    .iter()
                    .position(|projection| {
                        projection.kind() == SemanticProjectionKindV1::Dereference
                    })
                    .map_or(place.projections().len(), |offset| index + 1 + offset))
            })?;
            let (endpoint, address, memory) = self.source_object_project_path_v29(
                place,
                index + 1,
                next,
                endpoint,
                address,
                permission,
                memory,
                operations,
            )?;
            if next == place.projections().len() {
                return Ok(Some((endpoint, address, memory)));
            }
            let ty = self.with_emission_budget_v1(|this, budget| {
                let plan = this
                    .execution
                    .as_ref()
                    .and_then(|cursor| cursor.references)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .plan;
                let (ty, schema) =
                    source_reference_place_pointer_v29(plan, site, place, next, access, budget)?
                        .ok_or(ArgumentResourceV1::Accounting)?;
                if schema != endpoint.projected_schema {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok(ty)
            })?;
            value = self.source_object_read_value_v29(endpoint, address, memory, ty, operations)?;
            index = next;
        }
    }

    fn try_read_source_object_leaf_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        if self
            .scoped_memory
            .as_ref()
            .is_some_and(|recorder| recorder.index_payload.is_some())
            && let Some(value) = self
                .try_read_source_object_index_v29(block, statement, place, volatility, operations)?
        {
            return Ok(Some(value));
        }
        let Some((source, address, mut access)) = self.source_object_leaf_address_v29(
            block,
            statement,
            place,
            SourceReferenceAccessV29::Read,
            operations,
        )?
        else {
            return Ok(None);
        };
        if self.types[place.ty().index() as usize]
            .layout()
            .size_bytes()
            == Some(0)
            && self.with_emission_budget_v1(|this, budget| {
                source_grid_leader_zero_type_v29(this.types, this.callables, place.ty(), budget)
            })?
        {
            self.record_grid_leader_zero_v29(
                block,
                statement,
                place,
                source,
                SourceReferenceAccessV29::Read,
                None,
                volatility,
                operations.len(),
            )?;
            // Return the existing logical binding, not a value reconstructed
            // from zero bytes or a fallback through the legacy slot lookup.
            return self
                .source_grid_leader_object_binding_v29(block, statement, place, source)
                .map(Some);
        }
        let ty = self.source_object_leaf_type_v29(
            block,
            statement,
            place,
            SourceReferenceAccessV29::Read,
            source.projected_schema,
        )?;
        access.volatile = volatility == SemanticVolatilityV1::Volatile;
        self.source_object_read_value_v29(source, address, access, ty, operations)
            .map(Some)
    }

    fn try_read_source_object_index_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(read) = self
            .scoped_memory
            .as_ref()
            .and_then(|recorder| recorder.index_payload)
        else {
            return Ok(None);
        };
        let selected = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<Option<SemanticValueBindingV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<
                Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess, Type)>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<
                Option<(usize, usize, SourceReferenceScalarCellV29)>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<&SemanticRetainedLocalSlotV1>>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<
                Option<(&ScopedAllocationIdentityV29, &SemanticRetainedLocalSlotV1)>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<
                std::collections::btree_map::Range<
                    '_,
                    ScopedAllocationIdentityV29,
                    SemanticRetainedLocalSlotV1,
                >,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<
                std::ops::RangeInclusive<ScopedAllocationIdentityV29>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<Type>(plan, budget)?;
            source_reference_owned_prepay_v29::<bool>(plan, budget)?;
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let objects = ScopedAllocationIdentityV29::OriginalObject {
                local: read.local.index(),
                generation: 0,
            }..=ScopedAllocationIdentityV29::OriginalObject {
                local: read.local.index(),
                generation: u32::MAX,
            };
            if this.retained_local_slots.range(objects).next().is_none() {
                return Ok(None);
            }
            budget.charge_work(12)?;
            if read.site != execution_site_v29(block, statement)
                || read.local != place.local()
                || read.ty != place.ty()
                || !place.projections().is_empty()
                || volatility != SemanticVolatilityV1::NonVolatile
                || this
                    .scoped_memory
                    .as_ref()
                    .and_then(|recorder| recorder.frame)
                    != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
            {
                return Err(scoped_object_error_v29());
            }
            let generation =
                source_retained_index_generation_v29(plan, cursor.instance, read, budget)?;
            let mapping =
                plan.physical_object_generation(cursor.instance, read.local, generation, budget)?;
            let identity = ScopedAllocationIdentityV29::OriginalObject {
                local: read.local.index(),
                generation: mapping.map_or(generation, |(_, _, row)| row.generation),
            };
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let slot = this
                .retained_local_slots
                .get(&identity)
                .ok_or_else(scoped_object_error_v29)?;
            let SemanticRetainedStorageV29::Object {
                cell,
                schema,
                bytes,
                alignment,
            } = slot.storage
            else {
                return Err(scoped_object_error_v29());
            };
            if slot.semantic_type != read.ty {
                return Err(scoped_object_error_v29());
            }
            let logical = if let Some((logical, representative, physical)) = mapping {
                if representative != cell
                    || physical.ty != read.ty
                    || physical.kind != SourceBackingKindV29::Object(schema)
                {
                    return Err(scoped_object_error_v29());
                }
                logical
            } else {
                cell
            };
            if !budget.source_object_storage_matches_v29(
                plan,
                logical,
                cursor.instance,
                read.local,
                generation,
                schema,
                Some((bytes, alignment)),
            )? {
                return Err(scoped_object_error_v29());
            }
            let ty = lower_scalar_type(this.types, read.ty)?;
            let path = ScopedObjectPathV29 { first: 0, count: 0 };
            Ok(Some((
                ScopedObjectEndpointV29 {
                    object: ScopedObjectIdentityV29::Local {
                        instance: cursor.instance,
                        local: read.local,
                        generation,
                    },
                    source: ScopedObjectSourceV29::ProjectionIndex(read),
                    root_type: read.ty,
                    projected_type: read.ty,
                    root_schema: schema,
                    projected_schema: schema,
                    source_path: path,
                    path,
                },
                slot.pointer,
                MemoryAccess::new(AddressSpace::Private, alignment),
                ty,
            )))
        })?;
        let Some((source, address, access, ty)) = selected else {
            return Ok(None);
        };
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::ProjectionIndex(read),
            },
            |this| {
                this.emit(
                    operations,
                    ty,
                    OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, access }),
                )
            },
        )
        .map(Some)
    }

    fn try_write_source_object_leaf_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        value: &SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some((destination, address, mut access)) = self.source_object_leaf_address_v29(
            block,
            statement,
            place,
            SourceReferenceAccessV29::Write,
            operations,
        )?
        else {
            return Ok(false);
        };
        if matches!(value, SemanticValueBindingV1::GridLeader { .. }) {
            self.record_grid_leader_zero_v29(
                block,
                statement,
                place,
                destination,
                SourceReferenceAccessV29::Write,
                Some(value),
                volatility,
                operations.len(),
            )?;
            return Ok(true);
        }
        if let SemanticValueBindingV1::Aggregate(fields) = value {
            self.write_source_object_aggregate_fields_v29(
                block,
                statement,
                place,
                destination,
                address,
                access,
                fields,
                volatility,
                operations,
            )?;
            return Ok(true);
        }
        let expected = self.source_object_leaf_type_v29(
            block,
            statement,
            place,
            SourceReferenceAccessV29::Write,
            destination.projected_schema,
        )?;
        let source = self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(4)?;
            let SemanticValueBindingV1::Value { id, ty } = value else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if !invocation_equal_types_v1(ty, &expected, budget)? {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let site = execution_site_v29(block, statement);
            let original = if let Some((held, source)) = this
                .scoped_memory
                .as_ref()
                .and_then(|recorder| recorder.store_payload)
            {
                if held != *id {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                source
            } else {
                source_reference_emission_prepay_v29::<ScopedMemoryStoreSourceV29>(budget)?;
                match scoped_source_statement_v29(this.function, site) {
                    Some(SemanticStatementKindV1::Assign(assignment))
                        if std::ptr::eq(assignment.destination(), place)
                            && assignment.value().result_type() == place.ty() =>
                    {
                        ScopedMemoryStoreSourceV29::Assignment {
                            site,
                            ty: place.ty(),
                        }
                    }
                    Some(SemanticStatementKindV1::Store(store))
                        if std::ptr::eq(store.destination(), place)
                            && store.value().ty() == place.ty() =>
                    {
                        let (SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source)) =
                            store.value()
                        else {
                            return Err(ArgumentResourceV1::Accounting.into());
                        };
                        let cursor = this
                            .execution
                            .as_ref()
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        let occurrence = scoped_payload_occurrence_v29(
                            cursor,
                            site,
                            ExecutionOperandV29::StoreValue,
                            source,
                            budget,
                        )?
                        .ok_or(ArgumentResourceV1::Accounting)?;
                        ScopedMemoryStoreSourceV29::Operand {
                            site,
                            role: ExecutionOperandV29::StoreValue,
                            ty: place.ty(),
                            source: ScopedMemoryOperandSourceV29::Place(occurrence),
                        }
                    }
                    _ => return Err(ArgumentResourceV1::Accounting.into()),
                }
            };
            Ok((*id, original))
        })?;
        access.volatile = volatility == SemanticVolatilityV1::Volatile;
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::WriteValue {
                destination,
                value: ScopedObjectValueOriginV29::Original(source.1),
            },
            |this| {
                this.push_operation(operations, || {
                    Operation::new(
                        Vec::new(),
                        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            address,
                            value: source.0,
                            access,
                        }),
                    )
                })
            },
        )?;
        Ok(true)
    }

    fn source_object_local_endpoint_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        if self.scoped_memory.is_none() || !place.projections().is_empty() {
            return Ok(None);
        }
        self.with_emission_budget_v1(|this, budget| {
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let local = place.local().index();
            let objects = ScopedAllocationIdentityV29::OriginalObject { local, generation: 0 }
                ..=ScopedAllocationIdentityV29::OriginalObject { local, generation: u32::MAX };
            if this.retained_local_slots.range(objects).next().is_none() { return Ok(None); }
            let cursor = this.execution.as_ref().ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 12)?;
            let site = execution_site_v29(block, statement);
            let frame = this.scoped_memory.as_ref().and_then(|recorder| recorder.frame)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if frame.site != site || !scoped_object_original_place_v29(this.function, site, role)
                .is_some_and(|original| std::ptr::eq(original, place))
            { return Err(ArgumentResourceV1::Accounting.into()); }
            let source = source_reference_access_at_v29(plan, SourceReferenceSiteV29 {
                instance: cursor.instance, block, statement: statement.map(|value| value as usize),
            }, place, access, budget)?;
            if source.instance != cursor.instance || source.local != place.local()
                || source.source_local != place.local() || source.ty != place.ty()
                || source.loan.is_some() || !source.projections.is_empty()
                || !source.traversed.is_empty() || source.shared_path
            { return Err(ArgumentResourceV1::Accounting.into()); }
            let mapping = plan.physical_object_generation(cursor.instance, place.local(), source.generation, budget)?;
            let identity = ScopedAllocationIdentityV29::OriginalObject {
                local, generation: mapping.map_or(source.generation, |(_, _, row)| row.generation),
            };
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let slot = this.retained_local_slots.get(&identity).ok_or(ArgumentResourceV1::Accounting)?;
            let SemanticRetainedStorageV29::Object { cell, schema, bytes, alignment } = slot.storage else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if slot.semantic_type != place.ty() { return Err(ArgumentResourceV1::Accounting.into()); }
            let logical_cell = if let Some((logical, representative, physical)) = mapping {
                if representative != cell || physical.ty != place.ty() || physical.kind != SourceBackingKindV29::Object(schema) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                logical
            } else { cell };
            let selected = budget.source_object_storage_v29(plan, logical_cell, cursor.instance, place.local(), source.generation, schema)?;
            if !matches!(selected, SemanticRetainedStorageV29::Object {
                cell: checked_cell, schema: checked_schema, bytes: checked_bytes, alignment: checked_alignment,
            } if checked_cell == logical_cell && checked_schema == schema && checked_bytes == bytes && checked_alignment == alignment)
            { return Err(ArgumentResourceV1::Accounting.into()); }
            let path = ScopedObjectPathV29 { first: 0, count: 0 };
            Ok(Some((ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Local { instance: cursor.instance, local: place.local(), generation: source.generation },
                source: ScopedObjectSourceV29::Place { site, role, local: place.local(), prefix: 0 },
                root_type: place.ty(), projected_type: place.ty(), root_schema: schema, projected_schema: schema,
                source_path: path, path,
            }, slot.pointer, MemoryAccess::new(AddressSpace::Private, alignment))))
        })
    }

    fn source_object_local_tag_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        writing: bool,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        if self.scoped_memory.is_none() {
            return Ok(None);
        }
        self.with_emission_budget_v1(|this, budget| {
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let local = place.local().index();
            let objects = ScopedAllocationIdentityV29::OriginalObject { local, generation: 0 }
                ..=ScopedAllocationIdentityV29::OriginalObject { local, generation: u32::MAX };
            if this.retained_local_slots.range(objects).next().is_none() { return Ok(None); }
            let cursor = this.execution.as_ref().ok_or_else(scoped_object_allocation_error_v29)?;
            let references = cursor.references.ok_or_else(scoped_object_allocation_error_v29)?;
            let plan = references.plan;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 12)?;
            let site = execution_site_v29(block, statement);
            let role = if writing { ExecutionOperandV29::StatementPlace } else { ExecutionOperandV29::RvaluePlace };
            if !scoped_object_original_place_v29(this.function, site, role)
                .is_some_and(|original| std::ptr::eq(original, place))
                || !place.projections().is_empty()
                || !matches!(this.types.get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape), Some(SemanticTypeShapeV1::Enum { .. }))
            {
                return Err(scoped_object_allocation_error_v29());
            }
            let access = source_reference_access_at_v29(plan, SourceReferenceSiteV29 {
                instance: cursor.instance, block, statement: statement.map(|value| value as usize),
            }, place, if writing { SourceReferenceAccessV29::Write } else { SourceReferenceAccessV29::ReadDiscriminant }, budget)?;
            if access.instance != cursor.instance || access.local != place.local()
                || access.source_local != place.local() || access.ty != place.ty()
                || access.loan.is_some() || !access.projections.is_empty()
                || !access.traversed.is_empty() || access.shared_path {
                return Err(scoped_object_allocation_error_v29());
            }
            let mapping = plan.physical_object_generation(cursor.instance, place.local(), access.generation, budget)?;
            let identity = ScopedAllocationIdentityV29::OriginalObject {
                local, generation: mapping.map_or(access.generation, |(_, _, row)| row.generation),
            };
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let slot = this.retained_local_slots.get(&identity).ok_or_else(scoped_object_allocation_error_v29)?;
            let SemanticRetainedStorageV29::Object { cell, schema, bytes, alignment } = slot.storage else {
                return Err(scoped_object_allocation_error_v29());
            };
            if slot.semantic_type != place.ty() { return Err(scoped_object_allocation_error_v29()); }
            let logical_cell = if let Some((logical, representative, physical)) = mapping {
                if representative != cell || physical.ty != place.ty() || physical.kind != SourceBackingKindV29::Object(schema) {
                    return Err(scoped_object_allocation_error_v29());
                }
                logical
            } else { cell };
            let selected = budget.source_object_storage_v29(plan, logical_cell, cursor.instance, place.local(), access.generation, schema)?;
            if !matches!(selected, SemanticRetainedStorageV29::Object {
                cell: original_cell, schema: original_schema, bytes: original_bytes, alignment: original_alignment,
            } if original_cell == logical_cell && original_schema == schema && original_bytes == bytes && original_alignment == alignment) {
                return Err(scoped_object_allocation_error_v29());
            }
            let path = ScopedObjectPathV29 { first: 0, count: 0 };
            Ok(Some((ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Local { instance: cursor.instance, local: place.local(), generation: access.generation },
                source: ScopedObjectSourceV29::Place { site, role, local: place.local(), prefix: 0 },
                root_type: place.ty(), projected_type: place.ty(), root_schema: schema, projected_schema: schema,
                source_path: path, path,
            // The tag may begin at a less-aligned original byte offset than
            // its allocation. The Storage decoder checks that exact tag range.
            }, slot.pointer, MemoryAccess::new(AddressSpace::Private, 1))))
        })
    }

    fn try_emit_source_object_set_tag_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        variant: u32,
        operations: &mut Vec<Operation>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some((destination, address, access)) =
            self.source_object_local_tag_v29(block, statement, place, true)?
        else {
            return Ok(false);
        };
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(3)?;
            let Some(SemanticTypeShapeV1::Enum { variants, .. }) = this
                .types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Err(scoped_object_allocation_error_v29());
            };
            if variants
                .get(variant as usize)
                .is_none_or(|row| row.is_uninhabited())
            {
                return Err(scoped_object_allocation_error_v29());
            }
            Ok(())
        })?;
        let site = execution_site_v29(block, statement);
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::SetDiscriminant {
                destination,
                origin: ScopedObjectTagOriginV29::Statement(site),
                variant,
            },
            |this| {
                this.push_operation(operations, || {
                    Operation::new(
                        Vec::new(),
                        OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                            address,
                            variant,
                            access,
                        }),
                    )
                })
            },
        )?;
        Ok(true)
    }

    fn try_emit_source_object_read_tag_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        result_type: SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((source, address, access)) =
            self.source_object_local_tag_v29(block, statement, place, false)?
        else {
            return Ok(None);
        };
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(2)?;
            if !matches!(this.types.get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Enum { discriminant, .. }) if *discriminant == result_type) {
                return Err(scoped_object_allocation_error_v29());
            }
            Ok(())
        })?;
        let target = lower_scalar_type(self.types, result_type)?;
        let target_scalar = target
            .as_scalar()
            .filter(|scalar| scalar.is_integer())
            .ok_or_else(scoped_object_allocation_error_v29)?;
        let casts = plan_integer_cast_v1(ScalarType::U128, target_scalar)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        let site = execution_site_v29(block, statement);
        let mut value = self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::ReadDiscriminant {
                source,
                origin: ScopedObjectTagOriginV29::Statement(site),
            },
            |this| {
                this.emit(
                    operations,
                    Type::Scalar(ScalarType::U128),
                    OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant {
                        address,
                        access,
                    }),
                )
            },
        )?;
        for (kind, scalar) in casts.into_iter().flatten() {
            let (input, _) = value
                .value()
                .map_err(|_| scoped_object_allocation_error_v29())?;
            let to = Type::Scalar(scalar);
            value = self.emit(
                operations,
                to.clone(),
                OperationKind::Cast {
                    kind,
                    value: input,
                    to,
                },
            )?;
        }
        Ok(Some(value))
    }

    fn check_source_object_allocation_v29(
        &mut self,
        identity: ScopedAllocationIdentityV29,
        slot: &SemanticRetainedLocalSlotV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } = identity else {
            return Err(scoped_object_allocation_error_v29());
        };
        let SemanticRetainedStorageV29::Object {
            cell,
            schema,
            bytes,
            alignment,
        } = slot.storage
        else {
            return Err(scoped_object_allocation_error_v29());
        };
        self.with_emission_budget_v1(|this, budget| {
            let backing = SourceFunctionBackingViewV29::new(
                &this.defined_function_signatures, this.execution.as_ref(), this.semantic_function, budget,
            )?.ok_or_else(scoped_object_allocation_error_v29)?;
            let local = SemanticLocalIdV1::from_index(local);
            let (ordinal, row) = backing.cell(local, generation, budget)?
                .ok_or_else(scoped_object_allocation_error_v29)?;
            if ordinal != cell || row.ty != slot.semantic_type || row.kind != SourceBackingKindV29::Object(schema) {
                return Err(scoped_object_allocation_error_v29());
            }
            let selected = budget.source_object_storage_v29(
                backing.layouts.references, ordinal, backing.instance, local, generation, schema,
            )?;
            if !matches!(selected, SemanticRetainedStorageV29::Object {
                cell: checked_cell, schema: checked_schema, bytes: checked_bytes, alignment: checked_alignment,
            } if checked_cell == cell && checked_schema == schema && checked_bytes == bytes && checked_alignment == alignment) {
                return Err(scoped_object_allocation_error_v29());
            }
            Ok(())
        })
    }
}
