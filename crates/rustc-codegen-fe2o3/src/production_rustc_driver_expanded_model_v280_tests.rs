//! Genuine private V259 -> V280 progression, separate from V259 input positives.
use super::*;
use fe2o3_verifier::ExpandedSupportCensusV280;
use sha2::{Digest, Sha256};

#[path = "production_rustc_driver_expanded_aggregate_diagnostic_v280_tests.rs"]
mod aggregate_diagnostic;

#[path = "production_rustc_driver_expanded_model_export_v282_tests.rs"]
mod model_export;

#[path = "production_rustc_driver_product_frames_v283_tests.rs"]
mod product_frames;

const MODEL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_child";
const WIDE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_wide_runtime_child";
const ACCOUNT_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_account_child";
const ACCOUNT_CASE: &str = "FE2O3_TEST_EXPANDED_MODEL_ACCOUNT_V280";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum AccountCase {
    Payload,
    Overreport,
    Underreport,
    Refuse,
    Panic,
    Refund,
    Foreign,
}

const ACCOUNT_CASES: [AccountCase; 7] = [
    AccountCase::Payload,
    AccountCase::Overreport,
    AccountCase::Underreport,
    AccountCase::Refuse,
    AccountCase::Panic,
    AccountCase::Refund,
    AccountCase::Foreign,
];

enum CallbackPayload {
    Inline,
    Heap(Box<[u8; 17]>),
}

fn is_accounting_refusal(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        error.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>(),
        Some(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting)
    ) || error.source().is_some_and(is_accounting_refusal)
}

fn is_selected_refusal(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        error.downcast_ref::<SourceError>(),
        Some(SourceError::Unsupported(
            "selected expanded model consumer error"
        ))
    ) || error.source().is_some_and(is_selected_refusal)
}

struct AccountCallbacks {
    case: AccountCase,
    result: Option<Result<(), String>>,
}

