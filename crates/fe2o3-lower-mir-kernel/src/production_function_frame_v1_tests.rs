mod cursor_scope_tests {
    use super::*;
    include!("production_function_frame_scope_v1_tests.rs");
}

std::thread_local! {
    static FRAME_SNAPSHOT: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static FRAME_REACHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct DetachMode(bool);
impl DetachMode {
    fn install(value: bool) -> Self {
        Self(DETACH_EMISSION_BLOCKS_V1.replace(value))
    }
}
impl Drop for DetachMode {
    fn drop(&mut self) {
        DETACH_EMISSION_BLOCKS_V1.set(self.0);
    }
}

fn frame_source_snapshot(source: ExecutionCallSourceV29) -> String {
    format!("{:?}", (source.semantic, source.ssa, source.root))
}

fn capture_frame_outputs(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut snapshots = Vec::new();
    for (index, lowered) in emitted.iter().enumerate() {
        let Some(lowered) = lowered else { continue };
        let instance = instances.id_at(index).unwrap();
        let original = instances.instance(instance).unwrap();
        assert_eq!(lowered.source_call_instance, Some(instance));
        let mut rows = vec![
            format!("{:?}", lowered.function),
            format!(
                "{:?}",
                (
                    &lowered.blocks,
                    &lowered.statement_operation_spans,
                    &lowered.terminator_operation_spans,
                    &lowered.synthetic_operation_spans
                )
            ),
            format!(
                "{:?}",
                (
                    &lowered.parameter_bindings,
                    &lowered.parameter_component_bindings,
                    &lowered.ignored_parameter_bindings,
                    &lowered.generated_terminator_values
                )
            ),
            format!(
                "{:?}",
                (
                    &lowered.call_returns.sites.rows,
                    &lowered.call_returns.components.rows,
                    lowered.next_value,
                    lowered.emitted_operations
                )
            ),
            format!(
                "{:?}",
                (
                    &lowered.private_arrays.slots,
                    &lowered.private_arrays.effects,
                    lowered.private_arrays.active,
                    lowered.private_arrays.placement,
                    lowered.private_arrays.payload.occupied,
                    lowered.private_arrays.payload.capacity
                )
            ),
        ];
        if let Some(entry) = &lowered.invocation_entry {
            let subject = entry.subject.unwrap();
            assert!(subject.ledger == budget.work_ledger_identity_v1());
            assert_eq!(subject.instance, instance);
            assert_eq!(subject.function, original.function());
            rows.push(format!(
                "{:?}",
                (
                    frame_source_snapshot(subject.source),
                    entry.layout,
                    entry.span,
                    &entry.arguments,
                    &entry.components,
                    &entry.inputs
                )
            ));
        }
        if let Some(initialization) = &lowered.scoped_initialization {
            assert!(initialization.subject.ledger == budget.work_ledger_identity_v1());
            assert_eq!(initialization.subject.instance, instance);
            rows.push(format!(
                "{:?}",
                (&initialization.blocks, &initialization.initialized_locals)
            ));
        }
        if let Some(anchors) = &lowered.scoped_memory_anchors {
            assert!(anchors.subject.ledger == budget.work_ledger_identity_v1());
            assert_eq!(anchors.subject.instance, instance);
            rows.push(format!("{:?}", (anchors.placement, &anchors.rows)));
        }
        if let Some(observed) = &lowered.execution_observation {
            rows.push(format!(
                "{:?}",
                (
                    &observed.locals,
                    &observed.bindings,
                    &observed.retained_seeds
                )
            ));
        }
        if let Some(events) = &lowered.lifecycle_events {
            assert!(events.ledger == budget.work_ledger_identity_v1());
            assert_eq!(events.instance, instance);
            rows.push(format!(
                "{:?}",
                (
                    frame_source_snapshot(events.source),
                    events.function,
                    events.placement,
                    events.provider,
                    events.expected_rows,
                    &events.rows
                )
            ));
        }
        if let Some(asserts) = &lowered.instance_assert_origins {
            assert!(asserts.ledger == budget.work_ledger_identity_v1());
            assert_eq!(asserts.instance, instance);
            assert!(!asserts.failed);
            rows.push(format!(
                "{:?}",
                (
                    frame_source_snapshot(asserts.source),
                    asserts.function,
                    asserts.placement,
                    &asserts.arguments
                )
            ));
            for row in &asserts.records {
                rows.push(format!(
                    "{:?}",
                    (
                        row.site,
                        &row.emitted_function,
                        row.block,
                        row.first_operation,
                        row.operation_count,
                        row.expected,
                        row.semantic_success,
                        row.physical_success,
                        row.argument_start,
                        row.argument_count,
                        row.outcome
                    )
                ));
            }
        }
        snapshots.push(rows.join("\n"));
    }
    assert!(!snapshots.is_empty());
    FRAME_SNAPSHOT.with(|saved| *saved.borrow_mut() = snapshots);
    FRAME_REACHED.set(true);
    Ok(())
}

fn frame_output(owner: impl FnOnce() -> ProductionSemanticSsaOwnerV1, detach: bool) -> Vec<String> {
    let _mode = DetachMode::install(detach);
    FRAME_REACHED.set(false);
    FRAME_SNAPSHOT.with(|saved| saved.borrow_mut().clear());
    let result = run_suffix_owner(owner, capture_frame_outputs, LIMIT, LIMIT, |_, _, _| Ok(())).0;
    assert!(result.is_ok(), "detach={detach}: {result:?}");
    assert!(FRAME_REACHED.get());
    FRAME_SNAPSHOT.with(|saved| std::mem::take(&mut *saved.borrow_mut()))
}

#[test]
fn actual_root_frame_detachment_preserves_scalar_cells_arrays_and_one_time_initializers() {
    for case in [
        SuffixCase::Ordinary,
        SuffixCase::Cells,
        SuffixCase::Array,
        SuffixCase::Root,
        SuffixCase::Loop,
        SuffixCase::CyclicEntry,
        SuffixCase::Branch,
    ] {
        let uninterrupted = frame_output(|| suffix_owner(case), false);
        let detached = frame_output(|| suffix_owner(case), true);
        assert_eq!(uninterrupted, detached, "case {case:?}");
    }
}

#[test]
fn actual_root_frame_detachment_preserves_nominal_identity_and_assertion_events() {
    for case in [
        NominalLoopCase::Entry,
        NominalLoopCase::Header,
        NominalLoopCase::Borrowed,
        NominalLoopCase::Mixed,
        NominalLoopCase::Nested,
        NominalLoopCase::Repeated,
    ] {
        let uninterrupted = frame_output(|| nominal_loop_owner(case), false);
        let detached = frame_output(|| nominal_loop_owner(case), true);
        assert_eq!(uninterrupted, detached, "case {case:?}");
    }
    assert_eq!(
        frame_output(scoped_root_tests::assertion_owner, false),
        frame_output(scoped_root_tests::assertion_owner, true),
    );
}

fn frame_metadata_fixture(results: Vec<Type>) -> LoweredFunctionPlanV1 {
    LoweredFunctionPlanV1 {
        correspondence_owner: SemanticFunctionIdV1::from_index(0),
        semantic_function: SemanticFunctionIdV1::from_index(1),
        kernel_ir_function: FunctionId::new("frame_metadata_fixture"),
        role: SemanticKirFunctionRoleV1::InternalHelper,
        parameter_declarations: Vec::new(),
        parameter_types: Vec::new(),
        parameter_values: Vec::new(),
        call_arguments: Vec::new(),
        parameter_local_bindings: Vec::new(),
        parameter_component_bindings: Vec::new(),
        ignored_parameter_bindings: Vec::new(),
        result_types: results,
    }
}

#[test]
fn owned_plan_partition_moves_result_payload_without_clone_or_self_borrow() {
    for results in [
        vec![],
        vec![Type::Scalar(ScalarType::U32)],
        vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)],
    ] {
        let mut plan = frame_metadata_fixture(results);
        plan.parameter_types.push(Type::Scalar(ScalarType::U32));
        plan.parameter_values.push(ValueId(9));
        plan.parameter_declarations.push((0, 1, U32));
        let results = plan.result_types.as_ptr();
        let parameters = plan.parameter_values.as_ptr();
        let types = plan.parameter_types.as_ptr();
        let name = plan.kernel_ir_function.clone();
        let count = plan.result_types.len();
        let borrowed = FunctionEmissionPlanV1::Borrowed(&plan).split();
        assert!(std::ptr::eq(&*borrowed.assembly, &plan));
        assert_eq!(borrowed.result_types.as_ptr(), results);
        drop(borrowed);
        let owned = FunctionEmissionPlanV1::Owned(plan).split();
        assert!(owned.assembly.result_types.is_empty());
        assert_eq!(owned.result_types.as_ptr(), results);
        assert_eq!(owned.result_types.len(), count);
        assert_eq!(owned.assembly.parameter_values.as_ptr(), parameters);
        assert_eq!(owned.assembly.parameter_types.as_ptr(), types);
        assert_eq!(owned.assembly.parameter_declarations, vec![(0, 1, U32)]);
        assert_eq!(owned.assembly.kernel_ir_function, name);
        assert_eq!(owned.assembly.semantic_function.index(), 1);
        assert_eq!(owned.assembly.correspondence_owner.index(), 0);
        assert_eq!(
            owned.assembly.role,
            SemanticKirFunctionRoleV1::InternalHelper
        );
    }
}

