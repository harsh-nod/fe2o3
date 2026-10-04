fn source_length_owner_v76(write_only: bool, count: usize) -> ProductionSemanticSsaOwnerV1 {
    assert!((1..=3).contains(&count));
    let base = source_allocation_receiver_v29_tests::owner();
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let SemanticTerminatorKindV1::Call(call) = original.blocks()[0].terminator().kind() else {
        panic!("fixture retains an authentic len callable");
    };
    let mut blocks = Vec::new();
    for index in 0..count {
        let call = SemanticDirectCallV1::new_callable(
            call.callee(),
            call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                call.destination().unwrap().place().clone(),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(index as u32 + 1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([60 + index as u8; 32]),
                original.source(),
                original.blocks()[0].statements().to_vec(),
                SemanticTerminatorV1::new(original.source(), SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
        );
    }
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([70; 32]),
            original.source(),
            original.blocks()[1].statements().to_vec(),
            original.blocks()[1].terminator().clone(),
        )
        .unwrap(),
    );
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut callables = source.callables().to_vec();
    if write_only {
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[1] else {
            panic!("fixture compiler intrinsic");
        };
        let SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice,
            element,
            raw_index,
            index_space,
        } = *operation
        else {
            panic!("fixture len identity");
        };
        *operation = SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
            disjoint_slice,
            element,
            raw_index,
            index_space,
        };
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        source.roots().to_vec(),
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

fn source_length_abi_v76(
    owner: &ProductionSemanticSsaOwnerV1,
    write_only: bool,
) -> kernel_argument_abi_v18::tests::FixtureKernelAbiV18 {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let mut abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(owner);
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let [argument] = abi.arguments_mut(0).as_mut_slice() else {
        panic!("one exact source carrier");
    };
    argument.kind = ProductionKernelArgumentAbiKindV18::Descriptor {
        source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
        argument: LogicalArgumentV1::disjoint_slice(
            0,
            ValidName::new("length".to_owned()).unwrap(),
            &source,
            &layout,
            if write_only {
                fe2o3_kernel_descriptor::AccessMode::WriteOnly
            } else {
                fe2o3_kernel_descriptor::AccessMode::ReadWrite
            },
            0,
        )
        .unwrap(),
    };
    abi
}

fn source_length_native_run_v76(
    write_only: bool,
    count: usize,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let owner = source_length_owner_v76(write_only, count);
    let abi = source_length_abi_v76(&owner, write_only);
    let reached = std::cell::Cell::new(false);
    let (result, work, peak) = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        work,
        storage,
        |original, optimized, budget| {
            let floor = budget.storage();
            let rows = scoped_raw_admission_v29::checked_source_lengths_v76(original, 0, budget)?;
            assert_eq!(rows.len(), count);
            for row in rows {
                assert_eq!(row.root_parameter, 0);
                assert_eq!(row.element, ScalarType::U32);
                assert_eq!(
                    row.access,
                    if write_only {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadWrite
                    }
                );
            }
            let output = optimized.output_inventory(budget)?;
            let lengths = output
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::SliceLength { .. }))
                .count();
            assert!(lengths > 0 && lengths <= count);
            let launches = mixed_native_launches_v26(original, budget)?;
            let result = with_mixed_source_completion_v26(
                original,
                optimized,
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
                &mut |native, budget| {
                    assert_eq!(native.source_census(budget)?[3], lengths);
                    assert!(native.runtime_occurrences(budget)?.is_empty());
                    let premises = native.runtime_premises(budget)?;
                    assert_eq!(premises.len(), 1);
                    assert_eq!(premises[0].access_counts(), [0, 0]);
                    assert!(!premises[0].requires_initialized_extent());
                    let globals = native.completed_globals_v30(original, optimized, budget)?;
                    for operation in output.operations() {
                        if matches!(operation.operation.kind, OperationKind::SliceLength { .. }) {
                            assert_eq!(
                                globals.exact_operation(operation.coordinate, budget)?,
                                Some(slice_view_v1::CompletedGlobalOperationV26::Length)
                            );
                        }
                    }
                    assert!(native.source_roles_are_complete());
                    assert!(!native.runtime_requirements_are_discharged());
                    assert!(!native.grants_artifact_or_launch_authority());
                    reached.set(true);
                    Ok(())
                },
            );
            if let Err(error) = result {
                let error = ProductionAggregateSourceErrorV30::InitialCompletion(error);
                return match aggregate_source_resource_v30(&error) {
                    Some(resource) => Err(resource.into()),
                    None => panic!("exact source len completion failed: {error:?}"),
                };
            }
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
    (result, work, peak, reached.get())
}

#[test]
fn source_length_calls_complete_unused_and_duplicate_nominal_metadata_without_access_authority() {
    for write_only in [false, true] {
        for count in [1, 2, 3] {
            let (result, _, _, reached) = source_length_native_run_v76(
                write_only,
                count,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
            );
            result.unwrap();
            assert!(reached);
        }
    }
}

#[test]
fn source_length_calls_preserve_exact_and_one_short_whole_resource_boundaries() {
    let (result, work, storage, reached) =
        source_length_native_run_v76(false, 2, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(reached);
    let exact = source_length_native_run_v76(false, 2, work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, true));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, actual_work, actual_storage, _) =
            source_length_native_run_v76(false, 2, work_limit, storage_limit);
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert_eq!(error.actual(), work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(error.actual(), storage);
            }
            other => panic!("exact source len resource refusal: {other:?}"),
        }
        assert!(actual_work <= work_limit && actual_storage <= storage_limit);
    }
}

