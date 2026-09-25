//! Same live Rust/MIR/KIR owner and original resource ledger throughout observation.
use super::capture::{Events, NegativeRow, RunRow, Sink, Sites};
use super::oracle::{self, Control};
use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BlockId, ScalarType, Type, ValueId};
use fe2o3_kir_sim::*;
use fe2o3_lower_mir_kernel::{ProductionArgumentCoverageV1, ProductionSemanticKirErrorV1};
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1 as Ownership;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Arguments {
    pub parameters: [u32; 4],
    pub semantic_types: [u32; 4],
    pub local_ids: [u32; 4],
    pub ownership: [&'static str; 4],
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Census {
    // Diagnostic tags only, never an admission table.
    pub counts: [u32; 24],
    pub first_other: Option<[u32; 2]>,
    pub type_counts: [u32; 6],
    pub root_parameter_kinds: [&'static str; 4],
}
fn operation_tag(kind: &OperationKind) -> usize {
    match kind {
        OperationKind::Constant(_) => 0,
        OperationKind::Intrinsic(_) => 1,
        OperationKind::Unary { .. } => 2,
        OperationKind::Binary { .. } => 3,
        OperationKind::Compare { .. } => 4,
        OperationKind::Cast {
            kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
            ..
        } => 19,
        OperationKind::Cast { .. } => 5,
        OperationKind::Select { .. } => 6,
        OperationKind::SliceLength { .. } => 7,
        OperationKind::SliceData { .. } => 8,
        OperationKind::GetElementPointer { .. } => 9,
        OperationKind::Load { .. } => 10,
        OperationKind::GuardedLoad { .. } => 11,
        OperationKind::Store { .. } => 12,
        OperationKind::GuardedStore { .. } => 13,
        OperationKind::Alloca {
            element: Type::Scalar(_),
            address_space: AddressSpace::Private,
            ..
        } => 14,
        OperationKind::Alloca { .. } => 15,
        OperationKind::MemoryIntrinsic(_) => 16,
        OperationKind::Call { .. } => 17,
        OperationKind::Matrix(_) => 18,
        OperationKind::Wave(wave)
            if *wave
                == fe2o3_kernel_ir::WaveOperation::full(
                    fe2o3_kernel_ir::WaveOperationKind::LaneId,
                    fe2o3_kernel_ir::WaveWidth::Wave64,
                ) =>
        {
            20
        }
        _ => 23,
    }
}
fn type_tag(ty: &Type) -> usize {
    match ty {
        Type::Unit => 0,
        Type::Scalar(_) => 1,
        Type::Pointer(_) => 2,
        Type::Slice(_) => 3,
        Type::Vector(_) => 4,
        Type::Execution(_) => 5,
    }
}
fn parameter_kind(ty: &Type) -> &'static str {
    match ty {
        Type::Slice(slice)
            if slice.address_space == AddressSpace::Global
                && *slice.element == Type::Scalar(ScalarType::U16)
                && slice.access == AccessMode::ReadOnly =>
        {
            "global-ro-u16-slice"
        }
        Type::Slice(slice)
            if slice.address_space == AddressSpace::Global
                && *slice.element == Type::F32
                && slice.access == AccessMode::ReadWrite =>
        {
            "global-rw-f32-slice"
        }
        Type::Scalar(ScalarType::U32) => "u32",
        Type::Scalar(_) => "other-scalar",
        Type::Pointer(_) => "pointer",
        Type::Slice(_) => "other-slice",
        Type::Unit => "unit",
        Type::Vector(_) => "vector",
        Type::Execution(_) => "execution",
    }
}
fn census(
    view: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Census, Error> {
    budget.reserve_storage(2 * std::mem::size_of::<Census>())?;
    budget.charge_work(8192)?;
    let module = view.emission().original().executable().module();
    let mut row = Census {
        counts: [0; 24],
        first_other: None,
        type_counts: [0; 6],
        root_parameter_kinds: ["absent"; 4],
    };
    if let Some(root) = module.functions.first() {
        for (slot, ty) in row
            .root_parameter_kinds
            .iter_mut()
            .zip(&root.signature.parameters)
        {
            *slot = parameter_kind(ty);
        }
    }
    for function in &module.functions {
        for ty in &function.signature.parameters {
            row.type_counts[type_tag(ty)] += 1;
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                for def in &block.parameters {
                    row.type_counts[type_tag(&def.ty)] += 1;
                }
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let tag = operation_tag(&operation.kind);
                    row.counts[tag] += 1;
                    if matches!(tag, 15 | 23) && row.first_other.is_none() {
                        row.first_other = Some([block.id.0, ordinal as u32]);
                    }
                    for def in &operation.results {
                        row.type_counts[type_tag(&def.ty)] += 1;
                    }
                }
            }
        }
    }
    Ok(row)
}
fn checked_arguments(
    view: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Arguments, Error> {
    let owner = view.emission().original();
    let root = view.emission().semantic_function();
    budget.reserve_storage(2 * std::mem::size_of::<Arguments>() + 1024)?;
    let expected = [
        Type::slice(
            Type::Scalar(ScalarType::U16),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::Scalar(ScalarType::U16),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
        Type::Scalar(ScalarType::U32),
    ];
    owner
        .with_checked_arguments_v1(root, root, budget, |args| {
            let fail = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
            let mut row = Arguments {
                parameters: [0; 4],
                semantic_types: [0; 4],
                local_ids: [0; 4],
                ownership: [
                    "shared-borrow",
                    "shared-borrow",
                    "exclusive-owner",
                    "by-value",
                ],
            };
            let sources = args.source_arguments()?;
            if sources.len() != 4 {
                return Err(fail());
            }
            for (slot, source) in sources.enumerate() {
                let wanted = [
                    Ownership::SharedBorrow,
                    Ownership::SharedBorrow,
                    Ownership::ExclusiveOwner,
                    Ownership::ByValue,
                ][slot];
                if source.ordinal() as usize != slot || source.source_ownership() != wanted {
                    return Err(fail());
                }
                row.semantic_types[slot] = source.ty().index();
            }
            for (slot, wanted) in expected.iter().enumerate() {
                let physical = args.physical(slot)?.ok_or_else(fail)?;
                if physical.slot() != slot || physical.ty() != wanted {
                    return Err(fail());
                }
                row.parameters[slot] = physical.value().0;
            }
            if args.physical(4)?.is_some() {
                return Err(fail());
            }
            let mut seen = [false; 4];
            args.visit_nodes(|node| {
                if !node.source_path().is_empty() {
                    return Ok(());
                }
                let slot = node.source_argument() as usize;
                if slot >= 4
                    || seen[slot]
                    || node.semantic_type().index() != row.semantic_types[slot]
                {
                    return Err(fail());
                }
                let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
                    return Err(fail());
                };
                if parameter.slot() != slot || parameter.value().0 != row.parameters[slot] {
                    return Err(fail());
                }
                let (local, path) = node.local_binding().ok_or_else(fail)?;
                if !path.is_empty() {
                    return Err(fail());
                }
                row.local_ids[slot] = local.index();
                seen[slot] = true;
                Ok(())
            })?;
            if seen != [true; 4] {
                return Err(fail());
            }
            Ok(row)
        })
        .map_err(|_| Error::Unavailable("actual checked four-argument source relation refused"))
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct CpuRow {
    pub source: Snapshot,
    pub arguments: Arguments,
    pub census: Census,
    pub runs: [Option<RunRow>; 18],
    pub negatives: [Option<NegativeRow>; 16],
    pub attempted_runs: usize,
    pub same_original_ledger: bool,
    pub source_authority_in_copied_row: bool,
    pub numerical_profile: &'static str,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Progress {
    pub attempt: usize,
    pub pattern: usize,
    pub length: usize,
    pub control: Control,
    pub observed: &'static str,
    pub preflight: Option<capture::PreflightSummary>,
    pub debug_failure: Option<&'static str>,
    pub matrix_lane_mask: u64,
    pub global_writes: u64,
    pub floor_restored: bool,
}
fn sites(
    snapshot: &Snapshot,
    arguments: &Arguments,
    view: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
) -> Result<Sites, Error> {
    let rows = &snapshot.outside_use_rows;
    if snapshot.outside_uses != 1 || snapshot.outside_uses_per_component != [1, 0, 0, 0] {
        return Err(Error::Unavailable(
            "actual source sink is not sole component-zero use",
        ));
    }
    let [block, ordinal, operand, component] =
        rows[0][0].ok_or(Error::Unavailable("source sink absent"))?;
    if operand != 1 || component != 0 {
        return Err(Error::Unavailable("source sink operand relation"));
    }
    let body = view.emission().original().executable().module().functions[0]
        .body
        .as_ref()
        .unwrap();
    let op = body
        .blocks
        .iter()
        .find(|b| b.id == BlockId(block))
        .and_then(|b| b.operations.get(ordinal as usize))
        .ok_or(Error::Unavailable("actual source sink site absent"))?;
    if !matches!(op.kind, OperationKind::Store { value, .. } if value == ValueId(snapshot.matrix_results[0]))
    {
        return Err(Error::Unavailable(
            "actual component-zero sink is not unchanged Store",
        ));
    }
    let mut selected = None;
    for actual_block in &body.blocks {
        for (ordinal, actual) in actual_block.operations.iter().enumerate() {
            if std::ptr::eq(actual, view.emission().operation()) {
                if selected.is_some() {
                    return Err(Error::Unavailable("duplicated actual MFMA site"));
                }
                selected = Some((actual_block.id, ordinal as u32));
            }
        }
    }
    let matrix = selected.ok_or(Error::Unavailable("actual MFMA operation not in owner"))?;
    if matrix.0.0 != snapshot.matrix_span[2]
        || matrix.1 < snapshot.matrix_span[3]
        || matrix.1 >= snapshot.matrix_span[3] + snapshot.matrix_span[4]
    {
        return Err(Error::Unavailable("actual MFMA span join"));
    }
    Ok(Sites {
        matrix,
        store: (BlockId(block), ordinal),
        results: snapshot.matrix_results.map(ValueId),
        parameters: arguments.parameters.map(ValueId),
    })
}
fn execute(
    view: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    budget: &mut Budget<'_>,
    last: &Cell<Option<Progress>>,
    captured: &Cell<Option<Census>>,
    case: &str,
) -> Result<CpuRow, Error> {
    let fixed = 2 * std::mem::size_of::<CpuRow>() + 4096;
    if fixed > 65536 {
        return Err(Error::Unavailable("fixed CPU row observation storage"));
    }
    budget.reserve_storage(fixed)?;
    budget.charge_work(65536)?;
    let source = super::snapshot(view, budget)?;
    let graph = census(view, budget)?;
    captured.set(Some(graph));
    let arguments = checked_arguments(view, budget)?;
    let sites = sites(&source, &arguments, view)?;
    let owner = view.emission().original().executable();
    let limits = V12CpuObservationOptionsV1::default().simulation_limits();
    let (admitted, receipt) =
        AdmittedSimulationModuleV1::admit_v12_with_verification_budget(owner, limits, budget)
            .map_err(|_| Error::Unavailable("actual V12 numerical view admission refused"))?;
    budget.reserve_storage(receipt.retained_storage())?;
    let ledger = budget.work_ledger_identity_v1();
    let mut result = CpuRow {
        source,
        arguments,
        census: graph,
        runs: [None; 18],
        negatives: [None; 16],
        attempted_runs: 0,
        same_original_ledger: true,
        source_authority_in_copied_row: false,
        numerical_profile: "gfx942-bf16-f32-m16n16k16-wave64-exact-integer-v1",
    };
    // Every request, oracle scratch, sink, and temporary output check is prepaid
    // BEFORE construction. Returned fixed rows are already in the retained charge.
    for attempt in 0..34 {
        let (pattern, length, control) = if attempt < 18 {
            (attempt / 3, oracle::LENGTHS[attempt % 3], Control::Positive)
        } else {
            (2, 64, oracle::NEGATIVES[attempt - 18])
        };
        let floor = budget.storage();
        let before = budget.work();
        let copied = budget.with_prepaid_scope(floor, 1, 8_388_608, 65536, |budget| {
            let request = oracle::request(&owner.module().kernels[0].id, pattern, length, control);
            let mut sink = Sink::new(sites, pattern, length, control);
            let mut events = Events {
                fail: matches!(control, Control::EventFailure),
            };
            let mut options = V12CpuObservationOptionsV1::default();
            if matches!(control, Control::StepLimit) {
                options = options.with_step_limit(1).unwrap();
            }
            if matches!(control, Control::RecordLimit) {
                options = options.with_record_limit(1).unwrap();
            }
            let mut execution_steps = 0;
            let mut preflight = None;
            let mut observed_output = oracle::Output([0; 272]);
            let outcome = admitted.with_v12_cpu_observation_v1(
                V12CpuObservationInputV1::new(owner, &request),
                options,
                budget,
                (&mut events, &mut sink),
                |run, original| {
                    preflight = capture::preflight_summary(run);
                    assert!(original.work_ledger_identity_v1() == ledger);
                    if matches!(control, Control::Positive) {
                        if let Ok(run) = run {
                            assert_eq!(run.identity().wire_version(), 12);
                            assert_eq!(
                                run.identity().digest(),
                                owner.canonical().identity().digest()
                            );
                            assert_eq!(
                                run.identity().canonical_length(),
                                owner.canonical().canonical_bytes().len() as u64
                            );
                            oracle::check_output(run, pattern, length);
                            if case == "direct-error" {
                                return Err("source CPU observer error control");
                            }
                            if case == "direct-panic" {
                                panic!("source CPU observer panic control");
                            }
                        }
                    }
                    let output = match run {
                        Ok(run) => oracle::Output(
                            run.shared_buffer(oracle::INPUT_IDS[2])
                                .expect("actual output allocation")
                                .bytes()
                                .try_into()
                                .expect("272-byte backing"),
                        ),
                        Err(_) => oracle::Output([0; 272]),
                    };
                    Ok::<_, &'static str>((
                        capture::classify(run),
                        run.ok().map_or(0, SimulationExecutionV1::steps_executed),
                        output,
                    ))
                },
            );
            let observed = match outcome {
                Ok((code, steps, output)) => {
                    execution_steps = steps;
                    observed_output = output;
                    code
                }
                Err(V12CpuObservationErrorV1::IncompleteObservation) => "incomplete-observation",
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Launch)) => {
                    "profile-launch"
                }
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Limits)) => {
                    "profile-limits"
                }
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::OwnerMismatch,
                )) => "profile-owner-mismatch",
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::FunctionRoster,
                )) => "profile-function-roster",
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::CanonicalBytes,
                )) => "profile-canonical-bytes",
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Counts)) => {
                    "profile-counts"
                }
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Type)) => {
                    "profile-type"
                }
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Operation,
                )) => "profile-operation",
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Matrix)) => {
                    "profile-matrix"
                }
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Request,
                )) => "profile-request",
                Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Accounting,
                )) => "profile-accounting",
                Err(V12CpuObservationErrorV1::Resource(_)) => "resource-refusal",
                Err(V12CpuObservationErrorV1::Observer(_)) => "observer-error",
            };
            last.set(Some(Progress {
                attempt,
                pattern,
                length,
                control,
                observed,
                preflight,
                debug_failure: sink.failure,
                matrix_lane_mask: sink.matrix_mask,
                global_writes: sink.global_writes,
                floor_restored: false,
            }));
            if case == "direct-error" && observed == "observer-error" {
                return Err(Error::Unavailable("source CPU observer error control"));
            }
            if observed != capture::expected_refusal(control) || sink.failure.is_some() {
                return Err(Error::Unavailable(
                    "actual whole-graph CPU outcome differs; see retained progress/census",
                ));
            }
            if attempt < 18 {
                sink.complete();
            } else if sink.matrix_mask != 0 || sink.global_writes != 0 {
                // These are delivered observations. A stopped/limited debug
                // sink is not an execution-cancellation or rollback guarantee.
                return Err(Error::Unavailable(
                    "refused observation exposed matrix completion or global writes",
                ));
            }
            Ok((
                sink.values,
                sink.allocations,
                sink.matrix_mask,
                sink.store_mask,
                sink.records,
                sink.global_writes,
                execution_steps,
                observed,
                observed_output,
            ))
        })?;
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let mut progress = last.get().unwrap();
        progress.floor_restored = true;
        last.set(Some(progress));
        if attempt < 18 {
            result.runs[attempt] = Some(RunRow {
                pattern,
                output_length: length,
                values_row_major_le_hex: copied.0,
                actual_allocations: copied.1.unwrap(),
                output_with_canaries_le_hex: copied.8,
                matrix_lane_mask: copied.2,
                committed_store_lane_mask: copied.3,
                records: copied.4,
                steps: copied.6,
                storage_floor: floor,
                storage_after: budget.storage(),
                work_before: before,
                work_after: budget.work(),
            });
        } else {
            result.negatives[attempt - 18] = Some(NegativeRow {
                control,
                observed: copied.7,
                matrix_lane_mask: copied.2,
                global_writes: copied.5,
                floor_restored: true,
            });
        }
        result.attempted_runs += 1;
    }
    drop(admitted);
    budget.release_storage(receipt.retained_storage())?;
    Ok(result)
}
pub(super) fn observe<'tcx>(
    transaction: crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    case: &str,
) -> Value {
    let last = Cell::new(None);
    let graph = Cell::new(None);
    let accounting = Cell::new(None);
    let (result, phase) = transaction.observe_bf16_mfma_source_for_test_v1(|view, budget| {
        let floor = budget.storage();
        let work = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        // The return row is paid separately from the inner scratch scopes.
        let reserve = 2 * std::mem::size_of::<CpuRow>() + 4096;
        budget.reserve_storage(reserve)?;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget.with_prepaid_scope(budget.storage(), 1, 65536, 65536, |budget| {
                execute(view, budget, &last, &graph, case)
            })
        }));
        accounting.set(Some([
            floor,
            budget.storage(),
            work,
            budget.work(),
            reserve,
        ]));
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor + reserve);
        match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    });
    // Original source budget has ended. These inert copied rows now belong to
    // the independently fixed 256KiB test-output domain, not a reset source ledger.
    let diagnostic = result.as_ref().err().map(ToString::to_string);
    let row = match result {
        Ok(row) => json!({"stage":"actual_source_graph_cpu", "cpu":row, "phase":phase,
            "progress":last.get(), "accounting":accounting.get(), "census":graph.get()}),
        Err(_) => json!({"stage":"actual_source_cpu_refused", "diagnostic":diagnostic,
            "phase":phase, "progress":last.get(), "accounting":accounting.get(), "census":graph.get()}),
    };
    assert!(serde_json::to_vec(&row).unwrap().len() <= 256 * 1024);
    row
}
#[test]
fn fixed_rows_and_request_envelopes_are_finite() {
    assert!(2 * std::mem::size_of::<CpuRow>() + 4096 <= 65536);
    assert!(
        2 * std::mem::size_of::<Progress>()
            + 2 * std::mem::size_of::<Census>()
            + 2 * std::mem::size_of::<Arguments>()
            <= 4096
    );
    assert!(std::mem::size_of::<Sink>() * 2 + 8 * 1024 < 65536);
    assert_eq!(
        oracle::PATTERNS * oracle::LENGTHS.len() + oracle::NEGATIVES.len(),
        34
    );
}
