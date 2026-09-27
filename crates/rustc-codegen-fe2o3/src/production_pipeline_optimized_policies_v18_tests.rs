//! Real collected Rust reaches the closed production policy continuation.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::optimized_policies_v18_tests::optimized_policy_child";

mod allocation_detail {
    include!("production_pipeline_native_allocation_observation_v18_tests.rs");
}
use allocation_detail::AllocationObservation;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "UnresolvedOperationWire")]
struct UnresolvedOperation {
    function: u32,
    block: u32,
    operation: u32,
    kind: String,
    allocation: Option<AllocationObservation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnresolvedOperationWire {
    function: u32,
    block: u32,
    operation: u32,
    kind: String,
    #[serde(deserialize_with = "deserialize_allocation")]
    allocation: Option<AllocationObservation>,
}

fn deserialize_allocation<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AllocationObservation>, D::Error> {
    Option::<AllocationObservation>::deserialize(deserializer)
}

impl TryFrom<UnresolvedOperationWire> for UnresolvedOperation {
    type Error = String;
    fn try_from(value: UnresolvedOperationWire) -> Result<Self, Self::Error> {
        if value.kind.is_empty() || (value.kind == "Alloca") != value.allocation.is_some() {
            return Err("native unresolved kind/allocation detail mismatch".to_owned());
        }
        if let Some(allocation) = value.allocation {
            allocation.validate_shape().map_err(str::to_owned)?;
        }
        Ok(Self {
            function: value.function,
            block: value.block,
            operation: value.operation,
            kind: value.kind,
            allocation: value.allocation,
        })
    }
}

fn unresolved_operation(
    coordinate: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
    kind: Option<&str>,
    allocation: Option<AllocationObservation>,
) -> Result<Option<UnresolvedOperation>, String> {
    match (coordinate, kind, allocation) {
        (None, None, None) => Ok(None),
        (Some(coordinate), Some(kind), allocation) if !kind.is_empty() => Ok(Some(
            UnresolvedOperation::try_from(UnresolvedOperationWire {
                function: coordinate.block.function.0,
                block: coordinate.block.block,
                operation: coordinate.operation,
                kind: kind.to_owned(),
                allocation,
            })?,
        )),
        _ => Err("native unresolved coordinate/kind observation is incomplete".to_owned()),
    }
}

fn deserialize_unresolved_operation<'de, D>(
    deserializer: D,
) -> Result<Option<UnresolvedOperation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<UnresolvedOperation>::deserialize(deserializer)
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Observation {
    required: String,
    #[serde(deserialize_with = "deserialize_unresolved_operation")]
    first_unresolved_operation: Option<UnresolvedOperation>,
    preparation_refused: bool,
    execution_recipes: usize,
    lifecycle_operations: usize,
    native_completed: bool,
    defined_functions: usize,
    native_resource_cuts: usize,
    work: usize,
    retained: usize,
}

