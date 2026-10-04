//! Actual retained-input prefix/index checkpoint. Independent payload oracle;
//! this is test routing only, never access/origin/root-recipe admission.
use super::*;
use crate::production_ranked_projection_v1::{
    bf16_nominal_source_preparation_v1::with_nominal_rich_source_preparation_v1,
    canonical_assertion_facts_v1::with_nominal_canonical_facts_observation_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Q<T> = std::result::Result<T, QueryError>;
const HEADERS: usize = 32 * 1024;
const REFUSAL: &str = "actual root prefix callback refusal";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Observe,
    Error,
    Panic,
    Occupied,
    MissingInputs,
    DuplicateInputs,
    ChangedBinding,
    EqualInputClone,
    ForeignPendingLedger,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    roots: usize,
    references: usize,
    reserved: usize,
    operations: usize,
    next: u32,
    locals: usize,
    assigned: usize,
    processed: usize,
    fifo: usize,
    assembly_frame: usize,
}
#[derive(Default)]
struct Oracle {
    cursors: Vec<usize>,
    indices: Vec<Option<ProjectedDisjointIndexV1>>,
    fifo: Vec<usize>,
    input_shadow: Vec<ProductionRankedRootInputV1>,
}
fn projection(error: Error) -> QueryError {
    match error {
        Error::CanonicalAssertions(crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error)) => QueryError::Resource(error),
        _ => QueryError::Unavailable("actual root prefix source checkpoint refused"),
    }
}
fn expect_operation(
    actual: &[ProductionRankedOperationV1],
    cursor: &mut usize,
    expected: ProductionRankedOperationV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(128)?;
    if actual.get(*cursor) != Some(&expected) {
        return Err(Error::Incomplete(
            "actual root prefix operation differs from source oracle",
        ));
    }
    *cursor = cursor
        .checked_add(1)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok(())
}
fn oracle_id(next: &mut u32) -> Result<ProductionRankedValueIdV1> {
    let result = ProductionRankedValueIdV1::new(*next);
    *next = next
        .checked_add(1)
        .ok_or(Error::Incomplete("source oracle value domain exhausted"))?;
    Ok(result)
}
fn inspect_payload(
    view: &ActualRootPrefixIndicesV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    oracle: &mut Oracle,
    input_count: usize,
) -> Result<Observation> {
    let locals = view.function.locals().len();
    context.with_resources(|resources| {
        resources.work(
            locals
                .checked_mul(8)
                .and_then(|n| n.checked_add(256))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        resources.reserve(&mut oracle.cursors, locals)?;
        oracle.cursors.resize(locals, 0);
        resources.reserve(&mut oracle.indices, locals)?;
        oracle.indices.resize(locals, None);
        Ok(())
    })?;
    // Exact complete graph row/order and retained store/load/borrow equality are
    // established by the already independent original-source rescan in this SAME
    // loan. Only then may the independent FIFO interpreter below read its edges.
    super::super::initial_graph_v1::check_source_rows_for_prefix_for_test_v1(
        view.graph,
        rich,
        checked,
        context,
        &mut oracle.cursors,
    )?;
    context.with_resources(|resources| {
        resources.work(256)?;
        assert!(std::ptr::eq(view.function, rich.function()));
        assert_eq!(source_launch_input_v1(&view.input.source_launch), view.source_root.source_launch());
        assert_eq!(view.input.kernel_binding, view.source_root.kernel_binding());
        let mut cursor = 0usize;
        let mut next = 0u32;
        let mut reserved = 0usize;
        expect_operation(
            &view.prefix.entry_operations, &mut cursor,
            ranked_execution_layout_v1(view.source_root.layout()), resources,
        )?;
        // Independently inspect ACTUAL borrowed registration outputs; do not call
        // the shared prefix emitter, output-rank/count helper or ledger metadata.
        match view.references {
            [] => assert!(view.prefix.reserved_reference_values.is_none()),
            [binding] => {
                assert!(!binding.observable_output_writes.is_empty());
                for write in &binding.observable_output_writes {
                    resources.work(32)?;
                    let crate::reference_effect_v1::ReferenceOutputCoordinateV1::LogicalPoint(axes)
                        = &write.coordinate else {
                        return Err(Error::Incomplete("source oracle non-point reference output"));
                    };
                    for _ in 0..3 {
                        resources.work(64)?;
                        let id = oracle_id(&mut next)?;
                        assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().get(reserved), Some(&id));
                        reserved += 1;
                        expect_operation(&view.prefix.entry_operations, &mut cursor,
                            ProductionRankedOperationV1::SemanticConstant { result: id, value: 0 }, resources)?;
                    }
                    for axis in 0..axes.len() {
                        resources.work(64)?;
                        let id = oracle_id(&mut next)?;
                        assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().get(reserved), Some(&id));
                        reserved += 1;
                        expect_operation(&view.prefix.entry_operations, &mut cursor,
                            ProductionRankedOperationV1::SemanticSymbol {
                                result: id, symbol: u32::try_from(axis)
                                    .map_err(|_| Error::Incomplete("source oracle axis overflow"))?,
                            }, resources)?;
                    }
                }
                assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().len(), reserved);
            }
            _ => return Err(Error::Incomplete("source oracle duplicate reference registrations")),
        }
        let callables = checked.emission().owner().semantic_ssa().source_semantic().callables();
        // Source-order seeds; no shared seed/assignment/propagation helper.
        for block in view.function.blocks() {
            resources.work(64)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { continue; };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }, ..
            }) = callables.get(call.callee().index() as usize) else { continue; };
            resources.work(128)?;
            let destination = call.destination().ok_or(Error::Incomplete("source oracle seed destination"))?;
            let place = destination.place();
            if !place.projections().is_empty() { return Err(Error::Incomplete("source oracle seed projection")); }
            let local = place.local().index() as usize;
            if rich.scalar_counts().get(local).copied() != Some(1)
                || rich.address_escaped().get(local).copied() != Some(false)
                || oracle.indices.get(local).is_none()
                || oracle.indices[local].is_some()
            { return Err(Error::Incomplete("source oracle seed custody")); }
            let value = oracle_id(&mut next)?;
            expect_operation(&view.prefix.entry_operations, &mut cursor,
                ProductionRankedOperationV1::InvocationIndex { result: value, dimension: 0, launch_extent: 0 },
                resources)?;
            oracle.indices[local] = Some(ProjectedDisjointIndexV1 {
                value: ProductionRankedValueV1::Local(value),
                mapping: SemanticDisjointIndexSpaceV1::Index1d, precondition: None, availability: None,
            });
            resources.push(&mut oracle.fifo, local)?;
        }
        let mut fifo_cursor = 0usize;
        let mut processed = 0usize;
        while let Some(source) = oracle.fifo.get(fifo_cursor).copied() {
            resources.work(128)?;
            fifo_cursor += 1;
            let input = oracle.indices[source].ok_or(Error::Incomplete("source oracle missing queued value"))?;
            assert_eq!(rich.scalar_counts()[source], 1);
            assert!(!rich.address_escaped()[source]);
            for edge in &view.graph.edges()[source] {
                resources.work(256)?;
                processed = processed.checked_add(1).ok_or_else(|| resource(Resource::Arithmetic))?;
                let auth_block = match edge.kind {
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload { construction_block, .. } => construction_block,
                    CapabilityEdgeKindV1::Alias => edge.use_block,
                    _ => return Err(Error::Incomplete("source oracle outside closed index profile")),
                };
                if let Some(availability) = input.availability {
                    assert!(capability_availability_allows(
                        rich.option_dominance(), rich.enum_payload_dominance(), availability,
                        SemanticBlockIdV1::from_index(auth_block as u32),
                    ));
                }
                let projected = match edge.kind {
                    CapabilityEdgeKindV1::Alias => input,
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } =>
                        ProjectedDisjointIndexV1 {
                            availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)), ..input
                        },
                    _ => unreachable!(),
                };
                let row = oracle.indices.get_mut(edge.destination)
                    .ok_or(Error::Incomplete("source oracle out-of-range destination"))?;
                match row {
                    None => { *row = Some(projected); resources.push(&mut oracle.fifo, edge.destination)?; }
                    Some(existing) if *existing == projected => {}
                    Some(_) => return Err(Error::Incomplete("source oracle conflicting index origins")),
                }
            }
        }
        assert_eq!(cursor, view.prefix.entry_operations.len(), "no unobserved earlier/later emissions");
        assert_eq!(next, view.prefix.next_value, "one prefix-plus-index allocator");
        assert_eq!(view.indices.indices.len(), locals);
        assert_eq!(view.indices.grids.len(), locals);
        assert_eq!(view.indices.predicates.len(), locals);
        let mut assigned = 0usize;
        for (local, expected) in oracle.indices.iter().enumerate() {
            resources.work(128)?;
            assert_eq!(view.indices.indices[local], *expected);
            assert!(view.indices.grids[local].is_none());
            assert!(view.indices.predicates[local].is_none());
            assigned += usize::from(expected.is_some());
        }
        resources.work(oracle.fifo.len().checked_mul(32).ok_or_else(|| resource(Resource::Arithmetic))?)?;
        assert_eq!(view.indices.index_fifo, oracle.fifo);
        assert_eq!(view.indices.index_cursor, fifo_cursor);
        assert_eq!(view.indices.processed_edges, processed);
        assert!(view.indices.grid_fifo.is_empty());
        assert_eq!(view.indices.grid_cursor, 0);
        Ok(Observation {
            roots: input_count, references: view.references.len(), reserved,
            operations: cursor, next, locals, assigned, processed, fifo: fifo_cursor,
            assembly_frame: 0,
        })
    })
}
#[allow(clippy::too_many_arguments)]
fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Q<Observation> {
    let _accepted_run = super::accepted_frames::begin(match mode {
        Mode::Observe => super::accepted_frames::Mode::Observe,
        Mode::Occupied => super::accepted_frames::Mode::Occupied,
        Mode::Error => super::accepted_frames::Mode::Error,
        Mode::Panic => super::accepted_frames::Mode::Panic,
        Mode::MissingInputs => super::accepted_frames::Mode::MissingInputs,
        Mode::DuplicateInputs => super::accepted_frames::Mode::DuplicateInputs,
        Mode::ChangedBinding => super::accepted_frames::Mode::ChangedBinding,
        Mode::EqualInputClone => super::accepted_frames::Mode::EqualInputClone,
        Mode::ForeignPendingLedger => super::accepted_frames::Mode::ForeignPendingLedger,
    });
    let inputs = actual_inputs.inputs();
    let bindings = actual_inputs.bindings();
    assert!(actual_inputs.belongs_to(owner));
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let slot = budget as *const Budget<'_> as usize;
        let identity = budget.work_ledger_identity_v1();
        let before = budget.storage();
        let before_work = budget.work();
        let before_peak = budget.peak_storage();
        budget.charge_work(4 * HEADERS)?;
        budget.reserve_storage(HEADERS)?;
        let mut owned = HEADERS;
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut oracle = Oracle::default();
        let mut entered = false;
        // Both physical variable owners exist before any nested reservation.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                budget, |checked, budget| {
                    with_nominal_rich_source_preparation_v1(
                        owner, inventory, source.root(), source.root(), source.call_block(),
                        source.source_call(), budget, |rich, budget| {
                            with_nominal_canonical_facts_observation_v1(
                                owner, inventory, source.root(), source.root(), source.call_block(),
                                source.source_call(), budget, |facts| {
                                    with_nominal_recipe_resources_v1(facts, rich, &mut owned, |context| {
                                        if matches!(mode, Mode::MissingInputs | Mode::DuplicateInputs
                                            | Mode::ChangedBinding | Mode::EqualInputClone)
                                        {
                                            context.with_resources(|resources| {
                                                if mode == Mode::MissingInputs {
                                                    actual_selected_inputs_v1(
                                                        owner, checked, rich.function(), &inputs[..0], bindings, resources,
                                                    )?;
                                                    panic!("missing actual input accepted");
                                                }
                                                // Paid, OUTER-owned shadows test content validation only.
                                                // They cannot construct/replace the private lexical view.
                                                let count = if mode == Mode::DuplicateInputs { 2 } else { inputs.len() };
                                                resources.reserve(&mut oracle.input_shadow, count)?;
                                                for ordinal in 0..count {
                                                    resources.work(128)?;
                                                    let original = &inputs[if mode == Mode::DuplicateInputs { 0 } else { ordinal }];
                                                    let bytes = original.logical_name.len();
                                                    resources.work(bytes.checked_add(32).ok_or_else(|| resource(Resource::Arithmetic))?)?;
                                                    // Install the empty String owner before variable allocation.
                                                    oracle.input_shadow.push(ProductionRankedRootInputV1 {
                                                        logical_name: String::new(),
                                                        kernel_binding: original.kernel_binding,
                                                        source_launch: original.source_launch.clone(),
                                                    });
                                                    resources.reserve_storage(bytes)?;
                                                    let shadow = oracle.input_shadow.last_mut().unwrap();
                                                    shadow.logical_name.try_reserve_exact(bytes).map_err(|_| resource(Resource::Allocation))?;
                                                    if shadow.logical_name.capacity() != bytes { return Err(resource(Resource::Allocation)); }
                                                    shadow.logical_name.push_str(&original.logical_name);
                                                }
                                                if mode == Mode::ChangedBinding { oracle.input_shadow[0].kernel_binding[0] ^= 1; }
                                                assert!(!std::ptr::eq(inputs.as_ptr(), oracle.input_shadow.as_ptr()));
                                                let selected = actual_selected_inputs_v1(
                                                    owner, checked, rich.function(), &oracle.input_shadow, bindings, resources,
                                                )?;
                                                assert!(mode == Mode::EqualInputClone);
                                                assert!(std::ptr::eq(selected.input, &oracle.input_shadow[0]));
                                                Ok(())
                                            })?;
                                            if mode != Mode::EqualInputClone { unreachable!(); }
                                        }
                                        let accepted_stage = super::accepted_frames::enter();
                                        let observed = context.with_actual_root_prefix_indices_v1(
                                            checked, rich, actual_inputs, &mut pending,
                                            |view, context| {
                                                entered = true;
                                                let observed = inspect_payload(
                                                    &view, rich, checked, context, &mut oracle, inputs.len(),
                                                )?;
                                                match mode {
                                                    Mode::Error => Err(Error::Incomplete(REFUSAL)),
                                                    Mode::Panic => panic!("{REFUSAL}"),
                                                    _ => Ok(observed),
                                                }
                                            },
                                        )?;
                                        drop(accepted_stage);
                                        if mode == Mode::Occupied {
                                            let refused = context.with_actual_root_prefix_indices_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("occupied assembly entered") },
                                            );
                                            assert!(matches!(refused, Err(Error::Incomplete(
                                                "actual root assembly cannot be replaced or retried"
                                            ))));
                                        }
                                        if mode == Mode::ForeignPendingLedger {
                                            // Inert metadata-corruption control only. The actual
                                            // source graph and prefix NEVER run on this test ledger.
                                            let mut foreign_work = Work::new(0);
                                            let foreign = Budget::new(&mut foreign_work, 0);
                                            let (slot, old_identity) = pending.ledger.unwrap();
                                            assert!(old_identity != foreign.work_ledger_identity_v1());
                                            pending.ledger = Some((slot, foreign.work_ledger_identity_v1()));
                                            let refused = context.with_actual_root_prefix_indices_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("foreign pending ledger entered") },
                                            );
                                            assert!(matches!(refused, Err(Error::CanonicalAssertions(
                                                crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                                            ))));
                                        }
                                        Ok(observed)
                                    }).map_err(projection)
                                },
                            )
                        },
                    )
                },
            )
        }));
        let mut result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) }
        };
        if matches!(mode, Mode::MissingInputs | Mode::DuplicateInputs | Mode::ChangedBinding) {
            assert!(!entered);
            assert!(pending.prefix.entry_operations.is_empty());
            assert!(pending.indices.indices.is_empty());
            assert!(pending.indices.index_fifo.is_empty());
        } else {
            assert!(entered);
            assert!(pending.completed);
            assert!(!pending.prefix.entry_operations.is_empty());
        }
        if let Ok(observed) = &mut result {
            observed.assembly_frame = pending.frame_credits;
        }
        // These are post-preparation callback-error/panic controls, not claims
        // of panic injection at every partial-emission program point.
        drop(pending);
        drop(oracle);
        let floor = before.checked_add(owned).ok_or(QueryError::Resource(Resource::Arithmetic))?;
        if slot != budget as *const Budget<'_> as usize
            || identity != budget.work_ledger_identity_v1() || budget.storage() < floor
            || budget.work() < before_work || budget.peak_storage() < before_peak
            || budget.failed_work().is_some() || budget.failed_storage().is_some()
        {
            drop(result);
            return Err(QueryError::Resource(Resource::Accounting));
        }
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), before);
        result
    })
}
pub(crate) fn observe_actual_root_prefix_indices_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    super::accepted_frames::start(super::accepted_frames::Scope::S3);
    let work_before = budget.work();
    let observed = run(
        owner,
        source,
        inventory,
        actual_inputs,
        budget,
        Mode::Observe,
    )?;
    let occupied = run(
        owner,
        source,
        inventory,
        actual_inputs,
        budget,
        Mode::Occupied,
    )?;
    assert_eq!(observed, occupied);
    assert!(matches!(
        run(owner, source, inventory, actual_inputs, budget, Mode::Error),
        Err(QueryError::Unavailable(_))
    ));
    assert!(matches!(
        run(owner, source, inventory, actual_inputs, budget, Mode::Panic),
        Err(QueryError::CallbackPanicked)
    ));
    assert!(matches!(
        run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            Mode::MissingInputs
        ),
        Err(QueryError::Unavailable(_))
    ));
    assert!(matches!(
        run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            Mode::DuplicateInputs
        ),
        Err(QueryError::Unavailable(_))
    ));
    assert!(matches!(
        run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            Mode::ChangedBinding
        ),
        Err(QueryError::Unavailable(_))
    ));
    assert_eq!(
        run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            Mode::EqualInputClone
        )?,
        observed
    );
    assert_eq!(
        run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            Mode::ForeignPendingLedger
        )?,
        observed
    );
    // Direct aggregate ORIGINAL-ledger delta across Observe plus eight controls.
    let work = budget
        .work()
        .checked_sub(work_before)
        .ok_or(QueryError::Resource(Resource::Accounting))?;
    // Closed bounded summary. Exact payload equality was checked above, never
    // inferred from these counts. No logical names, paths, hashes or values print.
    eprintln!(
        "fe2o3-root-prefix-indices-v1 roots={} references={} reserved={} operations={} next={} locals={} assigned={} processed={} fifo={} assembly_frame={} work={}",
        observed.roots,
        observed.references,
        observed.reserved,
        observed.operations,
        observed.next,
        observed.locals,
        observed.assigned,
        observed.processed,
        observed.fifo,
        observed.assembly_frame,
        work,
    );
    eprintln!(
        "fe2o3-root-prefix-indices-controls-v1 occupied=pass error=pass panic=pass missing_inputs=pass duplicate_inputs=pass changed_binding=pass equal_clone_data_only=pass foreign_pending_ledger=pass"
    );
    super::accepted_frames::flush(
        super::accepted_frames::Scope::S3,
        work,
        observed.assembly_frame,
    );
    Ok(())
}
#[test]
fn actual_prefix_genuine_headers_cover_outer_owners_and_fixed_control_frames() {
    assert!(
        HEADERS
            >= 4 * size_of::<PendingActualRootPrefixIndicesV1>()
                + 4 * size_of::<Oracle>()
                + 4 * size_of::<Observation>()
                + 4 * size_of::<Budget<'static>>()
                + 4 * size_of::<Work>()
    );
}

#[test]
fn actual_prefix_operation_oracle_rejects_missing_or_changed_rows_without_advancing() {
    let operation = ProductionRankedOperationV1::SemanticConstant {
        result: ProductionRankedValueIdV1::new(91),
        value: 0,
    };
    let changed = ProductionRankedOperationV1::SemanticConstant {
        result: ProductionRankedValueIdV1::new(92),
        value: 0,
    };
    let mut cursor = 0;
    let mut resources = PreparationResourcesV1::unmetered();
    assert!(expect_operation(&[], &mut cursor, operation.clone(), &mut resources).is_err());
    assert_eq!(cursor, 0);
    assert!(expect_operation(&[changed], &mut cursor, operation.clone(), &mut resources).is_err());
    assert_eq!(cursor, 0);
    expect_operation(&[operation.clone()], &mut cursor, operation, &mut resources).unwrap();
    assert_eq!(cursor, 1);
}
