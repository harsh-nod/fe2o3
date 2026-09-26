//! Actual Rust enters the production prepared core and consumes its original V18 owner.
use super::*;
use fe2o3_kernel_ir::{AddressSpace, ExecutionOperationV15 as Execution, OperationKind};
use fe2o3_lower_mir_kernel::{
    ProductionSourceOwnedViewErrorV18 as ViewError, ProductionSourceOwnedViewV18 as View,
};

const SOURCE_OWNED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_v18_tests::source_owned_entrance_child";
const SOURCE_SCALAR_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_v18_tests::source_owned_scalar_consumer_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct OwnedObservation {
    source: [u8; 32],
    graph: [u8; 32],
    canonical_bytes: u64,
    nominal_profile: bool,
    capture: bool,
    kernels: usize,
    rows: usize,
    instances: usize,
    invocation_entries: usize,
    spans: usize,
    anchors: usize,
    assertions: usize,
    issues: usize,
    derives: usize,
    ends: usize,
    stores: usize,
    abi_arguments: usize,
    shared_slice_arguments: usize,
    compiler_laid_out_arguments: usize,
    generic_slice_parameters: usize,
    global_slice_parameters: usize,
    source_as0_slice_types: usize,
    helper_global_slice_parameters: usize,
    helper_generic_slice_parameters: usize,
    descriptor_widenings: usize,
}