#[derive(Default)]
struct PolicyCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for PolicyCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| "test work limit conversion")?;
            // Capture all original sources before collection can consume MIR.
            // Each transaction keeps its own receipt; no retained capture is cloned.
            let mut transactions = transactions_with_original_sources_for_test_v1::<5>(tcx)?;
            let transaction = transactions
                .next()
                .ok_or("missing preparation-probe source")??;
            let mut denied = OwnedBudget::new(Work::new(work), 0);
            let refused = transaction.inspect_optimized_source_policies_v18(&mut denied);
            assert!(
                matches!(
                    &refused,
                    Err(ProductionPipelineError::SourceOwnedEntrance(
                        fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                                _
                            )
                        )
                    ))
                ),
                "source preparation must refuse before policy: {:?}",
                refused.as_ref().err()
            );
            assert_eq!(denied.work(), 0);
            assert_eq!(denied.storage(), 0);
            assert!(denied.failed_storage().is_some());
            let transaction = transactions
                .next()
                .ok_or("missing typed execution-recipe source")??;
            let mut recipes_account = OwnedBudget::new(
                Work::new(work),
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let recipes = transaction
                .inspect_optimized_execution_recipes_v18(&mut recipes_account)
                .map_err(|error| format!("actual typed lifecycle recipe: {error:?}"))?;
            let execution_recipes = recipes.1;
            assert!(execution_recipes >= 1, "real original context issuance");
            assert_eq!(
                (
                    recipes_account.failed_work(),
                    recipes_account.failed_storage()
                ),
                (None, None)
            );
            let transaction = transactions
                .next()
                .ok_or("missing native-policy source")??;
            let mut account = OwnedBudget::new(
                Work::new(work),
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let (result, probe) =
                transaction.inspect_optimized_source_policies_observed_v18(&mut account);
            let probe = probe.ok_or("actual native candidate was never observed")?;
            assert!(probe.operation_count >= probe.lifecycle && probe.lifecycle > 0);
            let first_unresolved_operation = unresolved_operation(
                probe.first_unresolved.map(|(coordinate, _)| coordinate),
                probe.first_unresolved_kind,
                serde_json::from_value(
                    serde_json::to_value(probe.first_unresolved_allocation)
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            )?;
            use fe2o3_lower_mir_kernel::ProductionSourceNativeLifecycleErrorV18 as NativeError;
            let (required, defined_functions) = match (result, probe.first_unresolved) {
                (
                    Err(ProductionPipelineError::SourceNativeLifecycle(error)),
                    Some((coordinate, requirement)),
                ) => {
                    let NativeError::Unresolved(obligation) = error.as_ref() else {
                        return Err(format!("wrong actual census refusal: {error:?}"));
                    };
                    assert_eq!(obligation.coordinate(), coordinate);
                    assert_eq!(obligation.requirement(), requirement);
                    assert!(error.native_diagnostic().is_none());
                    assert_eq!((probe.native_entries, probe.consumers), (0, 0));
                    (format!("{requirement:?}"), 0)
                }
                (Ok(output), None) => {
                    assert!(!output.0.grants_authority());
                    assert!(output.1.defined_functions > 0);
                    assert_eq!((probe.native_entries, probe.consumers), (1, 1));
                    ("lifecycle-complete".to_owned(), output.1.defined_functions)
                }
                (other, expected) => {
                    return Err(format!(
                        "actual census {expected:?} disagrees with native result: error={:?}",
                        other.as_ref().err()
                    ));
                }
            };
            assert!(account.work() > 0);
            assert_eq!(
                (account.failed_work(), account.failed_storage()),
                (None, None)
            );
            let mut native_resource_cuts = 0;
            for storage_short in [false, true] {
                let transaction = transactions
                    .next()
                    .ok_or("missing actual native resource-cut source")??;
                let mut cut_account = OwnedBudget::new(
                    Work::new(work),
                    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                );
                let (result, cut_probe) = transaction
                    .inspect_optimized_source_policies_resource_cut_v18(
                        &mut cut_account,
                        storage_short,
                    );
                let cut_probe =
                    cut_probe.ok_or("resource control lost the exact actual candidate")?;
                assert_eq!(cut_probe.first_unresolved, probe.first_unresolved);
                assert_eq!(cut_probe.first_unresolved_kind, probe.first_unresolved_kind);
                assert_eq!(
                    cut_probe.first_unresolved_allocation,
                    probe.first_unresolved_allocation
                );
                match (result, cut_probe.first_unresolved) {
                    (Err(ProductionPipelineError::SourceNativeLifecycle(error)), None) => {
                        let NativeError::SourceAfterNative { source, diagnostic } = error.as_ref()
                        else {
                            panic!("native history erased by outer source cleanup: {error:?}");
                        };
                        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
                        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
                        match source {
                            SourceError::Resource(Resource::Storage(limit)) if storage_short => {
                                assert_eq!(
                                    limit.actual(),
                                    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT + 1
                                )
                            }
                            SourceError::Resource(Resource::Work(_)) if !storage_short => (),
                            other => panic!("wrong primary resource refusal: {other:?}"),
                        }
                        assert!(diagnostic.last_invocation().is_some());
                        assert!(diagnostic.observation().work_upper_bound() > 0);
                        assert_eq!((cut_probe.native_entries, cut_probe.consumers), (1, 0));
                        native_resource_cuts += 1;
                    }
                    (
                        Err(ProductionPipelineError::SourceNativeLifecycle(error)),
                        Some((coordinate, requirement)),
                    ) => {
                        let NativeError::Unresolved(obligation) = error.as_ref() else {
                            panic!("{error:?}");
                        };
                        assert_eq!(
                            (obligation.coordinate(), obligation.requirement()),
                            (coordinate, requirement)
                        );
                        assert!(error.native_diagnostic().is_none());
                        assert_eq!((cut_probe.native_entries, cut_probe.consumers), (0, 0));
                        assert_eq!(
                            (cut_account.failed_work(), cut_account.failed_storage()),
                            (None, None)
                        );
                    }
                    (other, _) => panic!(
                        "unexpected actual resource-cut result: error={:?}",
                        other.as_ref().err()
                    ),
                }
            }
            assert!(transactions.next().is_none());
            Ok(Observation {
                required,
                first_unresolved_operation,
                preparation_refused: true,
                execution_recipes,
                lifecycle_operations: probe.lifecycle,
                native_completed: defined_functions > 0,
                defined_functions,
                native_resource_cuts,
                work: account.work(),
                retained: account.storage(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires the actual-source request from its parent"]
fn optimized_policy_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PolicyCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual production policy callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("policy report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual optimized policy: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rust_optimized_output_reaches_fixed_policy_missing_recipe_gate() {
    run_actual_sources::<Observation>(
        &[
            ("plain", "let value = seed;"),
            ("workgroup", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
            (
                "repeated_callers",
                "#[inline(never)] fn shared(value: u32) -> u32 { value.wrapping_add(1) } #[inline(never)] fn left(value: u32) -> u32 { shared(value) } #[inline(never)] fn right(value: u32) -> u32 { shared(value) } let value = left(seed).wrapping_add(right(seed));",
            ),
        ],
        &[(0, 0), (3, 2)],
        CHILD,
        "SOURCE_OPTIMIZED_V18_NATIVE_POLICY",
        source,
        |_, _, label, observation, _| {
            assert_ne!(observation.required, "lifecycle-complete");
            assert!(observation.first_unresolved_operation.is_some());
            assert!(!observation.native_completed);
            assert_eq!(observation.native_resource_cuts, 0);
            assert!(observation.preparation_refused);
            assert!(observation.execution_recipes >= 1);
            if label == "workgroup" {
                assert!(observation.execution_recipes >= 3);
            }
            assert!(observation.work > 0);
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rust_lifecycle_output_runs_native_reports_and_preserves_resource_history() {
    run_actual_sources::<Observation>(
        &[
            ("context_only", "let _ = seed;"),
            ("workgroup_only", "let _ = ctx.with_workgroup(|_wg| seed);"),
        ],
        &[(0, 0), (3, 2)],
        CHILD,
        "SOURCE_OPTIMIZED_V18_NATIVE_LIFECYCLE",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn lifecycle_probe(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
"#
            )
        },
        |_, _, _, observation, _| {
            assert_eq!(observation.required, "lifecycle-complete");
            assert!(observation.first_unresolved_operation.is_none());
            assert!(observation.native_completed && observation.defined_functions > 0);
            assert_eq!(observation.native_resource_cuts, 2);
            assert!(observation.preparation_refused && observation.execution_recipes > 0);
            assert!(observation.lifecycle_operations > 0);
        },
    );
}

#[test]
fn native_unresolved_operation_observation_requires_paired_exact_coordinate() {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Coordinate,
    };
    let coordinate = Coordinate {
        block: Block {
            function: Function(2),
            block: 3,
        },
        operation: 5,
    };
    let observed = unresolved_operation(Some(coordinate), Some("Storage.ReadValue"), None).unwrap();
    assert_eq!(
        observed,
        Some(UnresolvedOperation {
            function: 2,
            block: 3,
            operation: 5,
            kind: "Storage.ReadValue".to_owned(),
            allocation: None,
        })
    );
    assert_eq!(unresolved_operation(None, None, None).unwrap(), None);
    for (coordinate, kind) in [
        (Some(coordinate), None),
        (None, Some("Alloca")),
        (Some(coordinate), Some("")),
    ] {
        assert_eq!(
            unresolved_operation(coordinate, kind, None).unwrap_err(),
            "native unresolved coordinate/kind observation is incomplete"
        );
    }
}

#[test]
fn native_unresolved_operation_observation_json_is_strict_and_lossless() {
    let mut observation = Observation {
        required: "Memory".to_owned(),
        first_unresolved_operation: Some(UnresolvedOperation {
            function: 2,
            block: 3,
            operation: 5,
            kind: "Storage.WriteValue".to_owned(),
            allocation: None,
        }),
        preparation_refused: true,
        execution_recipes: 3,
        lifecycle_operations: 3,
        native_completed: false,
        defined_functions: 0,
        native_resource_cuts: 0,
        work: 101,
        retained: 103,
    };
    let encoded = serde_json::to_value(&observation).unwrap();
    assert_eq!(
        encoded["first_unresolved_operation"],
        serde_json::json!({
            "function": 2, "block": 3, "operation": 5, "kind": "Storage.WriteValue", "allocation": null,
        })
    );
    assert_eq!(
        serde_json::from_value::<Observation>(encoded.clone()).unwrap(),
        observation
    );
    let mut missing = encoded.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("first_unresolved_operation");
    assert!(serde_json::from_value::<Observation>(missing).is_err());
    let mut extra = encoded.clone();
    extra["first_unresolved_operation"]["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Observation>(extra).is_err());
    let mut missing_kind = encoded.clone();
    missing_kind["first_unresolved_operation"]
        .as_object_mut()
        .unwrap()
        .remove("kind");
    assert!(serde_json::from_value::<Observation>(missing_kind).is_err());
    let mut invalid_coordinate = encoded;
    invalid_coordinate["first_unresolved_operation"]["operation"] = serde_json::json!(-1);
    assert!(serde_json::from_value::<Observation>(invalid_coordinate).is_err());
    observation.required = "lifecycle-complete".to_owned();
    observation.first_unresolved_operation = None;
    observation.native_completed = true;
    observation.defined_functions = 1;
    observation.native_resource_cuts = 2;
    let complete = serde_json::to_value(&observation).unwrap();
    assert!(complete["first_unresolved_operation"].is_null());
    assert_eq!(
        serde_json::from_value::<Observation>(complete).unwrap(),
        observation
    );
}

#[test]
fn native_allocation_observation_json_requires_complete_paired_details() {
    use allocation_detail::*;
    let ty = TypeObservation {
        kind: TypeTag::StorageObject,
        scalar: None,
        layout: Some(LayoutObservation {
            ordinal: 7,
            kind: LayoutTag::Scalar,
            size: 4,
            alignment: 4,
            scalar: Some(ScalarTag::U32),
            members: 0,
        }),
    };
    let allocation = AllocationObservation {
        element: ty,
        address_space: SpaceTag::Private,
        alignment: 4,
        count: None,
        result: 19,
        result_address_space: SpaceTag::Private,
        result_access: AccessTag::ReadWrite,
        result_pointee: ty,
        source: Some(AllocationSourceObservation {
            root: 0,
            instance: 2,
            function: 3,
            input: [0, 4, 5],
            semantic_sha256: [11; 32],
            private_slot_identity_available: false,
        }),
    };
    let row = UnresolvedOperation {
        function: 0,
        block: 1,
        operation: 2,
        kind: "Alloca".to_owned(),
        allocation: Some(allocation),
    };
    let encoded = serde_json::to_value(&row).unwrap();
    assert_eq!(
        serde_json::from_value::<UnresolvedOperation>(encoded.clone()).unwrap(),
        row
    );
    for path in [
        "allocation",
        "allocation.element.layout",
        "allocation.source",
        "allocation.count",
    ] {
        let mut missing = encoded.clone();
        let mut names = path.split('.').peekable();
        let mut parent = &mut missing;
        while let Some(name) = names.next() {
            if names.peek().is_none() {
                parent.as_object_mut().unwrap().remove(name);
                break;
            }
            parent = &mut parent[name];
        }
        assert!(
            serde_json::from_value::<UnresolvedOperation>(missing).is_err(),
            "missing {path}"
        );
    }
    for field in [
        "kind",
        "allocation",
        "extra",
        "bad_scalar",
        "bad_layout",
        "bad_slot",
        "bad_enum",
    ] {
        let mut wrong = encoded.clone();
        match field {
            "kind" => wrong["kind"] = serde_json::json!("Storage.ReadValue"),
            "allocation" => wrong["allocation"] = serde_json::Value::Null,
            "extra" => wrong["allocation"]["source"]["unexpected"] = serde_json::json!(true),
            "bad_scalar" => wrong["allocation"]["element"]["scalar"] = serde_json::json!("U32"),
            "bad_layout" => {
                wrong["allocation"]["element"]["layout"]["scalar"] = serde_json::Value::Null
            }
            "bad_slot" => {
                wrong["allocation"]["source"]["private_slot_identity_available"] =
                    serde_json::json!(true)
            }
            "bad_enum" => wrong["allocation"]["address_space"] = serde_json::json!("Unknown"),
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<UnresolvedOperation>(wrong).is_err(),
            "wrong {field}"
        );
    }
    let mut absent_source = encoded.clone();
    absent_source["allocation"]["source"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<UnresolvedOperation>(absent_source)
            .unwrap()
            .allocation
            .unwrap()
            .source
            .is_none()
    );
    let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            block: 1,
        },
        operation: 2,
    };
    assert!(unresolved_operation(Some(coordinate), Some("Alloca"), None).is_err());
    assert!(
        unresolved_operation(
            Some(coordinate),
            Some("Storage.ReadValue"),
            Some(allocation)
        )
        .is_err()
    );
    assert!(unresolved_operation(None, None, Some(allocation)).is_err());
}