#[test]
fn source_length_retained_rows_refuse_substitution_and_preserve_the_first_denial() {
    for fault in 0..10 {
        let owner = source_length_owner_v76(false, 1);
        let abi = source_length_abi_v76(&owner, false);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, _, budget| {
                let rows =
                    scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?
                        .unwrap();
                assert!(rows.issuers.is_empty() && rows.accesses.is_empty());
                assert_eq!(rows.lengths.len(), 1);
                let floor = budget.storage();
                let mut copy =
                    scoped_raw_admission_v29::issued_role_tests_v29::copied_issued_rows_v18(
                        rows, budget,
                    )?;
                let owned = budget.storage() - floor;
                match fault {
                    0 => copy.lengths[0].root_parameter = 1,
                    1 => copy.lengths[0].root_input = copy.lengths[0].length,
                    2 => copy.lengths[0].receiver = copy.lengths[0].length,
                    3 => copy.lengths[0].length = copy.lengths[0].receiver,
                    4 => copy.lengths[0].element = ScalarType::U64,
                    5 => copy.lengths[0].access = AccessMode::ReadOnly,
                    6 => copy.lengths[0].instance = ProductionCallInstanceIdV1(usize::MAX),
                    7 => {
                        copy.lengths[0].definition = Some(SsaValueV1::Definition(
                            fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX),
                        ))
                    }
                    8 => copy.lengths.push(copy.lengths[0]),
                    9 => copy.lengths.clear(),
                    _ => unreachable!(),
                }
                let error = scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                    original, 0, &copy, budget,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "issued immutable receipt differs from its original owner"
                    )
                ));
                drop(copy);
                budget.release_storage(owned)?;
                assert_eq!(budget.storage(), floor);
                let work = budget.work();
                assert!(original.query(budget).is_err());
                assert_eq!(budget.work(), work);
                reached.set(true);
                Ok(())
            },
        );
        assert!(
            result.is_err() && reached.get(),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn source_length_query_refuses_foreign_budget_without_debit_or_refund() {
    let owner = source_length_owner_v76(false, 1);
    let abi = source_length_abi_v76(&owner, false);
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, _, budget| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            foreign.reserve_storage(budget.storage())?;
            let before = (
                foreign.work(),
                foreign.storage(),
                budget.work(),
                budget.storage(),
            );
            assert!(
                scoped_raw_admission_v29::checked_source_lengths_v76(original, 0, &mut foreign)
                    .is_err()
            );
            assert_eq!(
                (
                    foreign.work(),
                    foreign.storage(),
                    budget.work(),
                    budget.storage()
                ),
                before
            );
            assert!(
                scoped_raw_admission_v29::checked_source_lengths_v76(original, 0, budget).is_err()
            );
            assert_eq!((budget.work(), budget.storage()), (before.2, before.3));
            reached.set(true);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(reached.get());
}

#[test]
fn source_length_call_and_replay_headers_match_independent_live_shapes() {
    type CallFrame<'a> = (
        [&'a (); 18],
        [usize; 8],
        ProductionCallInstanceIdV1,
        SemanticBlockIdV1,
        Option<SsaValueV1>,
        [SemanticTypeIdV1; 3],
        SourceReferenceAnchorV29,
        SourceIssuedRootTransportV29,
        SourceIssuedActualValueV29<'a>,
        PendingSourceLengthV76,
        Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)>,
        Result<
            (SourceIssuedRootTransportV29, PendingSourceLengthV76),
            ProductionSemanticKirErrorV1,
        >,
        Result<Option<&'a ValueDef>, ProductionSemanticKirErrorV1>,
        Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>,
        Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1>,
        Result<(SemanticTypeIdV1, bool), ProductionSemanticKirErrorV1>,
        Result<Type, ProductionSemanticKirErrorV1>,
        Option<Type>,
        Type,
    );
    assert_eq!(
        source_length_call_headers_v76().unwrap(),
        std::mem::size_of::<CallFrame<'_>>() + std::mem::align_of::<CallFrame<'_>>()
    );
    type ReplayFrame<'a> = (
        [&'a (); 12],
        [usize; 5],
        std::slice::Iter<'a, PendingSourceIssuedSiteV29>,
        &'a PendingSourceLengthV76,
        Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
        SourceOwnedResultV18<&'a [PendingSourceLengthV76]>,
        SourceOwnedResultV18<()>,
        Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)>,
        Result<Type, ProductionSemanticKirErrorV1>,
        [Type; 2],
    );
    assert_eq!(
        scoped_raw_admission_v29::source_length_replay_headers_v76().unwrap(),
        std::mem::size_of::<ReplayFrame<'_>>() + std::mem::align_of::<ReplayFrame<'_>>()
    );
}

#[test]
fn source_length_call_discriminator_does_not_issue_get_mut_access_authority() {
    let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Constant);
    let source = owner.source_semantic();
    let mut observed = 0;
    for function in source.functions() {
        for block in function.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if matches!(
                source.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                    ..
                })
            ) {
                assert!(source_length_call_v76(source, call).is_none());
                observed += 1;
            }
        }
    }
    assert!(observed > 0);
}