#[test]
fn an_already_emitted_original_block_cannot_be_replayed_or_recovered_as_a_frame() {
    let reached = std::cell::Cell::new(false);
    let probe = emission_services_tests::with_completed_helper(|lowering, _, _| {
        let entry = lowering.function.entry();
        let plan = frame_metadata_fixture(Vec::new());
        let mut frame = FunctionFrameV1::new(
            lowering,
            &plan,
            FunctionFrameBodyV1 {
                order: vec![entry],
                target_blocks: vec![],
                blocks: vec![],
                statements: vec![],
                terminators: vec![],
                synthetic: vec![],
            },
        )?;
        assert!(frame.step(None).is_err());
        assert!(frame.control.failed);
        let work = frame
            .lowering
            .emission_work
            .as_deref()
            .unwrap()
            .emission_service_work_v1();
        assert!(matches!(
            frame.step(None),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "emission services differ from their original function frame",
                ..
            })
        ));
        assert_eq!(
            frame
                .lowering
                .emission_work
                .as_deref()
                .unwrap()
                .emission_service_work_v1(),
            work
        );
        assert!(frame.detach().is_err());
        reached.set(true);
        Ok(())
    });
    assert!(probe.result.is_ok(), "{:?}", probe.result);
    assert!(reached.get());
}