fn inspect_original(
    view: &View<'_>,
    budget: &mut Budget<'_>,
) -> Result<OwnedObservation, ViewError> {
    let source = view.source_ssa(budget)?;
    view.check_original_source(source, budget)?;
    let original = view.source_semantic(budget)?;
    assert_eq!(
        original.semantic_sha256().as_bytes(),
        source.source_semantic_sha256()
    );
    assert_eq!(
        view.source_launch(budget)?.semantic_sha256(),
        source.source_semantic_sha256()
    );
    let graph = view.canonical(budget)?;
    assert!(std::ptr::eq(graph, view.canonical(budget)?));
    let floor = budget.storage();
    let mut result = OwnedObservation {
        source: *source.source_semantic_sha256(),
        graph: *graph.identity().digest(),
        canonical_bytes: graph.identity().canonical_length(),
        nominal_profile: matches!(original.wire_version(), SemanticMirWireVersionV1::V29),
        capture: source.occurrence_storage().is_some(),
        kernels: graph.module().kernels.len(),
        rows: graph.module().storage_layouts.len(),
        instances: 0,
        invocation_entries: 0,
        spans: 0,
        anchors: 0,
        assertions: view.assertion_count(budget)?,
        issues: 0,
        derives: 0,
        ends: 0,
        stores: 0,
        abi_arguments: 0,
        shared_slice_arguments: 0,
        compiler_laid_out_arguments: 0,
        generic_slice_parameters: 0,
        global_slice_parameters: 0,
        source_as0_slice_types: 0,
        helper_global_slice_parameters: 0,
        helper_generic_slice_parameters: 0,
        descriptor_widenings: 0,
    };
    for declaration in original.types() {
        budget.charge_work(1)?;
        if matches!(declaration.shape(), fe2o3_mir_model::semantic_mir_v1::SemanticTypeShapeV1::Pointer(pointer)
            if pointer.address_space() == 0
                && pointer.metadata() == fe2o3_mir_model::semantic_mir_v1::SemanticPointerMetadataV1::SliceLength)
        {
            result.source_as0_slice_types += 1;
        }
    }
    assert_eq!(view.root_count(budget)?, result.kernels);
    for root in 0..result.kernels {
        let (_, function) = view.root(root, budget)?;
        assert!(graph.module().functions[function].body.is_some());
        let arguments = view.kernel_argument_abi_count(root, budget)?
            .expect("the actual Prepared descriptor producer retains a complete ABI census");
        result.abi_arguments += arguments;
        for argument in 0..arguments {
            match view.kernel_argument_abi_kind(root, argument, budget)? {
                Some(fe2o3_kernel_descriptor::SourceTypeDescriptorV3::SharedSlice(_)) => {
                    result.shared_slice_arguments += 1;
                }
                None => result.compiler_laid_out_arguments += 1,
                _ => {}
            }
        }
        for parameter in &graph.module().functions[function].signature.parameters {
            if let fe2o3_kernel_ir::Type::Slice(slice) = parameter {
                if slice.address_space == AddressSpace::Generic {
                    result.generic_slice_parameters += 1;
                } else if slice.address_space == AddressSpace::Global {
                    result.global_slice_parameters += 1;
                }
            }
        }
        let instances = view.instance_count(root, budget)?;
        result.instances += instances;
        result.spans += view.span_count(root, budget)?;
        for instance in 0..instances {
            let (_, incoming) = view.instance(root, instance, budget)?;
            assert_eq!(incoming.is_none(), instance == 0);
            result.invocation_entries +=
                usize::from(view.invocation_entry(root, instance, budget)?.is_some());
            result.anchors += view.memory_anchor_count(root, instance, budget)?;
        }
    }
    for function in &graph.module().functions {
        budget.charge_work(1)?;
        if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
            for parameter in &function.signature.parameters {
                budget.charge_work(1)?;
                if let fe2o3_kernel_ir::Type::Slice(slice) = parameter {
                    match slice.address_space {
                        AddressSpace::Global => result.helper_global_slice_parameters += 1,
                        AddressSpace::Generic => result.helper_generic_slice_parameters += 1,
                        _ => {}
                    }
                }
            }
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    match &operation.kind {
                        OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::SliceToGeneric, .. } => result.descriptor_widenings += 1,
                        OperationKind::Execution(Execution::ContextIssue) => result.issues += 1,
                        OperationKind::Execution(Execution::WorkgroupDerive { .. }) => {
                            result.derives += 1
                        }
                        OperationKind::Execution(Execution::ScopeEnd { .. }) => result.ends += 1,
                        OperationKind::Store { access, .. }
                        | OperationKind::GuardedStore { access, .. }
                            if access.address_space == AddressSpace::Global =>
                        {
                            result.stores += 1
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert_eq!(
        budget.storage(),
        floor,
        "queries borrow existing rows without rebuilding an index"
    );
    budget.reserve_storage(std::mem::size_of::<OwnedObservation>())?;
    Ok(result)
}

#[derive(Default)]
struct OwnedCallbacks {
    result: Option<Result<OwnedObservation, String>>,
}

impl Callbacks for OwnedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let observation = transaction
                .consume_source_owned_v18(inspect_original)
                .map_err(|error| format!("actual source-owned prepared entrance: {error:?}"))?;
            // A separate authentic transaction runs only the hostile capture
            // boundary. It cannot replace or certify the successful owner above.
            transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .check_source_preparation_refusal_v18()
            .map_err(|error| format!("actual preparation refusal: {error:?}"))?;
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_entrance_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = OwnedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual source-owning rustc callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("source-owned report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "original source-owned entrance: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_nominal_source_uses_original_prepared_v18_entrance() {
    run_actual_sources::<OwnedObservation>(
        &[
            ("plain", "let value = seed;"),
            ("workgroup", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
            ("plain", "let value = seed;"),
        ],
        &[(0, 0), (3, 2)],
        SOURCE_OWNED_CHILD,
        "SOURCE_OWNED_V18_NOMINAL",
        source,
        |_, _, label, observation, previous| {
            assert!(observation.nominal_profile && observation.capture);
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.issues, 1);
            let derives = usize::from(label == "workgroup");
            assert_eq!((observation.derives, observation.ends), (derives, derives));
            assert_eq!(observation.stores, 1);
            assert!(
                observation.instances >= 2
                    && observation.spans > 0
                    && observation.canonical_bytes > 0
            );
            if let Some(old) = previous.get(label) {
                assert_eq!(old, &observation);
            } else {
                previous.insert(label.to_owned(), observation);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_ordinary_source_retains_its_original_non_nominal_profile() {
    run_actual_sources::<OwnedObservation>(
        &[
            ("ordinary", "let _ = seed.wrapping_add(3);"),
            ("ordinary", "let _ = seed.wrapping_add(3);"),
        ],
        &[(0, 0), (3, 2)],
        SOURCE_OWNED_CHILD,
        "SOURCE_OWNED_V18_ORDINARY",
        |body| {
            format!(
                r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordinary_probe(seed: u32) {{ {body} }}
"#
            )
        },
        |_, _, label, observation, previous| {
            assert!(!observation.nominal_profile);
            assert!(observation.capture);
            assert_eq!(observation.kernels, 1);
            assert_eq!(
                (observation.issues, observation.derives, observation.ends),
                (0, 0, 0)
            );
            assert!(observation.instances >= 1 && observation.canonical_bytes > 0);
            if let Some(old) = previous.get(label) {
                assert_eq!(old, &observation);
            } else {
                previous.insert(label.to_owned(), observation);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_shared_slice_and_compiler_laid_out_roots_capture_abi_without_as0_specialization() {
    run_actual_sources::<OwnedObservation>(
        &[("slice", "let _ = seed;"), ("mixed", "let _ = value;"),
          ("slice", "let _ = seed;")],
        &[(0, 0), (3, 2)], SOURCE_OWNED_CHILD, "SOURCE_OWNED_V18_KERNEL_ABI",
        |body| {
            let signature = if body.contains("value") {
                "input: &[u32], value: (u32, u32)"
            } else { "input: &[u32], seed: u32" };
            format!(r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn kernel_abi_probe({signature}) {{ let _ = input; {body} }}
"#)
        },
        |_, _, label, observation, previous| {
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.abi_arguments, 2);
            assert_eq!(observation.shared_slice_arguments, 1);
            assert_eq!(observation.compiler_laid_out_arguments, usize::from(label == "mixed"));
            assert_eq!(observation.generic_slice_parameters, 0);
            assert_eq!(observation.global_slice_parameters, 1,
                "the authenticated profile must select the physical entry representation");
            assert!(observation.source_as0_slice_types > 0,
                "physical selection must not relabel the original Rust AS0 declaration");
            assert_eq!((observation.issues, observation.derives, observation.ends), (0, 0, 0));
            if let Some(old) = previous.get(label) { assert_eq!(old, &observation); }
            else { previous.insert(label.to_owned(), observation); }
        },
    );
}

#[path = "production_source_descriptor_propagation_v18_tests.rs"]
mod descriptor_propagation_v18_tests;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ScalarOwnedObservation {
    source: [u8; 32],
    graph: [u8; 32],
    checked_stores: usize,
    foreign_owner_checked_stores: usize,
    same_type_mismatch_observed: bool,
}

#[derive(Default)]
struct ScalarOwnedCallbacks {
    result: Option<Result<ScalarOwnedObservation, String>>,
}

impl Callbacks for ScalarOwnedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let original = transaction_in_active_session_v1(
                tcx, crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?.consume_source_owned_v18(|view, budget| {
                let source = *view.source_ssa(budget)?.source_semantic_sha256();
                let graph = *view.canonical(budget)?.identity().digest();
                let count = crate::production_ranked_projection_v1::inspect_actual_source_scalar_consumer_v18(
                    view, budget, 0,
                )?;
                assert!(count > 0);
                budget.reserve_storage(std::mem::size_of::<([u8; 32], [u8; 32], usize)>())?;
                Ok((source, graph, count))
            }).map_err(|error| format!("actual backend scalar positive: {error:?}"))?;
            let foreign = transaction_in_active_session_v1(
                tcx, crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?.consume_source_owned_v18(|view, budget| {
                let count = crate::production_ranked_projection_v1::inspect_actual_source_scalar_consumer_v18(
                    view, budget, 1,
                )?;
                budget.reserve_storage(std::mem::size_of::<usize>())?;
                Ok(count)
            }).map_err(|error| format!("actual backend scalar foreign-source control: {error:?}"))?;
            assert_eq!(foreign, original.2, "both unchanged source transactions must run the same nonempty checker");
            let mismatch_seen = std::cell::Cell::new(false);
            let mismatch = transaction_in_active_session_v1(
                tcx, crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?.consume_source_owned_v18(|view, budget| {
                let result = crate::production_ranked_projection_v1::inspect_actual_source_scalar_consumer_v18(
                    view, budget, 2,
                );
                assert!(matches!(&result, Err(ViewError::Binding(
                    "actual scalar expression differs from its original source value"
                ))));
                mismatch_seen.set(true);
                result
            });
            assert!(mismatch_seen.get(), "the backend positive and actual Store mutation must execute before failure");
            assert!(matches!(mismatch, Err(error) if matches!(*error,
                crate::production_pipeline::ProductionPipelineError::SourceOwnedEntrance(ViewError::Binding(
                    "actual scalar expression differs from its original source value"
                )))));
            Ok(ScalarOwnedObservation {
                source: original.0, graph: original.1, checked_stores: original.2,
                foreign_owner_checked_stores: foreign, same_type_mismatch_observed: mismatch_seen.get(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_scalar_consumer_child() {
    let Some(path) = env::var_os(ARGS) else { return; };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ScalarOwnedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual source scalar rustc callback did not run");
    std::fs::write(env::var_os(RESULT).expect("source scalar report path"), serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(result.is_ok(), "actual source/backend scalar consumer: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_prepared_source_runs_backend_scalar_resolver_and_same_owner_comparison() {
    run_actual_sources::<ScalarOwnedObservation>(
        &[
            ("constant", "let value = 13_u32;"),
            ("arithmetic", "let value = seed.wrapping_add(3);"),
            ("constant", "let value = 13_u32;"),
        ],
        &[(0, 0), (3, 2)], SOURCE_SCALAR_CHILD, "SOURCE_SCALAR_CONSUMER_V18", source,
        |_, _, label, observation, previous| {
            assert!(observation.checked_stores > 0 && observation.same_type_mismatch_observed);
            assert_eq!(observation.checked_stores, observation.foreign_owner_checked_stores);
            if let Some(old) = previous.get(label) { assert_eq!(old, &observation); }
            else { previous.insert(label.to_owned(), observation); }
        },
    );
}
