thread_local! {
    static ENUM_CENSUS_OBSERVED_V55: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct EnumCensusObserverGuardV55(Option<ScopedSlotObserverV29>);
impl Drop for EnumCensusObserverGuardV55 {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.0);
    }
}

fn check_enum_census_mutations_v55(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let id = instances.root();
    let summary = receipt
        .instances
        .iter()
        .find(|row| row.instance == id)
        .unwrap();
    let slots = &receipt.slots[summary.slots.clone()];
    let lowered = emitted[id.index()].as_mut().unwrap();
    let function = lowered.function.clone();
    let spans = lowered.synthetic_operation_spans.clone();
    let archive = lowered.execution_observation.as_ref().unwrap();
    let spills = archive.enum_spills.clone();
    let subject = archive.subject;
    let plan = archive.plan;
    let merge_local = (instances.owner().source_semantic().functions()[0]
        .locals()
        .len()
        - 3) as u32;
    assert_eq!(spills.len(), 2, "one enum merge with two non-Unit leaves");
    assert_eq!(
        spills
            .iter()
            .map(|spill| (spill.local, spill.variant, spill.field, spill.component))
            .collect::<Vec<_>>(),
        vec![(merge_local, 1, 0, 0), (merge_local, 1, 2, 0)]
    );
    let span = *spans
        .iter()
        .find(|row| row.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
        .unwrap();
    let entry = span.kernel_ir_block;
    let first = span.first_operation_ordinal as usize;
    let block = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .position(|block| block.id == entry)
        .unwrap();
    let floor = budget.storage();
    for fault in 0..29 {
        let archive = lowered.execution_observation.as_mut().unwrap();
        match fault {
            0 => {}
            1 => {
                archive.enum_spills.pop();
            }
            2 => archive.enum_spills.push(spills[0].clone()),
            3 => archive.enum_spills[0].pointer = spills[1].pointer,
            4 => archive.enum_spills[0].emitted_operation += 1,
            5 => archive.enum_spills[0].emitted_block.0 += 1,
            6 => archive.enum_spills[0].local = u32::MAX,
            7 => archive.enum_spills[0].source_type = U32,
            8 => archive.enum_spills[0].variant = 0,
            9 => archive.enum_spills[0].field_type = UNIT,
            10 => archive.enum_spills[0].component = 1,
            11 => archive.enum_spills[0].alignment = 1,
            12 => archive.enum_spills[0].element = Type::Scalar(ScalarType::I32),
            13 => {
                lowered.synthetic_operation_spans.retain(|row| {
                    row.rule != SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage
                });
            }
            14 => lowered.synthetic_operation_spans.push(span),
            15 => {
                lowered
                    .synthetic_operation_spans
                    .iter_mut()
                    .find(|row| row.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
                    .unwrap()
                    .operation_count -= 1;
            }
            16 => {
                lowered
                    .synthetic_operation_spans
                    .iter_mut()
                    .find(|row| row.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
                    .unwrap()
                    .first_operation_ordinal += 1;
            }
            17 => {
                lowered.function.body.as_mut().unwrap().blocks[block]
                    .operations
                    .remove(first);
            }
            18 => {
                let operation =
                    function.body.as_ref().unwrap().blocks[block].operations[first].clone();
                lowered
                    .function
                    .body
                    .as_mut()
                    .unwrap()
                    .blocks
                    .last_mut()
                    .unwrap()
                    .operations
                    .push(operation);
            }
            19 => {
                lowered.function.body.as_mut().unwrap().blocks[block].operations[first].results
                    [0]
                .id = spills[1].pointer;
            }
            20 => {
                lowered.function.body.as_mut().unwrap().blocks[block].operations[first].kind =
                    OperationKind::Constant(Constant::U32(0));
            }
            21 => {
                let OperationKind::Alloca { alignment, .. } =
                    &mut lowered.function.body.as_mut().unwrap().blocks[block].operations[first]
                        .kind
                else {
                    unreachable!()
                };
                *alignment = 1;
            }
            22 => {
                lowered.function.body.as_mut().unwrap().blocks[block].operations[first].results
                    [0]
                .ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                );
            }
            23 => archive.subject.source.semantic[0] ^= 1,
            24 => archive.subject.function = SemanticFunctionIdV1::from_index(u32::MAX),
            25 => archive.enum_spills.swap(0, 1),
            26 => {
                archive.enum_spills[0].pointer = spills[1].pointer;
                lowered.function.body.as_mut().unwrap().blocks[block].operations[first].results
                    [0]
                .id = spills[1].pointer;
            }
            27 => {
                lowered
                    .synthetic_operation_spans
                    .iter_mut()
                    .find(|row| row.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
                    .unwrap()
                    .correspondence_owner = SemanticFunctionIdV1::from_index(u32::MAX);
            }
            28 => {
                archive.enum_spills.pop();
                lowered
                    .synthetic_operation_spans
                    .iter_mut()
                    .find(|row| row.rule == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
                    .unwrap()
                    .operation_count -= 1;
            }
            _ => unreachable!(),
        }
        let result = with_canonical_call_scratch_v1(budget, |budget| {
            check_scoped_allocation_census_v55(instances, id, lowered, entry, first, slots, budget)
        });
        assert_eq!(budget.storage(), floor);
        lowered.function = function.clone();
        lowered.synthetic_operation_spans = spans.clone();
        let archive = lowered.execution_observation.as_mut().unwrap();
        archive.enum_spills = spills.clone();
        archive.subject = subject;
        archive.plan = plan;
        if matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
        ) {
            return result;
        }
        assert_eq!(
            result.is_ok(),
            fault == 0,
            "enum census mutation {fault}: {result:?}"
        );
        if fault != 0 {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "execution archive differs from its original source definition"
                    || detail == "scoped source-slot allocation census is incomplete or mismatched")
            );
        }
    }
    let saved = lowered.execution_observation.take();
    let absent = with_canonical_call_scratch_v1(budget, |budget| {
        check_scoped_allocation_census_v55(instances, id, lowered, entry, first, slots, budget)
    });
    lowered.execution_observation = saved;
    assert_eq!(budget.storage(), floor);
    if matches!(
        absent,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
    ) {
        return absent;
    }
    assert!(
        matches!(absent, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        if detail == "execution archive differs from its original source definition")
    );
    ENUM_CENSUS_OBSERVED_V55.set(ENUM_CENSUS_OBSERVED_V55.get() + 1);
    Ok(())
}

#[test]
fn scoped_allocation_census_authenticates_enum_archive_span_and_actual_allocations() {
    ENUM_CENSUS_OBSERVED_V55.set(0);
    let _restore = EnumCensusObserverGuardV55(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(check_enum_census_mutations_v55)),
    );
    probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        inspect_enum_spills_v48,
    )
    .0
    .unwrap();
    assert!(ENUM_CENSUS_OBSERVED_V55.get() > 0);
}