#[allow(dead_code)]
struct BackingViewFields {
    layouts: &'static (),
    instance: usize,
}

#[allow(dead_code)]
struct FrameControlFields {
    next_block: usize,
    failed: bool,
    credit: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    growth: Option<emission_services_tests::GrowthFields>,
    backing: Option<BackingViewFields>,
}

#[test]
fn frame_control_header_and_constructor_have_independent_inclusive_bounds() {
    assert_eq!(
        std::mem::size_of::<BackingViewFields>(),
        std::mem::size_of::<SourceFunctionBackingViewV29<'static>>()
    );
    let header = std::mem::size_of::<FrameControlFields>()
        + std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>()
        + (std::mem::size_of::<PlanEnvelopeFields>()
            - std::mem::size_of::<&LoweredFunctionPlanV1>());
    assert_eq!(
        std::mem::size_of::<FrameControlFields>(),
        std::mem::size_of::<FunctionFrameControlV1<'static>>()
    );
    assert_eq!(header, function_frame_headers_v1().unwrap());
    for case in 0..4 {
        let short = case % 2 == 1;
        let storage_test = case >= 2;
        let floor = std::cell::Cell::new(0);
        let before = std::cell::Cell::new(0);
        let reached = std::cell::Cell::new(false);
        let probe = emission_services_tests::with_completed_helper(|mut lowering, _, _| {
            let entry = lowering.function.entry();
            let budget = lowering.emission_work.as_deref_mut().unwrap();
            let limit = emission_services_tests::LIMIT;
            if storage_test {
                budget.reserve_storage(limit - header + usize::from(short) - budget.storage())?;
            } else {
                budget.charge_work(
                    limit - 4 + usize::from(short) - budget.emission_service_work_v1().unwrap(),
                )?;
            }
            before.set(budget.emission_service_work_v1().unwrap());
            floor.set(budget.storage());
            let plan = frame_metadata_fixture(Vec::new());
            let frame = FunctionFrameV1::new(
                lowering,
                &plan,
                FunctionFrameBodyV1 {
                    order: vec![entry],
                    target_blocks: vec![],
                    blocks: vec![],
                    statements: vec![],
                    terminators: vec![],
                    synthetic: vec![],
                },
            );
            assert_eq!(frame.is_ok(), !short);
            drop(frame);
            reached.set(true);
            Ok(())
        });
        assert!(probe.result.is_ok(), "case {case}: {:?}", probe.result);
        assert!(reached.get());
        assert_eq!(
            probe.work - before.get(),
            if !storage_test && short { 0 } else { 4 }
        );
        assert_eq!(probe.storage, floor.get() + if short { 0 } else { header });
    }
}

