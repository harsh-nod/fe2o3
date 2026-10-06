//! Actual Rust object-array numeric producers after original owning completion.
use super::*;
use fe2o3_kernel_ir::{Constant, OperationKind, StorageOperationV1, StorageProjectionV1, Type};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticOperandV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1, SemanticTypeShapeV1,
};
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::array_index_tests::array_index_source_child";
const FINAL_REFUSAL: &str = "original source integer final admission required";

fn program(length: &str) -> String {
    let length: usize = length.parse().unwrap();
    assert!([1, 3, 5].contains(&length));
    let values = vec!["pointer"; length].join(", ");
    format!(
        r#"use fe2o3_device::{{kernel, DeviceGlobalMutPtr}};
#[inline(never)]
fn keep_array(_values: &[*mut u32; {length}]) {{}}
#[inline(never)]
fn keep_pointer(_pointer: *mut u32) {{}}
#[inline(never)]
fn transport(pointer: *mut u32) {{
    let values = [{values}];
    keep_array(&values);
    let [.., tail] = values;
    keep_pointer(tail);
}}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn array_transport(pointer: DeviceGlobalMutPtr<u32>, _nominal: usize) {{
    let raw = pointer.as_raw();
    transport(raw);
    transport(raw);
}}
"#,
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    version: u16,
    length: u64,
    array_instances: usize,
    source_components: usize,
    source_constant_reads: usize,
    retained_array_projects: usize,
    physical_indices: Vec<u64>,
    owning_completion: bool,
    materialized: bool,
    foreign_owner_refused: bool,
    foreign_ledger_refused: bool,
    final_refused: bool,
}

#[derive(Default)]
struct ArrayCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for ArrayCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("independent original array source: {error:?}"))?;
            start_preparation_observation_v29();
            let completed = Cell::new(false);
            let continuation = transaction()?.with_original_source_integer_custody_v18(
                |source, handoff, roots, _, budget| {
                    // This callback is after owning admission, independent source
                    // reconstruction, consumer replay, and integer adoption.
                    // Nonempty rows below therefore cannot be a preflight-only
                    // observation of a candidate that never completed its root.
                    let semantic = source.source_semantic(budget)?;
                    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V35);
                    assert_eq!(source.root_count(budget)?, 1);
                    let (root, _) = source.root(0, budget)?;
                    let inputs = semantic.functions()[root.index() as usize].abi().source_input_types();
                    assert_eq!(inputs.len(), 2);
                    assert_eq!(semantic.types()[inputs[1].index() as usize].rust_type_kind(), SemanticRustTypeKindV1::Usize);
                    assert!(matches!(roots[0].arguments[1].kind, AbiKind::Descriptor {
                        source: SourceTypeDescriptorV3::Usize, ..
                    }));
                    source.require_kernel_argument_abi_v18(AbiInput { roots }, budget)?;
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                    assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                    let mut length = None;
                    let mut array_functions = Vec::new();
                    let mut source_components = 0;
                    let mut source_constant_reads = 0;
                    let mut retained_array_projects = 0;
                    for instance in 0..source.instance_count(0, budget)? {
                        if !source.instance_active(0, instance, budget)? {
                            continue;
                        }
                        let (function, _) = source.instance(0, instance, budget)?;
                        let declaration = &semantic.functions()[function.index() as usize];
                        let mut array_local = None;
                        for block in declaration.blocks() {
                            for statement in block.statements() {
                                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                                    continue;
                                };
                                let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                                    continue;
                                };
                                if aggregate.kind() != &SemanticAggregateKindV1::Array {
                                    continue;
                                }
                                let SemanticTypeShapeV1::Array { element, length: actual } =
                                    semantic.types()[assignment.value().result_type().index() as usize].shape()
                                else { panic!("original array aggregate type"); };
                                let SemanticTypeShapeV1::Pointer(pointer) = semantic.types()[element.index() as usize].shape() else {
                                    panic!("fixture must retain raw-pointer array, not scalar-array lowering");
                                };
                                assert_eq!(pointer.kind(), SemanticPointerKindV1::Raw);
                                assert_eq!(pointer.metadata(), SemanticPointerMetadataV1::None);
                                assert_eq!(pointer.pointer_width_bits(), 64);
                                assert!(assignment.destination().projections().is_empty());
                                assert_eq!(aggregate.operands().len(), *actual as usize);
                                assert!(array_local.replace(assignment.destination().local()).is_none());
                                if let Some(previous) = length.replace(*actual) {
                                    assert_eq!(previous, *actual);
                                }
                                source_components += aggregate.operands().len();
                            }
                        }
                        if let Some(local) = array_local {
                            array_functions.push(function);
                            for block in declaration.blocks() {
                                for statement in block.statements() {
                                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                                        continue;
                                    };
                                    let place = match assignment.value().kind() {
                                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => place,
                                        SemanticRvalueKindV1::Load(load) => load.source(),
                                        _ => continue,
                                    };
                                    if place.local() != local || place.projections().is_empty() {
                                        continue;
                                    }
                                    let [projection] = place.projections() else {
                                        panic!("fixture must use one original constant-index projection");
                                    };
                                    let SemanticProjectionKindV1::ConstantIndex { offset, minimum_length, from_end } = projection.kind() else {
                                        panic!("fixture original array-pattern read is not constant-indexed");
                                    };
                                    let index = if from_end {
                                        length.unwrap().checked_sub(offset).unwrap()
                                    } else {
                                        offset
                                    };
                                    assert_eq!(index + 1, length.unwrap());
                                    assert!(minimum_length > 0 && minimum_length <= length.unwrap());
                                    source_constant_reads += 1;
                                }
                            }
                        }
                        for ordinal in 0..source.memory_anchor_count(0, instance, budget)? {
                            if let Some((_, _, StorageOperationV1::Project { step: StorageProjectionV1::ArrayIndex(_), .. }, _)) =
                                source.memory_object(0, instance, ordinal, budget)?
                            {
                                retained_array_projects += 1;
                            }
                        }
                    }
                    let length = length.expect("actual Rust array aggregate survived capture");
                    assert_eq!(array_functions.len(), 2);
                    assert_eq!(array_functions[0], array_functions[1], "two real activations of the same helper");
                    assert_eq!(source_components, 2 * length as usize);
                    assert_eq!(source_constant_reads, 2);
                    assert_eq!(retained_array_projects, source_components + source_constant_reads);

                    let original = source.canonical(budget)?;
                    let (_, root_function) = source.root(0, budget)?;
                    let body = original.module().functions[root_function].body.as_ref().unwrap();
                    let mut physical_indices = Vec::new();
                    for block in &body.blocks {
                        for (ordinal, operation) in block.operations.iter().enumerate() {
                            let OperationKind::Storage(StorageOperationV1::Project { step: StorageProjectionV1::ArrayIndex(index), .. }) = operation.kind else {
                                continue;
                            };
                            let producer = &block.operations[ordinal.checked_sub(1).expect("index producer before project")];
                            let OperationKind::Constant(Constant::Index(value)) = producer.kind else {
                                panic!("actual array index must have the exact Index producer");
                            };
                            assert_eq!(producer.results.len(), 1);
                            assert_eq!(producer.results[0].id, index);
                            assert_eq!(producer.results[0].ty, Type::INDEX);
                            assert!(value < length);
                            physical_indices.push(value);
                        }
                    }
                    physical_indices.sort_unstable();
                    let mut expected: Vec<_> = (0..2)
                        .flat_map(|_| (0..length).chain(std::iter::once(length - 1)))
                        .collect();
                    expected.sort_unstable();
                    assert_eq!(physical_indices, expected);
                    assert_eq!(physical_indices.len(), retained_array_projects);
                    let output = handoff.output(budget)?;
                    assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                    let report = Observation {
                        source: *source.source_ssa(budget)?.source_semantic_sha256(),
                        original: Sha256::digest(original.canonical_bytes()).into(),
                        output: Sha256::digest(output.owner().canonical_bytes()).into(),
                        version: semantic.wire_version().as_u16(), length,
                        array_instances: array_functions.len(), source_components,
                        source_constant_reads, retained_array_projects, physical_indices,
                        owning_completion: true, materialized: false,
                        foreign_owner_refused: false, foreign_ledger_refused: false,
                        final_refused: false,
                    };
                    completed.set(true);
                    Ok(report)
                },
            ).map_err(|error| format!("actual array original-source owning completion: {error:?}"))?;
            assert!(
                completed.get(),
                "caught callback panic cannot satisfy completion"
            );
            assert!(
                take_preparation_observation_v29()
                    .expect("array preparation observed")
                    .materialized
            );
            let mut report = continuation.into_observation();
            report.materialized = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |source, _, _, _, budget| {
                        let error = source.check_original_source(&foreign, budget).unwrap_err();
                        assert!(matches!(
                            error,
                            SourceError::Binding("foreign original SSA owner")
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding("foreign original SSA owner"))
            ));
            report.foreign_owner_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |_, handoff, _, _, _| {
                        let mut work =
                            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                        let foreign_budget = Budget::new(&mut work, 20_000_000);
                        let error = match handoff.output(&foreign_budget) {
                            Err(error) => error,
                            Ok(_) => panic!("foreign ledger acquired array output"),
                        };
                        assert!(matches!(
                            error,
                            SourceError::Resource(ResourceError::Accounting)
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            report.foreign_ledger_refused = true;

            start_preparation_observation_v29();
            let error = transaction()?
                .original_source_integer_finalizer_refusal_v18()
                .unwrap_err();
            assert!(
                take_preparation_observation_v29()
                    .expect("array finalizer preparation observed")
                    .materialized
            );
            assert!(matches!(error, Error::Unsupported(FINAL_REFUSAL)));
            report.final_refused = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn array_index_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ArrayCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("array-index compiler callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("array-index result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual array-index source: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_original_source_array_indices_complete_before_integer_final_refusal() {
    run_actual_sources::<Observation>(
        &[("array1", "1"), ("array3", "3"), ("array5", "5")],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_ARRAY_INDEX_SOURCE_V18",
        program,
        |opt, mir, label, report, _| {
            assert_eq!((opt, mir), (0, 0));
            let length = match label {
                "array1" => 1,
                "array3" => 3,
                "array5" => 5,
                _ => unreachable!(),
            };
            assert_eq!(report.version, 35);
            assert_eq!(report.length, length);
            assert_eq!(report.array_instances, 2);
            assert_eq!(report.source_components, 2 * length as usize);
            assert_eq!(report.source_constant_reads, 2);
            assert_eq!(report.retained_array_projects, 2 * (length as usize + 1));
            assert_eq!(
                report.physical_indices.len(),
                report.retained_array_projects
            );
            assert!(report.owning_completion && report.materialized);
            assert!(
                report.foreign_owner_refused
                    && report.foreign_ledger_refused
                    && report.final_refused
            );
        },
    );
}

#[test]
fn array_index_actual_source_fixture_keeps_safe_opaque_transport_and_distinct_child() {
    assert!(
        CHILD.ends_with("::original_source_tests::array_index_tests::array_index_source_child")
    );
    assert_ne!(CHILD, ORIGINAL_CHILD);
    assert_ne!(CHILD, MIXED_CHILD);
    for length in ["1", "3", "5"] {
        let source = program(length);
        assert_eq!(source.matches("transport(raw);").count(), 2);
        assert_eq!(source.matches("keep_array(&values);").count(), 1);
        assert_eq!(source.matches("let [.., tail] = values;").count(), 1);
        assert_eq!(source.matches("keep_pointer(tail);").count(), 1);
        assert!(source.contains("pointer.as_raw()"));
        assert!(source.contains("_nominal: usize"));
        assert!(!source.contains("unsafe"));
        assert!(!source.contains("black_box"));
    }
}
