#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DescriptorFault {
    None,
    SourceAudit,
    Gep,
    Data,
    Extent,
    Guard,
    Missing,
    Volatility,
    OrderedSourceAudit,
    ExternalHelperAlignment,
    ExternalHelperResultType,
    ExternalHelperVolatility,
}
thread_local! {
    static DESCRIPTOR_FAULT: std::cell::Cell<DescriptorFault> = const { std::cell::Cell::new(DescriptorFault::None) };
    static DESCRIPTOR_EMITTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DESCRIPTOR_TAMPERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DESCRIPTOR_ASSERTIONS_STARTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DESCRIPTOR_ASSERTIONS_COMPLETED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DESCRIPTOR_RUN_COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
struct DescriptorObservers {
    slot: Option<ScopedSlotObserverV29>,
    postflight: Option<SourceReferencePostflightObserverV29>,
}
impl DescriptorObservers {
    fn install(fault: DescriptorFault) -> Self {
        DESCRIPTOR_FAULT.set(fault);
        DESCRIPTOR_EMITTED.set(0);
        DESCRIPTOR_TAMPERED.set(0);
        DESCRIPTOR_ASSERTIONS_STARTED.set(0);
        DESCRIPTOR_ASSERTIONS_COMPLETED.set(0);
        DESCRIPTOR_RUN_COMPLETED.set(false);
        Self {
            slot: SCOPED_SLOT_OBSERVER_V29.replace(Some(descriptor_slot_observer)),
            postflight: SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29
                .replace(Some(descriptor_postflight_observer)),
        }
    }
}
impl Drop for DescriptorObservers {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.slot);
        SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.set(self.postflight);
    }
}
fn descriptor_postflight_observer(
    references: Option<&mut SourceReferenceEmissionV29<'_, '_>>,
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    success: bool,
) {
    let Some(references) = references.filter(|_| success) else {
        return;
    };
    if references.descriptors.is_empty() {
        return;
    }
    DESCRIPTOR_ASSERTIONS_STARTED.set(DESCRIPTOR_ASSERTIONS_STARTED.get() + 1);
    assert_eq!(
        references.plan.descriptors.len(),
        references.descriptors.len()
    );
    if DESCRIPTOR_FAULT.get() == DescriptorFault::SourceAudit {
        for row in &references.plan.descriptors {
            row.check(instances, budget).unwrap();
            let mut foreign = *row;
            foreign.source ^= 1;
            assert!(foreign.check(instances, budget).is_err());
            let mut stale = *row;
            stale.index_value = stale.holder_value;
            assert!(stale.check(instances, budget).is_err());
        }
    }
    if DESCRIPTOR_FAULT.get() == DescriptorFault::OrderedSourceAudit {
        audit_ordered_descriptor_sources_v29(references.plan, instances, budget);
    }
    for claim in &references.descriptors {
        assert!(matches!(
            claim.get().unwrap().producer,
            SourceReferenceSelectorProducerV29::Address { .. }
        ));
    }
    DESCRIPTOR_EMITTED.set(DESCRIPTOR_EMITTED.get() + references.descriptors.len());
    if DESCRIPTOR_FAULT.get() == DescriptorFault::Missing {
        references.descriptors[0].set(None);
        DESCRIPTOR_TAMPERED.set(DESCRIPTOR_TAMPERED.get() + 1);
    }
    DESCRIPTOR_ASSERTIONS_COMPLETED.set(DESCRIPTOR_ASSERTIONS_COMPLETED.get() + 1);
}
fn descriptor_slot_observer(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = DESCRIPTOR_FAULT.get();
    let count = emitted
        .iter()
        .flatten()
        .filter_map(|lowered| lowered.function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::GetElementPointer { .. }))
        .count();
    DESCRIPTOR_EMITTED.set(DESCRIPTOR_EMITTED.get() + count);
    if matches!(
        fault,
        DescriptorFault::None
            | DescriptorFault::SourceAudit
            | DescriptorFault::Missing
            | DescriptorFault::OrderedSourceAudit
    ) {
        return Ok(());
    }
    if matches!(
        fault,
        DescriptorFault::ExternalHelperAlignment
            | DescriptorFault::ExternalHelperResultType
            | DescriptorFault::ExternalHelperVolatility
    ) {
        let mut changed = 0;
        for operation in emitted
            .iter_mut()
            .flatten()
            .filter_map(|lowered| lowered.function.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|block| &mut block.operations)
        {
            let OperationKind::Load { access, .. } = &mut operation.kind else {
                continue;
            };
            if access.address_space != AddressSpace::Generic {
                continue;
            }
            assert_eq!(operation.results.len(), 1);
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
            assert_eq!(access.alignment, 4);
            assert!(!access.volatile);
            match fault {
                DescriptorFault::ExternalHelperAlignment => access.alignment = 8,
                DescriptorFault::ExternalHelperResultType => {
                    operation.results[0].ty = Type::Scalar(ScalarType::U64)
                }
                DescriptorFault::ExternalHelperVolatility => access.volatile = true,
                _ => unreachable!(),
            }
            changed += 1;
        }
        assert_eq!(changed, 1, "actual external-reference helper load reached");
        DESCRIPTOR_TAMPERED.set(DESCRIPTOR_TAMPERED.get() + changed);
        return Ok(());
    }
    for lowered in emitted.iter_mut().flatten() {
        let body = lowered.function.body.as_mut().unwrap();
        let descriptor_pointers: BTreeSet<ValueId> = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::GetElementPointer { .. }))
            .map(|operation| {
                assert_eq!(operation.results.len(), 1);
                operation.results[0].id
            })
            .collect();
        let other = body
            .parameters
            .iter()
            .zip(&lowered.function.signature.parameters)
            .filter_map(|(id, ty)| matches!(ty, Type::Slice(_)).then_some(*id))
            .nth(1)
            .unwrap();
        for block in &mut body.blocks {
            if fault == DescriptorFault::Guard {
                if let Some(Terminator::ConditionalBranch { then_target, .. }) =
                    block.terminator.as_ref()
                {
                    block.terminator = Some(Terminator::Branch {
                        target: *then_target,
                        arguments: vec![],
                    });
                    DESCRIPTOR_TAMPERED.set(DESCRIPTOR_TAMPERED.get() + 1);
                    return Ok(());
                }
            }
            for operation in &mut block.operations {
                match (&mut operation.kind, fault) {
                    (
                        OperationKind::Load { pointer, access }
                        | OperationKind::Store {
                            pointer, access, ..
                        },
                        DescriptorFault::Volatility,
                    ) if descriptor_pointers.contains(pointer) => {
                        access.volatile = !access.volatile;
                    }
                    (OperationKind::GetElementPointer { base, offset }, DescriptorFault::Gep) => {
                        *offset = *base
                    }
                    (OperationKind::SliceData { slice }, DescriptorFault::Data)
                    | (OperationKind::SliceLength { slice }, DescriptorFault::Extent) => {
                        *slice = other
                    }
                    _ => continue,
                }
                DESCRIPTOR_TAMPERED.set(DESCRIPTOR_TAMPERED.get() + 1);
                return Ok(());
            }
        }
    }
    panic!("actual descriptor producer not reached");
}