#[allow(dead_code)]
enum PlanEnvelopeFields {
    Borrowed(&'static LoweredFunctionPlanV1),
    Owned(LoweredFunctionPlanV1),
}
#[allow(dead_code)]
struct PlanPartsFields {
    assembly: EmissionReadOnlyV1<'static, LoweredFunctionPlanV1>,
    result_types: EmissionReadOnlyV1<'static, Vec<Type>>,
}
#[allow(dead_code)]
struct PlanStorageFields {
    source: Option<&'static SourceReferencePlanV29<'static, 'static>>,
    growth: Option<emission_services_tests::GrowthFields>,
    entry: usize,
    required: usize,
    credit: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

#[allow(dead_code)]
struct OutputContextFields {
    function: &'static SemanticFunctionDeclV1,
    semantic_ssa: &'static ProductionSemanticSsaFunctionPlanV1,
    retained_header: bool,
    source_call_instance: Option<ProductionCallInstanceIdV1>,
    backing: Option<SourceFunctionBackingViewV29<'static>>,
    initialization_subject: Option<ScopedInitializationSubjectV29>,
    instance_assert_origins: Option<InstanceAssertCaptureV1>,
    has_runtime_assert: bool,
    invocation_entry: Option<InvocationEntryRelationV1>,
    parameter_bindings: Vec<SemanticKirParameterBindingV1>,
}

#[allow(dead_code)]
struct OwnedFrameFields {
    lowering: SemanticFunctionLoweringV1<'static, 'static>,
    assembly: PlanEnvelopeFields,
    body: FunctionFrameBodyV1,
    control: FrameControlFields,
}

#[allow(dead_code)]
struct PreparedFrameFields {
    frame: OwnedFrameFields,
    context: OutputContextFields,
}

#[allow(dead_code)]
struct UnassembledFrameFields {
    assembly: PlanEnvelopeFields,
    context: OutputContextFields,
    result_types: EmissionReadOnlyV1<'static, Vec<Type>>,
    infallible_asserts: InfallibleAssertDecisionsV1<'static>,
    target_blocks: Vec<BasicBlock>,
    blocks: Vec<SemanticKirBlockCorrespondenceV1>,
    statement_operation_spans: Vec<SemanticKirStatementOperationSpanV1>,
    terminator_operation_spans: Vec<SemanticKirTerminatorOperationSpanV1>,
    synthetic_operation_spans: Vec<SemanticKirSyntheticOperationSpanV1>,
    retained_local_slots: BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>,
    initialized_at_entry: BTreeMap<u32, BTreeSet<u32>>,
    generated_terminator_values: Vec<SemanticKirGeneratedTerminatorValuesV1>,
    call_returns: CallReturnBufferV1,
    private_arrays: PrivateArrayFunctionRowsV1,
    scoped_memory_anchors: Option<ScopedMemoryAnchorsV29>,
    lifecycle_events: Option<PendingLifecycleEventsV29>,
    emitted_operations: usize,
    next_value: u32,
    execution_observation: Option<ExecutionArchiveV29>,
}

#[test]
fn shared_driver_plan_envelopes_have_independent_exact_and_one_short_storage() {
    use std::mem::size_of;
    assert_eq!(
        size_of::<PlanEnvelopeFields>(),
        size_of::<FunctionEmissionPlanV1<'static>>()
    );
    assert_eq!(
        size_of::<PlanPartsFields>(),
        size_of::<FunctionEmissionPlanPartsV1<'static>>()
    );
    assert_eq!(
        size_of::<PlanStorageFields>(),
        size_of::<FunctionPlanStorageV1<'static>>()
    );
    assert_eq!(
        size_of::<OutputContextFields>(),
        size_of::<FunctionFrameOutputContextV1<'static>>()
    );
    assert_eq!(
        size_of::<PreparedFrameFields>(),
        size_of::<PreparedFunctionFrameV1<'static, 'static>>()
    );
    assert_eq!(
        size_of::<UnassembledFrameFields>(),
        size_of::<UnassembledFunctionFrameV1<'static>>()
    );
    let header = size_of::<PlanEnvelopeFields>()
        + size_of::<PlanPartsFields>()
        + size_of::<PlanStorageFields>()
        + size_of::<
            Result<
                Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
                Box<dyn std::any::Any + Send>,
            >,
        >()
        + size_of::<PreparedFrameFields>()
        + size_of::<Result<PreparedFrameFields, ProductionSemanticKirErrorV1>>()
        + size_of::<UnassembledFrameFields>()
        + size_of::<Result<UnassembledFrameFields, ProductionSemanticKirErrorV1>>();
    assert_eq!(header, function_frame_plan_headers_v1().unwrap());
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let probe = emission_services_tests::with_completed_helper(|mut lowering, _, _| {
            let budget = lowering.emission_work.as_deref_mut().unwrap();
            let fill = emission_services_tests::LIMIT - header + usize::from(short);
            budget.reserve_storage(fill - budget.storage())?;
            let work = budget.emission_service_work_v1();
            let result = FunctionPlanStorageV1::new(lowering.execution.as_ref(), budget);
            assert_eq!(result.is_ok(), !short);
            match result {
                Ok(storage) => {
                    assert_eq!(budget.storage(), emission_services_tests::LIMIT);
                    storage.finish(budget)?;
                }
                Err(error) => assert!(matches!(
                    error,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )),
            }
            assert_eq!(budget.storage(), fill);
            assert_eq!(budget.emission_service_work_v1(), work);
            reached.set(true);
            Ok(())
        });
        assert!(probe.result.is_ok(), "{:?}", probe.result);
        assert!(reached.get());
    }
}