impl Callbacks for AccountCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget) as usize;
            // Production retains root-phase charges until the owning transaction
            // ends. The callback's dynamic payload is settled inside that scope.
            let run = |budget: &mut Budget<'_>| {
                let mut called = 0;
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    transaction.with_original_source_expanded_model_v280(
                        budget,
                        |source, original, tile, _, _, pair, model, budget| {
                            called += 1;
                            pair.check(source, original, tile, budget)?;
                            assert!(
                                !model
                                    .generated_source(budget)
                                    .map_err(SourceError::ExpandedModel)?
                                    .is_empty()
                            );
                            match self.case {
                                AccountCase::Payload => {
                                    budget.reserve_storage(17)?;
                                    Ok((CallbackPayload::Heap(Box::new([9; 17])), 17))
                                }
                                AccountCase::Overreport => Ok((CallbackPayload::Inline, 1)),
                                AccountCase::Underreport => {
                                    budget.reserve_storage(1)?;
                                    Ok((CallbackPayload::Inline, 0))
                                }
                                AccountCase::Refuse => Err(SourceError::Unsupported(
                                    "selected expanded model consumer error",
                                )),
                                AccountCase::Panic => std::panic::panic_any(280usize),
                                AccountCase::Refund => {
                                    budget.release_storage(1)?;
                                    Ok((CallbackPayload::Inline, 0))
                                }
                                AccountCase::Foreign => {
                                    let mut work = Work::new(500_000_000);
                                    let mut foreign = Budget::new(&mut work, 20_000_000);
                                    foreign.reserve_storage(budget.storage())?;
                                    let before = budget.work();
                                    let error = model
                                        .census(&mut foreign)
                                        .err()
                                        .expect("foreign model account must refuse");
                                    assert_eq!(budget.work(), before);
                                    Err(SourceError::ExpandedModel(error))
                                }
                            }
                        },
                    )
                }));
                assert_eq!(
                    called, 1,
                    "the actual generated model must precede every callback case"
                );
                assert_eq!(slot, std::ptr::from_ref(budget) as usize);
                assert!(ledger == budget.work_ledger_identity_v1());
                match self.case {
                    AccountCase::Panic => {
                        let payload = outcome
                            .err()
                            .expect("original model callback panic must propagate");
                        assert_eq!(*payload.downcast::<usize>().unwrap(), 280);
                    }
                    AccountCase::Payload => {
                        let result = outcome.unwrap()?;
                        let CallbackPayload::Heap(payload) = result.into_observation() else {
                            panic!("owned payload")
                        };
                        assert_eq!(*payload, [9; 17]);
                        let retained = budget.storage();
                        let root_phase = retained.checked_sub(17).expect("paid payload backing");
                        drop(payload);
                        budget.release_storage(17).unwrap();
                        assert_eq!(budget.storage(), root_phase);
                    }
                    AccountCase::Refuse => {
                        let error = outcome.unwrap().err().expect("selected consumer refusal");
                        assert!(
                            is_selected_refusal(&error),
                            "original error was replaced: {error:?}"
                        );
                    }
                    _ => {
                        let error = outcome.unwrap().err().expect("accounting refusal");
                        assert!(is_accounting_refusal(&error), "wrong refusal: {error:?}");
                    }
                }
                Ok::<_, SourceError>(())
            };
            let headers = [
                std::mem::size_of_val(&run)
                    .checked_mul(2)
                    .ok_or_else(|| "model account frame arithmetic".to_owned())?,
                std::mem::align_of_val(&run),
                2 * std::mem::size_of::<Result<(), SourceError>>(),
                8 * std::mem::size_of::<usize>(),
            ]
            .into_iter()
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or_else(|| "model account frame arithmetic".to_owned())?;
            budget
                .with_prepaid_scope(floor, 1, 1, headers, run)
                .map_err(|error| format!("model account transaction: {error:?}"))?;
            assert_eq!(slot, std::ptr::from_ref(&budget) as usize);
            assert!(ledger == budget.work_ledger_identity_v1());
            assert_eq!(
                budget.storage(),
                floor,
                "complete owned transaction must return the original floor"
            );
            Ok(())
        })());
        Compilation::Stop
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ModelObservation {
    graphs: [[u8; 32]; 3],
    runtime_and_instances: [u8; 32],
    references: [u8; 32],
    model: [u8; 32],
    census: [u8; 32],
    counts: [usize; 6],
    helper_instances: [usize; 2],
    model_consumer_called: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct WideRootObservation {
    source_root: u32,
    rank: u8,
    exact_workgroup: [u32; 3],
    max_grid: [u32; 3],
    source_layout: [u64; 3],
    physical_extents: [u64; 3],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct WideRuntimeObservation {
    index_bits: u8,
    little_endian: bool,
    roots: [WideRootObservation; 2],
}

#[derive(Debug, Serialize, Deserialize)]
struct WideModelObservation {
    observation: ModelObservation,
    runtime: WideRuntimeObservation,
    diagnostic_export: bool,
}

struct CollectedObservation {
    observation: ModelObservation,
    wide_runtime: Option<WideRuntimeObservation>,
}

fn number(hash: &mut Sha256, value: usize, budget: &mut Budget<'_>) -> Result<(), SourceError> {
    budget.charge_work(8)?;
    hash.update(
        u64::try_from(value)
            .map_err(|_| fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?
            .to_le_bytes(),
    );
    Ok(())
}

fn census_identity(
    rows: &ExpandedSupportCensusV280<'_>,
    budget: &mut Budget<'_>,
) -> Result<[u8; 32], SourceError> {
    let mut hash = Sha256::new();
    for length in [
        rows.roots.len(),
        rows.instances.len(),
        rows.calls.len(),
        rows.cuts.len(),
        rows.candidates.len(),
        rows.zero_edges.len(),
        rows.frame_demands_v281.len(),
        rows.forwarding_v288.len(),
    ] {
        number(&mut hash, length, budget)?;
    }
    for root in rows.roots {
        for value in [
            root.source_function as usize,
            root.original_function,
            root.target_function.0 as usize,
            root.instances.start,
            root.instances.end,
            root.cuts.start,
            root.cuts.end,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.instances {
        for value in [
            row.root,
            row.instance,
            row.source_function as usize,
            row.active as usize,
            row.incoming.is_some() as usize,
            row.incoming.map_or(0, |x| x.0),
            row.incoming.map_or(0, |x| x.1 as usize),
            row.locals.start,
            row.locals.end,
            row.cuts.start,
            row.cuts.end,
            row.calls.start,
            row.calls.end,
            row.parent_call_v281.is_some() as usize,
            row.parent_call_v281.unwrap_or(0),
            row.depth_v281,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.calls {
        for value in [
            row.root,
            row.caller,
            row.block as usize,
            row.callable as usize,
            row.kind as usize,
            row.reachable as usize,
            row.child.is_some() as usize,
            row.child.unwrap_or(0),
            row.continuation_v281.is_some() as usize,
            row.continuation_v281.unwrap_or(0),
            row.carries_v281.start,
            row.carries_v281.end,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.cuts {
        for value in [
            row.root,
            row.instance,
            row.block as usize,
            row.candidates.start,
            row.candidates.end,
            row.zero_edges.start,
            row.zero_edges.end,
            row.zero_rank,
            row.current_v281.start,
            row.current_v281.end,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.candidates {
        for value in [
            row.block,
            row.operation.is_some() as usize,
            row.operation.unwrap_or(0),
            row.prefix,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for &(from, to) in rows.zero_edges {
        number(&mut hash, from, budget)?;
        number(&mut hash, to, budget)?;
    }
    for demand in rows.frame_demands_v281 {
        let value = match demand.value {
            fe2o3_mir_model::SsaValueV1::Definition(id) => [0, id.get() as usize, 0],
            fe2o3_mir_model::SsaValueV1::BlockArgument { block, variable } => {
                [1, block.get() as usize, variable.get() as usize]
            }
        };
        for field in [
            demand.root,
            demand.instance,
            demand.local,
            demand.logical_local,
            demand.component_block,
            demand.overwritten.is_some() as usize,
            demand.overwritten.map_or(0, |range| range.0),
            demand.overwritten.map_or(0, |range| range.1),
            value[0],
            value[1],
            value[2],
        ] {
            number(&mut hash, field, budget)?;
        }
    }
    for row in rows.forwarding_v288 {
        use fe2o3_verifier::ExpandedSupportForwardingV288 as F;
        match *row {
            F::Begin {
                root,
                instance,
                value,
                atom,
                source_type,
                original,
                coordinate,
            } => {
                for field in [0, root, instance, atom, source_type as usize, original] {
                    number(&mut hash, field, budget)?;
                }
                let value = match value {
                    fe2o3_mir_model::SsaValueV1::Definition(id) => [0, id.get() as usize, 0],
                    fe2o3_mir_model::SsaValueV1::BlockArgument { block, variable } => {
                        [1, block.get() as usize, variable.get() as usize]
                    }
                };
                for field in value {
                    number(&mut hash, field, budget)?;
                }
                product_frames::definition_identity(&mut hash, coordinate, budget)?;
            }
            F::Erased {
                original,
                coordinate,
            } => {
                for field in [1, original] {
                    number(&mut hash, field, budget)?;
                }
                product_frames::definition_identity(&mut hash, coordinate, budget)?;
            }
            F::Incoming {
                original,
                edge,
                incoming,
                coordinate,
            } => {
                for field in [
                    2,
                    original,
                    edge.edge.source.function.0 as usize,
                    edge.edge.source.block as usize,
                    edge.edge.successor as usize,
                    edge.argument as usize,
                    incoming,
                ] {
                    number(&mut hash, field, budget)?;
                }
                product_frames::definition_identity(&mut hash, coordinate, budget)?;
            }
            F::Retained {
                original,
                coordinate,
                descendants,
            } => {
                for field in [3, original, descendants] {
                    number(&mut hash, field, budget)?;
                }
                product_frames::definition_identity(&mut hash, coordinate, budget)?;
            }
            F::Target {
                actual,
                coordinate,
                function,
            } => {
                for field in [4, actual, function.0 as usize] {
                    number(&mut hash, field, budget)?;
                }
                product_frames::definition_identity(&mut hash, coordinate, budget)?;
            }
        }
    }
    Ok(hash.finalize().into())
}

struct ModelCallbacks {
    wide: bool,
    result: Option<Result<CollectedObservation, String>>,
}

impl Callbacks for ModelCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let mut called = 0;
            let wide = self.wide;
            let result = transaction.with_original_source_expanded_model_v280(
                &mut budget,
                |source, original, tile, roots, _, pair, model, budget| {
                    called += 1;
                    let scratch = 2 * std::mem::size_of::<Sha256>()
                        + 40 * std::mem::size_of::<usize>()
                        + std::mem::size_of::<
                            fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>,
                        >()
                        + std::mem::size_of::<CollectedObservation>()
                        + 2 * std::mem::size_of::<WideRuntimeObservation>()
                        + 2 * std::mem::size_of::<[Option<WideRootObservation>; 2]>()
                        + 2 * std::mem::size_of::<WideRootObservation>()
                        + 4 * std::mem::size_of::<[u64; 3]>()
                        + 2 * std::mem::size_of::<[u32; 3]>()
                        + 16 * std::mem::size_of::<usize>()
                        + 16 * std::mem::size_of::<&()>();
                    budget.reserve_storage(scratch)?;
                    pair.check(source, original, tile, budget)?;
                    assert_eq!(pair.reference_count(budget)?, 0);
                    let subject = pair.subject(budget)?;
                    assert_eq!(subject.references, empty_reference_input_identity());
                    let bytes = model
                        .generated_source(budget)
                        .map_err(SourceError::ExpandedModel)?;
                    budget.charge_work(bytes.len())?;
                    let model_identity = Sha256::digest(bytes).into();
                    assert!(!bytes.is_empty());
                    let text = std::str::from_utf8(bytes).unwrap();
                    assert!(text.contains("micro: MemoryMicroStateV30"));
                    assert!(text.contains("micro.observations == target_prefix"));
                    let wide_runtime = if wide {
                        use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
                        let (width, endian) = pair.runtime(budget)?;
                        assert_eq!(width, FormalIndexWidth::Bits64);
                        assert_eq!(endian, EndiannessV2::Little);
                        let checked = pair.roots(budget)?;
                        assert_eq!(checked.len(), 2);
                        let mut captured = [None; 2];
                        for (root, input) in checked.iter().enumerate() {
                            budget.charge_work(24)?;
                            let retained = source.source_launch(budget)?.roots()[root];
                            assert_eq!(input.source_root, source.root(root, budget)?.0);
                            assert_eq!(input.source_launch, retained.source_launch());
                            assert_eq!(input.source_layout, retained.layout());
                            let rank = input.source_launch.rank();
                            let workgroup = input.source_launch.exact_workgroup().unwrap();
                            let grid = input.source_launch.max_grid();
                            assert_eq!(rank, 1);
                            assert_eq!(workgroup, [64, 1, 1]);
                            assert_eq!(&grid[1..], &[1, 1]);
                            assert!(matches!(grid[0], u32::MAX | 67_108_864));
                            let ExplicitLaunchExtent::Exact { rank: physical_rank, extents } = input.launch else {
                                panic!("exact authenticated physical envelope");
                            };
                            let expected = [
                                u64::from(workgroup[0]).checked_mul(u64::from(grid[0])).unwrap(),
                                1,
                                1,
                            ];
                            assert_eq!(physical_rank, rank);
                            assert_eq!(extents, expected);
                            // Only the architecture's dynamic grid limit uses a zero marker.
                            let source_extent = if grid[0] == u32::MAX { 0 } else { expected[0] };
                            assert_eq!(input.source_layout.global_extents(), [source_extent, 1, 1]);
                            captured[root] = Some(WideRootObservation {
                                source_root: input.source_root.index(), rank,
                                exact_workgroup: workgroup, max_grid: grid,
                                source_layout: input.source_layout.global_extents(),
                                physical_extents: extents,
                            });
                        }
                        let captured = captured.map(Option::unwrap);
                        assert_eq!(captured[0].max_grid, captured[1].max_grid);
                        let launch_literals = match captured[0].max_grid[0] {
                            u32::MAX => [
                                "spec fn invocation_runtime_launch_0_v36() -> (int, Seq<int>) { (1, seq![274877906880int, 1int, 1int]) }",
                                "spec fn invocation_runtime_launch_1_v36() -> (int, Seq<int>) { (1, seq![274877906880int, 1int, 1int]) }",
                            ],
                            67_108_864 => [
                                "spec fn invocation_runtime_launch_0_v36() -> (int, Seq<int>) { (1, seq![4294967296int, 1int, 1int]) }",
                                "spec fn invocation_runtime_launch_1_v36() -> (int, Seq<int>) { (1, seq![4294967296int, 1int, 1int]) }",
                            ],
                            _ => unreachable!(),
                        };
                        for literal in [
                            "spec fn invocation_runtime_index_bytes_v36() -> int { 8 }",
                            launch_literals[0], launch_literals[1],
                        ] {
                            budget.charge_work(text.len())?;
                            assert_eq!(text.matches(literal).count(), 1);
                        }
                        Some(WideRuntimeObservation { index_bits: 64, little_endian: true, roots: captured })
                    } else {
                        None
                    };
                    assert!(
                        bytes.len() <= fe2o3_verifier::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3
                    );
                    let rows = model.census(budget).map_err(SourceError::ExpandedModel)?;
                    assert_eq!(rows.roots.len(), roots.len());
                    assert_eq!(rows.roots.len(), source.root_count(budget)?);
                    let (mut instance_end, mut cut_end) = (0, 0);
                    for (root, row) in rows.roots.iter().enumerate() {
                        budget.charge_work(1)?;
                        let (function, original_function) = source.root(root, budget)?;
                        assert_eq!(
                            (row.source_function, row.original_function),
                            (function.index(), original_function)
                        );
                        assert_eq!(
                            row.target_function,
                            pair.roots(budget)?[root].target_function
                        );
                        assert_eq!(row.instances.start, instance_end);
                        assert_eq!(row.cuts.start, cut_end);
                        assert_eq!(row.instances.len(), source.instance_count(root, budget)?);
                        for (instance, entry) in
                            rows.instances[row.instances.clone()].iter().enumerate()
                        {
                            budget.charge_work(1)?;
                            let (function, incoming) = source.instance(root, instance, budget)?;
                            assert_eq!(
                                (entry.root, entry.instance, entry.source_function),
                                (root, instance, function.index())
                            );
                            assert_eq!(
                                entry.active,
                                source.instance_active(root, instance, budget)?
                            );
                            assert_eq!(
                                entry.incoming,
                                incoming.map(|(caller, block)| (caller, block.index()))
                            );
                            let blocks = source.source_semantic(budget)?.functions()
                                [function.index() as usize]
                                .blocks();
                            assert_eq!(entry.cuts.len(), blocks.len());
                            for (block, pc) in entry.cuts.clone().enumerate() {
                                budget.charge_work(1)?;
                                let cut = &rows.cuts[pc];
                                assert_eq!(
                                    (cut.root, cut.instance, cut.block),
                                    (root, instance, block as u32)
                                );
                                if !entry.active {
                                    assert!(cut.candidates.is_empty());
                                    assert!(cut.current_v281.is_empty());
                                }
                                for demand in &rows.frame_demands_v281[cut.current_v281.clone()] {
                                    budget.charge_work(1)?;
                                    assert_eq!(
                                        (demand.root, demand.instance, demand.component_block),
                                        (root, instance, block)
                                    );
                                }
                            }
                            if let Some((parent, block)) = incoming {
                                assert!(parent < instance);
                                assert_eq!(
                                    original.defined_call_instance(root, parent, block, budget)?,
                                    instance
                                );
                                let call = &rows.calls[entry.parent_call_v281.unwrap()];
                                if !entry.active {
                                    assert!(call.carries_v281.is_empty());
                                }
                                assert_eq!(
                                    (call.root, call.caller, call.block, call.child),
                                    (root, parent, block.index(), Some(instance))
                                );
                                let ancestor = &rows.instances[row.instances.start + parent];
                                assert_eq!(entry.depth_v281, ancestor.depth_v281 + 1);
                                for demand in &rows.frame_demands_v281[call.carries_v281.clone()] {
                                    budget.charge_work(1)?;
                                    assert_eq!((demand.root, demand.instance), (root, parent));
                                    assert_eq!(
                                        Some(demand.component_block),
                                        call.continuation_v281
                                    );
                                }
                            } else {
                                assert_eq!(instance, 0);
                                assert_eq!((entry.parent_call_v281, entry.depth_v281), (None, 0));
                            }
                        }
                        instance_end = row.instances.end;
                        cut_end = row.cuts.end;
                    }
                    assert_eq!(
                        (instance_end, cut_end),
                        (rows.instances.len(), rows.cuts.len())
                    );
                    assert!(!rows.frame_demands_v281.is_empty());
                    for demand in rows.frame_demands_v281 {
                        budget.charge_work(2)?;
                        let frame = &rows.instances
                            [rows.roots[demand.root].instances.start + demand.instance];
                        assert!(demand.local < frame.locals.len());
                        assert_eq!(demand.logical_local, frame.locals.start + demand.local);
                        let endpoint = original.ssa_typed_endpoint_v36(
                            demand.root,
                            demand.instance,
                            demand.value,
                            budget,
                        )?;
                        assert_eq!(
                            endpoint.source_function(budget)?.index(),
                            frame.source_function
                        );
                        assert_eq!(
                            endpoint.source_local(budget)?.index() as usize,
                            demand.local
                        );
                    }
                    let (helper_instances, mapped) =
                        expanded_roots_tests::original_helper_instances(source, original, budget)?;
                    assert!(mapped.into_iter().all(|count| count > 0));
                    let observation = ModelObservation {
                        graphs: subject.graphs.map(|identity| *identity.digest()),
                        runtime_and_instances: subject.runtime_and_instances,
                        references: subject.references,
                        model: model_identity,
                        census: census_identity(&rows, budget)?,
                        counts: [
                            rows.roots.len(),
                            rows.instances.len(),
                            rows.calls.len(),
                            rows.cuts.len(),
                            rows.candidates.len(),
                            rows.zero_edges.len(),
                        ],
                        helper_instances,
                        model_consumer_called: true,
                    };
                    // Wide boundary cases are observed in-process only. Their
                    // reused response basenames must not enter the finite export roster.
                    if !wide {
                        model_export::observe(bytes, &observation, budget)?;
                    }
                    budget.release_storage(scratch)?;
                    Ok((CollectedObservation { observation, wide_runtime }, 0))
                },
            );
            let observed = result
                .map_err(|error| format!("actual expanded support: {error:?}"))?
                .into_observation();
            assert_eq!(called, 1);
            Ok(observed)
        })());
        Compilation::Stop
    }
}

fn child(wide: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ModelCallbacks { wide, result: None };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual expanded model callback");
    if wide {
        let result = result.map(|collected| WideModelObservation {
            observation: collected.observation,
            runtime: collected.wide_runtime.expect("wide runtime capture"),
            diagnostic_export: false,
        });
        std::fs::write(
            env::var_os(RESULT).unwrap(),
            serde_json::to_vec(&result).unwrap(),
        )
        .unwrap();
        assert!(result.is_ok(), "wide expanded support: {result:?}");
    } else {
        let result = result.map(|collected| {
            assert!(collected.wide_runtime.is_none());
            Some(collected.observation)
        });
        std::fs::write(
            env::var_os(RESULT).unwrap(),
            serde_json::to_vec(&result).unwrap(),
        )
        .unwrap();
        assert!(result.is_ok(), "expanded support: {result:?}");
    }
}

#[test]
#[ignore = "process helper; exact source request supplied by its parent"]
fn expanded_model_child() {
    child(false);
}

#[test]
#[ignore = "process helper; exact source request supplied by its parent"]
fn expanded_model_wide_runtime_child() {
    child(true);
}

#[test]
#[ignore = "process helper; exact finite source and accounting case supplied by its parent"]
fn expanded_model_account_case_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = AccountCallbacks {
        case: serde_json::from_str(&env::var(ACCOUNT_CASE).unwrap()).unwrap(),
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual model accounting callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "model accounting: {result:?}");
}

#[test]
#[ignore = "process helper; exact finite source request supplied by its parent"]
fn expanded_model_account_child() {
    if env::var_os(ARGS).is_none() {
        return;
    }
    let response = PathBuf::from(env::var_os(RESULT).unwrap());
    // Each case gets new one-shot MIR custody rather than recollecting a session.
    for (ordinal, case) in ACCOUNT_CASES.into_iter().enumerate() {
        let case_response = response.with_extension(format!("model-account-{ordinal}.json"));
        assert!(!case_response.exists());
        let child = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                &ACCOUNT_CHILD.replace(
                    "expanded_model_account_child",
                    "expanded_model_account_case_child",
                ),
                "--ignored",
                "--nocapture",
            ])
            .env(ACCOUNT_CASE, serde_json::to_string(&case).unwrap())
            .env(RESULT, &case_response)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "model case {case:?}: {}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        let result: Result<(), String> =
            serde_json::from_slice(&std::fs::read(case_response).unwrap()).unwrap();
        result.unwrap();
    }
    let result: Result<usize, String> = Ok(ACCOUNT_CASES.len());
    std::fs::write(response, serde_json::to_vec(&result).unwrap()).unwrap();
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic ordinary AMD source compilation"]
fn actual_rustc_expanded_support_model_covers_complete_roots_and_runtime_width_boundaries() {
    run_actual_sources::<Option<ModelObservation>>(
        &[
            ("two", "two"),
            ("two", "two"),
            ("mixed", "mixed"),
            ("scalar", "scalar"),
            ("helper", "helper"),
        ],
        &[(0, 0), (3, 0)],
        MODEL_CHILD,
        "EXPANDED_SUPPORT_V280",
        |case| expanded_roots_tests::root_source_with_grid(case, Some(3)),
        |_, _, label, outcome, observations| {
            let observed = outcome.as_ref().expect("complete model must be generated");
            assert!(observed.model_consumer_called);
            assert_eq!(observed.counts[0], 2);
            assert!(observed.counts[1] >= 2 && observed.counts[3] > 0);
            assert_ne!(observed.model, [0; 32]);
            assert_ne!(observed.census, [0; 32]);
            if let Some(previous) = observations.get(label) {
                assert_eq!(&outcome, previous);
            } else {
                observations.insert(label.to_owned(), outcome);
            }
        },
    );
    run_actual_sources::<WideModelObservation>(
        &[("default", "default"), ("above-u32", "above-u32")],
        &[(0, 0), (3, 0)],
        WIDE_CHILD,
        "EXPANDED_SUPPORT_WIDE_V292",
        |case| {
            expanded_roots_tests::root_source_with_grid(
                "two",
                match case {
                    "default" => None,
                    "above-u32" => Some(67_108_864),
                    _ => unreachable!(),
                },
            )
        },
        |_, _, label, outcome, _| {
            let (grid, source_extent) = match label {
                "default" => (u32::MAX, 0),
                "above-u32" => (67_108_864, 4_294_967_296),
                _ => unreachable!(),
            };
            assert_eq!(outcome.runtime.index_bits, 64);
            assert!(outcome.runtime.little_endian);
            assert!(!outcome.diagnostic_export);
            assert!(outcome.observation.model_consumer_called);
            assert_eq!(outcome.observation.counts[0], 2);
            assert_ne!(outcome.observation.model, [0; 32]);
            assert_ne!(outcome.observation.census, [0; 32]);
            assert_ne!(
                outcome.runtime.roots[0].source_root,
                outcome.runtime.roots[1].source_root
            );
            for root in outcome.runtime.roots {
                assert_eq!(root.rank, 1);
                assert_eq!(root.exact_workgroup, [64, 1, 1]);
                assert_eq!(root.max_grid, [grid, 1, 1]);
                assert_eq!(root.source_layout, [source_extent, 1, 1]);
                assert_eq!(root.physical_extents, [64 * u64::from(grid), 1, 1]);
            }
        },
    );
    run_actual_sources::<usize>(
        &[("model-account", "two")],
        &[(0, 0)],
        ACCOUNT_CHILD,
        "EXPANDED_SUPPORT_ACCOUNT_V280",
        |case| expanded_roots_tests::root_source_with_grid(case, Some(3)),
        |_, _, _, completed, _| assert_eq!(completed, ACCOUNT_CASES.len()),
    );
}
