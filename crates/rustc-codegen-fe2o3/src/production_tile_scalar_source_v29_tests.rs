//! Real Rust -> SSA -> Pending -> complete checked tile candidate; no final safety grant.
use super::*;
use fe2o3_kernel_ir::{AddressSpace, ExecutionOperationV15 as Execution, OperationKind};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedTileScalarTransportV29 as Checked,
    ProductionTileCallInstanceV29 as CallInstance,
    ProductionTileCallOccurrenceV29 as CallOccurrence,
    ProductionTileOccurrenceCoordinateV29 as Coordinate,
    ProductionTilePendingKindV29 as PendingKind, ProductionTileScalarOrderV29 as Order,
    ProductionTileScalarTransportErrorV29 as TransportError,
};
#[path = "production_tile_scalar_source_protocol_v29_tests.rs"]
mod protocol;

const BLOCKED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::tile_scalar_source_tests::tile_source_blocked_child";
const STRIPED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::tile_scalar_source_tests::tile_source_striped_child";

#[test]
fn public_tile_source_coordinate_types_and_getters_are_accessible() {
    use fe2o3_lower_mir_kernel::{
        ProductionTileInstanceV29, ProductionTilePendingGlobalReadV29,
        ProductionTileSourceAliasV29, ProductionTileWorkgroupSubjectV29,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1;
    fn inert<T: Copy + Eq + std::fmt::Debug>() {}
    inert::<CallInstance>();
    inert::<CallOccurrence>();
    assert_eq!(
        std::mem::size_of::<CallInstance>(),
        std::mem::size_of::<usize>()
    );
    let _: fn(CallInstance) -> usize = CallInstance::index;
    let _: fn(CallOccurrence) -> CallInstance = CallOccurrence::caller;
    let _: fn(CallOccurrence) -> SemanticBlockIdV1 = CallOccurrence::block;
    let _: fn(&ProductionTileInstanceV29) -> CallInstance = ProductionTileInstanceV29::instance;
    let _: fn(&ProductionTileInstanceV29) -> Option<CallOccurrence> =
        ProductionTileInstanceV29::incoming;
    let _: fn(&ProductionTileSourceAliasV29) -> CallInstance =
        ProductionTileSourceAliasV29::instance;
    let _: fn(&ProductionTileSourceAliasV29) -> Option<CallOccurrence> =
        ProductionTileSourceAliasV29::removed_call;
    let _: fn(&ProductionTileWorkgroupSubjectV29) -> CallOccurrence =
        ProductionTileWorkgroupSubjectV29::producer;
    let _: fn(&ProductionTilePendingGlobalReadV29) -> CallInstance =
        ProductionTilePendingGlobalReadV29::instance;
}

fn inspect_call_occurrence(
    view: &Checked<'_>,
    root: usize,
    occurrence: CallOccurrence,
    budget: &mut Budget<'_>,
) -> Result<(), TransportError> {
    let mut source_function = None;
    for ordinal in view.root(root, budget)?.instances() {
        let instance = view.instance(ordinal, budget)?;
        if instance.instance() == occurrence.caller() {
            assert!(source_function.replace(instance.function()).is_none());
        }
    }
    let function = source_function.expect("caller belongs to the exact retained root");
    let semantic = view.source_semantic(budget)?;
    let source = &semantic.functions()[function.index() as usize];
    let block = &source.blocks()[occurrence.block().index() as usize];
    budget.charge_work(1)?;
    assert!(matches!(
        block.terminator().kind(),
        fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1::Call(_)
    ));
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct TileObservation {
    source: [u8; 32],
    pending: [u8; 32],
    current: [u8; 32],
    request: [u8; 32],
    source_file: [u8; 32],
    target: u16,
    order: u8,
    roots: usize,
    instances: usize,
    aliases: usize,
    attachments: usize,
    piece_aliases: usize,
    inventory: [usize; 7],
    mapped: [usize; 7],
    pending_kinds: [usize; 5],
    obligations: usize,
    source_loads: usize,
    source_parts: usize,
    generated_reads: usize,
    part_results: usize,
    multiple_aliases: usize,
    callbacks: usize,
}
fn inspect(
    view: &Checked<'_>,
    budget: &mut Budget<'_>,
    order: Order,
) -> Result<TileObservation, TransportError> {
    let inventory = view.current_inventory(budget)?;
    let pending = view.pending_ancestor(budget)?;
    let mut report = TileObservation {
        source: *view.source_identity(budget)?,
        pending: *view.pending_identity(budget)?.digest(),
        current: *view.current_identity(budget)?.digest(),
        request: [0; 32],
        source_file: [0; 32],
        target: 0,
        order: match order {
            Order::Blocked => 0,
            Order::Striped => 1,
        },
        roots: view.root_count(budget)?,
        instances: view.instance_count(budget)?,
        aliases: view.source_alias_count(budget)?,
        attachments: view.attachment_count(budget)?,
        piece_aliases: view.piece_alias_count(budget)?,
        inventory: [
            inventory.operations().len(),
            inventory.definitions().len(),
            inventory.uses().len(),
            inventory.blocks().len(),
            inventory.blocks().len(),
            inventory.edges().len(),
            inventory.edge_arguments().len(),
        ],
        mapped: [
            view.operation_count(budget)?,
            view.definition_count(budget)?,
            view.use_site_count(budget)?,
            view.block_count(budget)?,
            view.terminator_count(budget)?,
            view.edge_count(budget)?,
            view.edge_argument_count(budget)?,
        ],
        pending_kinds: [0; 5],
        obligations: view.pending_obligation_count(budget)?,
        source_loads: 0,
        source_parts: 0,
        generated_reads: 0,
        part_results: 0,
        multiple_aliases: 0,
        callbacks: 1,
    };
    assert_eq!(
        inventory.functions().len(),
        pending.pending_module().functions.len()
    );
    for (current, original) in inventory
        .functions()
        .iter()
        .zip(&pending.pending_module().functions)
    {
        budget.charge_work(1)?;
        assert_eq!(current.function.id, original.id);
        assert_eq!(current.function.signature, original.signature);
        assert_eq!(current.function.role, original.role);
        assert_eq!(current.function.body.is_some(), original.body.is_some());
    }
    for function in &pending.pending_module().functions {
        budget.charge_work(1)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    match operation.kind {
                        OperationKind::Execution(Execution::MaskedTileLoadU32 {
                            elements, ..
                        }) => {
                            assert_eq!(elements, 2);
                            report.source_loads += 1;
                        }
                        OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                            elements,
                            ..
                        }) => {
                            assert_eq!(elements, 2);
                            report.source_parts += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let mut incoming_calls = 0;
    for ordinal in 0..view.instance_count(budget)? {
        let instance = view.instance(ordinal, budget)?;
        let root = view.root(instance.root(), budget)?;
        assert!(root.instances().contains(&ordinal));
        assert!(instance.instance().index() < root.instances().len());
        if let Some(call) = instance.incoming() {
            inspect_call_occurrence(view, instance.root(), call, budget)?;
            incoming_calls += 1;
        }
    }
    let mut removed_calls = 0;
    for ordinal in 0..view.source_alias_count(budget)? {
        let alias = view.source_alias(ordinal, budget)?;
        let root = view.root(alias.root(), budget)?;
        assert!(root.source_aliases().contains(&ordinal));
        assert!(alias.instance().index() < root.instances().len());
        if let Some(call) = alias.removed_call() {
            inspect_call_occurrence(view, alias.root(), call, budget)?;
            removed_calls += 1;
        }
    }
    assert!(incoming_calls > 0 && removed_calls > 0);
    for (i, actual) in inventory.operations().iter().enumerate() {
        assert_eq!(
            view.operation(i, budget)?.coordinate(),
            Coordinate::Operation(actual.coordinate)
        );
    }
    for (i, actual) in inventory.definitions().iter().enumerate() {
        let row = view.definition(i, budget)?;
        assert_eq!(row.coordinate(), Coordinate::Definition(actual.coordinate));
        assert_eq!(row.value(), actual.value);
        if row.piece_aliases().len() > 1 {
            report.multiple_aliases += 1;
        }
        for alias in row.piece_aliases() {
            assert!(view.piece_alias(alias, budget)? < view.piece_count(budget)?);
        }
    }
    for (i, actual) in inventory.uses().iter().enumerate() {
        let row = view.use_site(i, budget)?;
        assert_eq!(row.coordinate(), Coordinate::Use(actual.coordinate));
        assert_eq!(row.definition(), Some(actual.definition));
        assert_eq!(row.value(), Some(actual.value));
    }
    for (i, actual) in inventory.blocks().iter().enumerate() {
        assert_eq!(
            view.block(i, budget)?.coordinate(),
            Coordinate::Block(actual.coordinate)
        );
        assert_eq!(
            view.terminator(i, budget)?.coordinate(),
            Coordinate::Terminator(actual.coordinate)
        );
    }
    for (i, actual) in inventory.edges().iter().enumerate() {
        assert_eq!(
            view.edge(i, budget)?.coordinate(),
            Coordinate::Edge(actual.coordinate)
        );
    }
    for (i, actual) in inventory.edge_arguments().iter().enumerate() {
        let row = view.edge_argument(i, budget)?;
        assert_eq!(
            row.coordinate(),
            Coordinate::EdgeArgument(actual.coordinate)
        );
        assert_eq!(row.definition(), Some(actual.incoming_definition));
        assert_eq!(row.target_definition(), Some(actual.target_definition));
        assert_eq!(row.value(), Some(actual.value));
    }
    for i in 0..view.piece_count(budget)? {
        let piece = view.piece(i, budget)?;
        if piece.stage() == fe2o3_lower_mir_kernel::ProductionTileExpansionStageV29::Parts
            && matches!(
                piece.target(),
                fe2o3_lower_mir_kernel::ProductionTileTargetCoordinateV29::Result { .. }
            )
        {
            report.part_results += 1;
        }
    }
    for i in 0..report.obligations {
        let row = view.pending_obligation(i, budget)?;
        let kind = match row.kind() {
            PendingKind::Source { alias } => {
                assert!(!view.source_alias(alias, budget)?.attachments().is_empty());
                0
            }
            PendingKind::CollectiveLifecycle { .. } => 1,
            PendingKind::LaunchGeometry { root } => {
                view.root(root, budget)?;
                2
            }
            PendingKind::GlobalRead => {
                let read = row.global_read().expect("pending generated-read subject");
                let source = read.source();
                let function =
                    &pending.pending_module().functions[source.block.function.0 as usize];
                let original = &function.body.as_ref().unwrap().blocks[source.block.block as usize]
                    .operations[source.operation as usize];
                let OperationKind::Execution(Execution::MaskedTileLoadU32 {
                    workgroup,
                    input,
                    base,
                    lanes,
                    elements,
                }) = original.kind
                else {
                    panic!("read source must be an original pending Load")
                };
                assert_eq!(read.input(), input);
                assert_eq!(read.base(), base);
                assert_eq!(read.workgroup().value(), workgroup);
                assert_eq!(read.lanes(), lanes);
                assert_eq!(read.elements(), elements);
                assert!(read.component() < u32::from(elements));
                assert!(read.selection() < report.source_loads);
                let semantic = view.source_semantic(budget)?;
                assert_eq!(
                    read.workgroup().type_identity(),
                    semantic.types()[read.workgroup().semantic_type().index() as usize].identity()
                );
                inspect_call_occurrence(view, read.root(), read.workgroup().producer(), budget)?;
                // Every original Load must have all components exactly once and one
                // distinct selection ordinal, independent of the transport builder.
                let mut components = [false; 2];
                for other in 0..report.obligations {
                    let other = view.pending_obligation(other, budget)?;
                    let Some(other) = other.global_read() else {
                        continue;
                    };
                    if other.source() == source {
                        assert_eq!(other.selection(), read.selection());
                        assert!(other.component() < u32::from(elements));
                        assert!(!std::mem::replace(
                            &mut components[other.component() as usize],
                            true
                        ));
                    } else {
                        assert_ne!(other.selection(), read.selection());
                    }
                }
                assert!(components.into_iter().all(|seen| seen));
                assert_eq!(
                    inventory.operations()[read.operation()].coordinate,
                    read.output()
                );
                assert!(
                    matches!(inventory.operations()[read.operation()].operation.kind,
                    OperationKind::Load { access, .. } if access.address_space == AddressSpace::Global)
                );
                assert!(
                    view.origin(read.origin(), budget)?
                        .pieces()
                        .contains(&read.piece())
                );
                assert_eq!(
                    view.source_alias(read.source_alias(), budget)?.instance(),
                    read.instance()
                );
                assert_eq!(
                    view.root(read.root(), budget)?.function(),
                    read.output().block.function
                );
                assert_eq!(
                    view.root(read.root(), budget)?.function(),
                    source.block.function
                );
                report.generated_reads += 1;
                3
            }
            PendingKind::RetainedAttachment { attachment } => {
                view.attachment(attachment, budget)?;
                4
            }
        };
        report.pending_kinds[kind] += 1;
    }
    Ok(report)
}
struct TileCallbacks {
    order: Order,
    visits: usize,
    result: Option<Result<TileObservation, String>>,
}
impl Callbacks for TileCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.visits += 1;
        assert_eq!(self.visits, 1, "one actual compiler transaction");
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut visits = 0;
            let report = transaction
                .consume_checked_tile_scalar_source_v29(self.order, |view, budget| {
                    visits += 1;
                    assert_eq!(visits, 1, "one actual complete checked candidate");
                    inspect(view, budget, self.order)
                })
                .map_err(|error| format!("real tile source transport refused: {error:?}"))?;
            assert_eq!(visits, 1);
            Ok(report)
        })());
        Compilation::Stop
    }
}
fn run_child(order: Order) {
    let request = protocol::Request::read();
    let mut callbacks = TileCallbacks {
        order,
        visits: 0,
        result: None,
    };
    rustc_driver::run_compiler(&request.args, &mut callbacks);
    request.assert_unchanged();
    assert_eq!(callbacks.visits, 1);
    let result = callbacks
        .result
        .expect("actual rustc callback did not run")
        .map(|mut row| {
            row.request = request.request_hash;
            row.source_file = request.source_hash;
            row.target = request.target;
            protocol::validate(&row).expect("complete real report");
            row
        });
    assert!(!request.response.exists());
    std::fs::write(&request.response, serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(
        result.is_ok(),
        "tile transport producer refusal: {result:?}"
    );
}
#[test]
#[ignore = "strict child; requires FE2O3_TEST_CONTEXT_SOURCE_ARGS_V29, FE2O3_TEST_CONTEXT_SOURCE_RESULT_V29 and FE2O3_CONTEXT_PROTOCOL_SOURCE from managed parent"]
fn tile_source_blocked_child() {
    run_child(Order::Blocked);
}
#[test]
#[ignore = "strict child; requires FE2O3_TEST_CONTEXT_SOURCE_ARGS_V29, FE2O3_TEST_CONTEXT_SOURCE_RESULT_V29 and FE2O3_CONTEXT_PROTOCOL_SOURCE from managed parent"]
fn tile_source_striped_child() {
    run_child(Order::Striped);
}

const CASES: &[(&str, &str)] = &[
    ("single", "single"),
    ("repeated_helper", "repeated"),
    ("branch_parts", "branch"),
];
fn program(case: &str) -> String {
    let (extract, call) = match case {
        "single" => (
            "let ([a,b],[ma,mb]) = fragment.into_parts();",
            "read(&workgroup,input,base as usize)",
        ),
        "repeated" => (
            "let ([a,b],[ma,mb]) = fragment.into_parts();",
            "read(&workgroup,input,base as usize).wrapping_add(read(&workgroup,input,base.wrapping_add(7) as usize))",
        ),
        "branch" => (
            "let ([a,b],[ma,mb]) = if base == 0 { fragment.into_parts() } else { let ([a,b],mask) = fragment.into_parts(); ([a.wrapping_add(1),b],mask) };",
            "read(&workgroup,input,base as usize)",
        ),
        _ => panic!("unknown source case"),
    };
    format!(
        r#"use fe2o3_device::{{kernel, thread, DisjointSlice, KernelContext, WorkgroupCapability, MaskedTile1D}};
#[inline(never)]
fn read<Brand>(workgroup: &WorkgroupCapability<'_, Brand>, input: &[u32], base: usize) -> u32 {{
    let tile = MaskedTile1D::<u32,64,2,_>::load_masked(workgroup,input,base);
    let fragment = tile.into_fragment();
    {extract}
    let a = if ma {{ a }} else {{ 0 }};
    let b = if mb {{ b }} else {{ 0 }};
    a.wrapping_add(b)
}}
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1]))]
pub fn tile_probe(mut ctx: KernelContext<'_>, input: &[u32], base: u64, mut output: DisjointSlice<u32>) {{
    ctx.with_workgroup(|workgroup| {{
        let value = {call};
        if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
    }});
}}
"#
    )
}
fn managed(order: Order) {
    run_actual_sources::<TileObservation>(
        CASES,
        &[(0, 0), (3, 2)],
        match order {
            Order::Blocked => BLOCKED_CHILD,
            Order::Striped => STRIPED_CHILD,
        },
        "CHECKED_TILE_SOURCE_TRANSPORT",
        program,
        |_, _, label, report, _| {
            protocol::validate(&report).unwrap();
            assert_eq!(
                report.order,
                match order {
                    Order::Blocked => 0,
                    Order::Striped => 1,
                }
            );
            let case = CASES.iter().find(|(name, _)| *name == label).unwrap().1;
            let expected: [u8; 32] = Sha256::digest(program(case)).into();
            assert_eq!(report.source_file, expected);
            if case == "repeated" {
                assert!(report.source_loads >= 2);
            }
            if case == "branch" {
                assert!(report.source_parts >= 2);
            }
            protocol::audit_real_report(&report);
        },
    );
}
#[test]
#[ignore = "managed real-source gate: pinned nightly rust-src, SDK dependencies; gfx942/gfx950, opt0/mir0 and opt3/mir2"]
fn actual_blocked_tile_source_reaches_checked_transport() {
    managed(Order::Blocked);
}
#[test]
#[ignore = "managed real-source gate: pinned nightly rust-src, SDK dependencies; gfx942/gfx950, opt0/mir0 and opt3/mir2"]
fn actual_striped_tile_source_reaches_checked_transport() {
    managed(Order::Striped);
}