fn run_descriptor_module(
    case: DescriptorCase,
    fault: DescriptorFault,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ScopedModuleErrorV29>, usize, usize) {
    run_descriptor_owner_module(
        descriptor_source_owner(case),
        case,
        fault,
        work_limit,
        storage_limit,
    )
}

fn run_descriptor_owner_module(
    owner: ProductionSemanticSsaOwnerV1,
    case: DescriptorCase,
    fault: DescriptorFault,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ScopedModuleErrorV29>, usize, usize) {
    let _observers = DescriptorObservers::install(fault);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let (input, launch) = with_module_fixture_view(
            &owner,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| OwnedExecutionInputV29::capture(source, budget),
        )?;
        let mut donor = Some(ScopedSourceInputsV29 {
            owner,
            launch,
            input: input?,
        });
        let owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        assert!(donor.is_none());
        assert!(
            DESCRIPTOR_EMITTED.get() > 0,
            "actual root path must consume source descriptor receipts"
        );
        let floor = budget.storage();
        let identity = *owner.pending.graph.identity();
        let replay = owner.replay(&mut budget);
        assert_eq!(owner.pending.graph.identity(), &identity);
        assert_eq!(budget.storage(), floor);
        let retained = owner.retained_storage;
        drop(owner);
        budget.release_storage(retained)?;
        replay
    })();
    assert_eq!(
        budget.storage(),
        MODULE_FLOOR,
        "case {case:?}, fault {fault:?}, result {result:?}"
    );
    DESCRIPTOR_RUN_COMPLETED.set(true);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn actual_scalar_slice_reads_writes_and_both_metadata_forms_reach_v18_and_replay() {
    for write in [false, true] {
        for explicit in [false, true] {
            for metadata_length in [false, true] {
                let case = DescriptorCase {
                    write,
                    explicit,
                    metadata_length,
                    ..DescriptorCase::READ
                };
                run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
                    .0
                    .unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
            }
        }
    }
}

#[test]
fn original_repeated_loop_guard_tracks_current_index_ssa_and_descriptor() {
    let case = DescriptorCase {
        looped: true,
        ..DescriptorCase::READ
    };
    run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
        .0
        .unwrap();
}

include!("production_source_ordered_descriptor_v29_tests.rs");
