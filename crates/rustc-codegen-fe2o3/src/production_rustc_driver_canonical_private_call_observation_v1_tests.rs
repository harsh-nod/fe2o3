#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PrivateGraph {
    definitions: usize,
    allocations: usize,
    addresses: usize,
    loads: usize,
    stores: usize,
    calls: usize,
    typed_calls: usize,
    duplicate_operands: usize,
    cross_block_memory: usize,
}
fn private_graph(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) -> PrivateGraph {
    use fe2o3_kernel_ir::OperationKind as K;
    let mut row = PrivateGraph {
        definitions: 0,
        allocations: 0,
        addresses: 0,
        loads: 0,
        stores: 0,
        calls: 0,
        typed_calls: 0,
        duplicate_operands: 0,
        cross_block_memory: 0,
    };
    for function in &owner.module().functions {
        let Some(body) = &function.body else { continue };
        row.definitions += 1;
        // Diagnostic topology only. The production physical reader separately
        // authenticates exact cell offsets, reaching stores and source lifetime.
        let base = |pointer| {
            body.blocks
                .iter()
                .flat_map(|b| &b.operations)
                .find_map(|op| match &op.kind {
                    K::GetElementPointer { base, .. }
                        if op.results.iter().any(|r| r.id == pointer) =>
                    {
                        Some(*base)
                    }
                    _ => None,
                })
                .unwrap_or(pointer)
        };
        for (block_index, block) in body.blocks.iter().enumerate() {
            for operation in &block.operations {
                match &operation.kind {
                    K::Alloca { .. } => row.allocations += 1,
                    K::GetElementPointer { .. } => row.addresses += 1,
                    K::Load { pointer, .. } => {
                        row.loads += 1;
                        for (store_block, predecessor) in body.blocks.iter().enumerate() {
                            if block_index != store_block && predecessor.operations.iter().any(|o| {
                                matches!(&o.kind, K::Store { pointer: stored, .. } if base(*stored) == base(*pointer))
                            }) {
                                row.cross_block_memory += 1;
                            }
                        }
                    }
                    K::Store { .. } => row.stores += 1,
                    K::Call { callee, arguments }
                        if owner
                            .module()
                            .functions
                            .iter()
                            .any(|f| f.id == *callee && f.body.is_some()) =>
                    {
                        row.calls += 1;
                        if arguments.len() == 2 && operation.results.len() == 1 {
                            row.typed_calls += 1;
                            row.duplicate_operands += usize::from(arguments[0] == arguments[1]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    row
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrivateChecks {
    functions: usize,
    stages: usize,
    pending: usize,
    spans: usize,
    zero_spans: usize,
    physical: usize,
    retained: usize,
    removed: usize,
    source_aliases: usize,
    shared_aliases: usize,
    call_aliases: usize,
    private_frames: usize,
    typed_frames: usize,
    assertions: usize,
    memory: [usize; 2],
    definitions: usize,
    elisions: usize,
    source_work: usize,
    native_work: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrivateObservation {
    actual_target: Target,
    original: Subject,
    output: Subject,
    roots: Vec<Root>,
    symbols: Vec<String>,
    rounds: Vec<Round>,
    before: PrivateGraph,
    after: PrivateGraph,
    checks: PrivateChecks,
    source_floor: usize,
    additional: usize,
    owner_floor: usize,
    final_storage: usize,
    work: usize,
}
fn private_validate(request: &PrivateRequest, row: &PrivateObservation) -> Result<(), String> {
    let mut names = row
        .roots
        .iter()
        .map(|root| root.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let mut expected = request.case.roots().to_vec();
    expected.sort_unstable();
    require(names == expected, "exact complete source root membership")
        .map_err(|error| format!("{error}: expected {expected:?}, got {names:?}"))?;
    require(
        row.roots
            .windows(2)
            .all(|pair| pair[0].function < pair[1].function),
        "canonical source root identity order",
    )
    .map_err(|error| format!("{error}: {:?}", row.roots))?;
    require(
        row.actual_target == request.target
            && row
                .roots
                .iter()
                .all(|r| r.function != [0; 32] && r.body != [0; 32] && !r.entry.is_empty())
            && row.original.digest != [0; 32]
            && row.output.digest != [0; 32]
            && row.original.bytes > 0
            && row.output.bytes > 0
            && row.symbols.len() >= row.before.definitions
            && row
                .symbols
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == row.symbols.len(),
        "actual complete ordered roots and canonical subjects",
    )
    .map_err(|error| {
        format!(
            "{error}: target={:?}, roots={:?}, original={:?}, output={:?}, symbols={:?}, definitions={}",
            row.actual_target, row.roots, row.original, row.output, row.symbols, row.before.definitions
        )
    })?;
    let checks = row.checks;
    let expected_definitions = request.case.roots().len()
        + if request.case == PrivateCase::SharedPrivateOpt0 {
            2
        } else {
            1
        };
    require(
        row.before.definitions == expected_definitions
            && row.after.definitions == expected_definitions
            && checks.functions == expected_definitions
            && checks.stages == 9 * expected_definitions
            && checks.pending == 19
            && checks.spans > 0
            && checks.zero_spans > 0
            && checks.call_aliases >= row.before.calls
            && row.before.calls > 0
            && row.before.calls == row.after.calls
            && row.before.typed_calls > 0
            && row.after.typed_calls == row.before.typed_calls
            && row.before.duplicate_operands > 0
            && row.after.duplicate_operands > 0
            && Some(checks.physical) == checks.retained.checked_add(checks.removed)
            && checks.source_aliases >= checks.physical
            && checks.typed_frames > 0
            && checks.definitions > 0
            && checks.source_work > 0
            && checks.native_work > 0
            && row.source_floor > 0
            && row.additional > 0
            && row.source_floor.checked_add(row.additional) == Some(row.owner_floor)
            && row.final_storage == 0
            && row.work > 0,
        "real complete final reports, typed calls, source aliases and restored floor",
    )?;
    require(
        !row.rounds.is_empty()
            && row.rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1,
        "complete actual owned round extent",
    )?;
    let mut input = &row.original;
    for (ordinal, round) in row.rounds.iter().enumerate() {
        require(
            round.ordinal as usize == ordinal
                && &round.input == input
                && round.integer.bytes > 0
                && round.integer.digest != [0; 32]
                && round.output.bytes > 0
                && round.output.digest != [0; 32]
                && round.changed == (round.input != round.output)
                && round.changed == (ordinal + 1 != row.rounds.len()),
            "exact adjacent original/integer/scalar/terminal chain",
        )?;
        input = &round.output;
    }
    require(input == &row.output, "actual final F endpoint")?;
    if request.case.opt0() {
        require(
            row.original != row.output
                && row.rounds.len() >= 2
                && checks.elisions > 0
                && row.before.allocations > 0
                && row.before.addresses > 0
                && row.before.loads > 0
                && row.before.stores > 0
                && row.after.allocations == row.before.allocations
                && row.after.addresses == row.before.addresses
                && row.after.loads == row.before.loads
                && row.after.stores == row.before.stores
                && checks.memory[0] == row.after.allocations
                && Some(checks.memory[1]) == row.after.loads.checked_add(row.after.stores)
                && checks.assertions > 0,
            "nonvacuous checked rewrite retaining actual arrays and source bounds obligations",
        )?;
    }
    match request.case {
        PrivateCase::SharedPrivateOpt0 => require(
            checks.private_frames == 2 && checks.shared_aliases > 0 && row.before.calls == 5,
            "both root-qualified aliases of the one retained private helper",
        ),
        PrivateCase::CrossBlockPrivateOpt0 => require(
            checks.private_frames == 0
                && row.before.cross_block_memory > 0
                && row.after.cross_block_memory > 0
                && row.before.calls == 3,
            "actual cross-block private root with all three retained typed calls",
        ),
        PrivateCase::NormalTypedCalls => require(
            checks.private_frames == 0 && row.before.calls == 1,
            "normal-MIR genuine typed call, not an empty/noop substitute",
        ),
    }
}
fn private_observe(
    tcx: TyCtxt<'_>,
    request: &PrivateRequest,
) -> Result<PrivateObservation, String> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(cpu == request.target.cpu(), "actual compiler target")?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| e.to_string())?;
    transaction.consume_pre_ranked_for_test_v1(|source, descriptors| {
        require(descriptors.len() == request.case.roots().len(), "complete real descriptor roster")?;
        let source_roots = census::roots(source.semantic_ssa().source_semantic())?;
        require(source_roots.len() == source.executable().module().kernels.len(), "source/executable roots")?;
        require(source_roots.iter().zip(&source.executable().module().kernels)
            .all(|(root, kernel)| root.name == kernel.id.as_str()), "exact ordered source/kernel root association")?;
        let roots = source_roots.into_iter().zip(&source.executable().module().kernels)
            .map(|(root, kernel)| Root { name: root.name, function: root.function, body: root.body, entry: kernel.entry.as_str().into() }).collect();
        let original = subject(source.executable());
        let original_bytes = source.executable().canonical().canonical_bytes().to_vec();
        let symbols: Vec<String> = source.executable().module().functions.iter().map(|f| f.id.as_str().into()).collect();
        let before = private_graph(source.executable());
        let source_floor = source.unit_local_source_storage_floor_v1().map_err(|e| e.to_string())?;
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).map_err(|e| e.to_string())?;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, crate::production_canonical_phase_policy_v1::STORAGE_LIMIT);
        budget.reserve_storage(source_floor).map_err(|e| e.to_string())?;
        // A genuine producer or admission refusal is a failing gate, never an
        // alternate positive or permission to discard the original obligation.
        let (owner, receipt) = NeutralOwner::try_prepare_with_private_calls_v1(source, &mut budget).map_err(|e| e.to_string())?;
        require(budget.storage() == source_floor, "factory restores original source floor")?;
        let additional = receipt.retained_storage();
        budget.reserve_storage(additional).map_err(|e| e.to_string())?;
        let owner_floor = owner.retained_storage_floor_v1();
        require(budget.storage() == owner_floor, "actual retained owner floor")?;
        require(owner.original_source().executable().canonical().canonical_bytes() == original_bytes,
            "exact unchanged original canonical N bytes")?;
        require(owner.output().module().functions.iter().map(|f| f.id.as_str()).eq(symbols.iter().map(String::as_str)),
            "exact ordered retained N/F symbol roster")?;
        require(owner.output().module().kernels == owner.original_source().executable().module().kernels,
            "exact ordered root metadata retained")?;
        let output = subject(owner.output());
        let output_bytes = owner.output().canonical().canonical_bytes().to_vec();
        let after = private_graph(owner.output());
        // Diagnostic vectors are constructed outside the controlled callback;
        // only the prepaid Copy summary escapes the paid borrowed view.
        let mut input = original.clone();
        let mut rounds = Vec::new();
        for round in owner.history().rounds() {
            let current = subject(round.output());
            rounds.push(Round { ordinal: round.ordinal(), input: input.clone(),
                integer: subject(round.integer().owner()), changed: input != current, output: current.clone() });
            input = current;
        }
        let summary_storage = size_of::<PrivateChecks>();
        budget.reserve_storage(summary_storage).map_err(|e| e.to_string())?;
        let checks = owner.with_private_call_policy_checks_v1(&mut budget, |view, budget| {
            let metadata = view.original_metadata(budget)?;
            let original_inventory = metadata.inventory(budget)?;
            assert!(std::ptr::eq(original_inventory.owner(), owner.original_source().executable()));
            let spans = metadata.spans(budget)?;
            budget.charge_work(spans.len())?;
            let zero_spans = spans.iter().filter(|s| s.operations().is_empty()).count();
            let output = view.final_inventory(budget)?;
            assert!(std::ptr::eq(output.owner(), owner.output()));
            let lineage = view.lineage(budget)?;
            let definitions = lineage.original_definition_count(budget)?;
            let mut elisions = 0;
            for original in 0..definitions {
                elisions += usize::from(lineage.definition_descendant_count(original, budget)? == 0);
            }
            let aliases = view.source_aliases(budget)?;
            let physical = view.operation_count(budget)?;
            let (mut retained, mut removed, mut shared_aliases) = (0, 0, 0);
            for n in 0..physical {
                let operation = view.operation(n, budget)?;
                budget.charge_work(original_inventory.operations().len())?;
                let original = original_inventory.operations().iter().find(|o| o.coordinate == operation.original()).unwrap();
                assert!(matches!(
                    (operation.kind(), &original.operation.kind),
                    (PrivateKind::Allocation, fe2o3_kernel_ir::OperationKind::Alloca { .. })
                    | (PrivateKind::Address, fe2o3_kernel_ir::OperationKind::GetElementPointer { .. })
                    | (PrivateKind::Load, fe2o3_kernel_ir::OperationKind::Load { .. })
                    | (PrivateKind::Store, fe2o3_kernel_ir::OperationKind::Store { .. })
                    | (PrivateKind::Call, fe2o3_kernel_ir::OperationKind::Call { .. })
                ));
                if let Some(current) = operation.current() {
                    assert!(operation.removal().is_none());
                    budget.charge_work(output.operations().len())?;
                    let final_index = output.operations().iter().position(|o| o.coordinate == current).unwrap();
                    assert_eq!(lineage.operation_origin(final_index, budget)?,
                        fe2o3_lower_mir_kernel::ProductionCanonicalScalarOperationOriginV1::Original(operation.original()));
                    retained += 1;
                } else {
                    let event = operation.removal().expect("actual first checked unreachable-removal event");
                    assert!((event.round() as usize) < owner.history().rounds().len());
                    removed += 1;
                }
                budget.charge_work(aliases.len())?;
                let mut count: usize = 0;
                for alias in aliases.iter().filter(|a| a.operation() == n) {
                    assert!(alias.association() < metadata.function_count(budget)?);
                    assert!(alias.span() < spans.len());
                    count += 1;
                }
                assert!(count > 0);
                shared_aliases += count.saturating_sub(1);
            }
            budget.charge_work(aliases.len())?;
            assert!(aliases.iter().all(|a| a.operation() < physical));
            let calls = view.calls(budget)?;
            for call in calls {
                budget.charge_work(1)?;
                let operation = view.operation(call.operation(), budget)?;
                assert_eq!(operation.kind(), PrivateKind::Call);
                let original_call = &original_inventory.calls()[call.original_call()];
                assert_eq!(original_call.coordinate, operation.original());
                let caller = metadata.function(call.caller_association(), budget)?;
                let callee = metadata.function(call.callee_association(), budget)?;
                assert_eq!(caller.source().correspondence_owner(), call.root());
                assert_eq!(callee.source().correspondence_owner(), call.root());
                assert_eq!(original_call.target, Some(callee.canonical().coordinate));
                assert_eq!(original_call.coordinate.block.function, caller.canonical().coordinate);
            }
            let callables = view.original_callables(budget)?;
            assert_eq!(callables.len(), metadata.function_count(budget)?);
            let (mut private_frames, mut typed_frames) = (0, 0);
            for callable in callables {
                budget.charge_work(1)?;
                assert_eq!(metadata.function(callable.association(), budget)?.canonical().coordinate, callable.function());
                private_frames += usize::from(callable.kind() == CallKind::PrivateFrame);
                if callable.kind() == CallKind::PrivateFrame {
                    assert_eq!(callable.source_decision(), fe2o3_mir_model::SemanticCallableDecisionV1::Rejected);
                }
                typed_frames += usize::from(matches!(callable.kind(), CallKind::EmptyOnly | CallKind::DeterministicEmpty));
            }
            let assertions = view.assertion_count(budget)?;
            for n in 0..assertions {
                assert!(view.assertion(n, budget)?.span() < spans.len());
            }
            let memory = view.memory_census(budget)?;
            assert_eq!(memory, [after.allocations, after.loads + after.stores]);
            let policies = view.policies(budget)?;
            assert!(std::ptr::eq(policies.owner(budget)?, owner.output()));
            assert_eq!(policies.module_function_count(budget)?, owner.output().module().functions.len());
            let functions = policies.definition_count(budget)?;
            assert_eq!(functions, after.definitions);
            let mut stages = 0;
            let mut previous = None;
            for n in 0..functions {
                let coordinate = policies.definition_coordinate(n, budget)?;
                assert!(previous.is_none_or(|p| p < coordinate.0));
                previous = Some(coordinate.0);
                assert!(owner.output().module().functions[coordinate.0 as usize].body.is_some());
                assert_eq!(policies.history(n, budget)?.function(), coordinate.0 as usize);
                let report = policies.report(n, budget)?;
                assert_eq!(report.paired_stage_count(), 9);
                assert_eq!(report.reports().pass_order(), &fe2o3_pliron::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert!(report.reports().is_clean());
                stages += report.paired_stage_count();
            }
            let native_work = policies.observation(budget)?.work_upper_bound();
            let source_resources = view.source_resources(budget)?;
            assert!(source_resources.failed_storage.is_none());
            assert!(!view.ranked_verification_is_complete() && !view.grants_artifact_or_launch_authority());
            assert!(!policies.ranked_verification_is_complete() && !policies.grants_artifact_or_launch_authority());
            Ok::<_, ProductionCanonicalScalarSourceErrorV1>(PrivateChecks {
                functions, stages, pending: policies.pending_obligations().iter().count(),
                spans: spans.len(), zero_spans, physical, retained, removed,
                source_aliases: aliases.len(), shared_aliases, call_aliases: calls.len(),
                private_frames, typed_frames, assertions, memory, definitions, elisions,
                source_work: source_resources.work, native_work,
            })
        }).map_err(|e| e.to_string())?;
        require(budget.storage() == owner_floor + summary_storage, "fresh C callback restores retained floor")?;
        require(owner.original_source().executable().canonical().canonical_bytes() == original_bytes
            && owner.output().canonical().canonical_bytes() == output_bytes, "callback preserves both exact original and final subjects")?;
        drop(owner);
        budget.release_storage(owner_floor).map_err(|e| e.to_string())?;
        budget.release_storage(summary_storage).map_err(|e| e.to_string())?;
        Ok(PrivateObservation { actual_target: request.target, original, output, roots, symbols,
            rounds, before, after, checks, source_floor, additional, owner_floor,
            final_storage: budget.storage(), work: budget.work() })
    }).map_err(|e| e.to_string())?
}