#[test]
fn scoped_allocation_census_keeps_full_resource_boundaries_with_hostile_replays() {
    let _restore = EnumCensusObserverGuardV55(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(check_enum_census_mutations_v55)),
    );
    let measured = probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        inspect_enum_spills_v48,
    );
    measured.0.unwrap();
    let exact = probe(
        transported_enum_owner_v50,
        measured.1,
        measured.2,
        inspect_enum_spills_v48,
    );
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for work_short in [false, true] {
        let work = measured.1 - usize::from(work_short);
        let storage = measured.2 - usize::from(!work_short);
        let result = probe(
            transported_enum_owner_v50,
            work,
            storage,
            inspect_enum_spills_v48,
        );
        match entrance_resource(result.0.err().expect("one-short complete census account")) {
            ArgumentResourceV1::Work(error) if work_short => {
                assert_eq!(error.actual(), measured.1);
                assert_eq!(error.limit(), work);
            }
            ArgumentResourceV1::Storage(error) if !work_short => {
                assert_eq!(error.actual(), measured.2);
                assert_eq!(error.limit(), storage);
            }
            other => panic!("wrong enum census resource failure: {other:?}"),
        }
    }
}

#[test]
fn scoped_allocation_census_headers_cover_independent_borrowed_frame_and_pointer_sort() {
    type Frame<'a> = (
        &'a ExecutionArchiveV29,
        &'a [ExecutionEnumSpillV48],
        &'a [ScopedSourceSlotV29],
        &'a [SemanticKirSyntheticOperationSpanV1],
        Option<&'a SemanticKirSyntheticOperationSpanV1>,
        &'a BasicBlock,
        &'a Operation,
        &'a SemanticTypeDeclV1,
        (&'a Type, &'a Type),
        std::slice::Iter<'a, ExecutionEnumSpillV48>,
        std::slice::Iter<'a, ScopedSourceSlotV29>,
        std::slice::Iter<'a, SemanticKirSyntheticOperationSpanV1>,
        Vec<ValueId>,
        Option<(u32, u32, u32, usize)>,
        [usize; 12],
    );
    assert_eq!(
        source_reference_emission_headers_v29::<ScopedEnumSpillCensusFrameV55<'_>>().unwrap(),
        source_reference_emission_headers_v29::<Frame<'_>>().unwrap()
    );
}
