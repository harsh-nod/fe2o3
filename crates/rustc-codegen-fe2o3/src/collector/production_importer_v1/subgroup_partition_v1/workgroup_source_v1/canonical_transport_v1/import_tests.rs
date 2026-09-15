//! Real registered AMD kernel -> authenticated collection -> canonical MIR V20.
//! No synthetic root custody, SSA owner-equality claim, machine proof or launch.

use super::*;
use crate::collector::production_importer_v1::construct_production_semantic_mir_v1;
use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;
use std::path::PathBuf;

fn request(
    mir: &AdmittedInertSemanticMirV1,
    callables: Vec<SemanticCallableDeclV1>,
) -> Result<InertSemanticMirRequestV1, SemanticMirErrorV1> {
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        mir.functions().to_vec(),
        callables,
        mir.roots().to_vec(),
    )
}

fn shared_edge(
    mir: &AdmittedInertSemanticMirV1,
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
) {
    assert_ne!(reference, owned);
    assert_ne!(
        mir.types()[reference.index() as usize].identity(),
        mir.types()[owned.index() as usize].identity()
    );
    let SemanticTypeShapeV1::Pointer(pointer) = mir.types()[reference.index() as usize].shape()
    else {
        panic!("source borrow must remain a reference");
    };
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
    assert_eq!(pointer.pointee(), owned);
}

struct Probe {
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("workgroup_import_source.rs".into()),
            input: include_str!("source.rs").into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            crate::collector::session_crate_binding(tcx),
            Some(registration_binding_v1()),
            "macro registration must use the actual rustc session binding"
        );
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .expect("actual authenticated AMD target");
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("collect registered Workgroup kernel with original nested getter bodies");
        super::super::context_entry_source_v1::tests::check_collection(tcx, &closure);
        let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let epoch_instances = closure
            .collection
            .functions
            .iter()
            .filter(|function| epoch_provider_v1(tcx, function.instance))
            .map(|function| function.instance)
            .collect::<Vec<_>>();
        let [epoch_instance] = epoch_instances.as_slice() else {
            panic!("retain the exact reviewed getter once, not an epoch terminal");
        };
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap_or_else(|error| {
            panic!("real Workgroup source must complete canonical import: {error}")
        });
        let mir = &imported.semantic_mir;
        assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V20);
        assert_eq!(mir.roots().len(), 1);
        mir.require_complete_external_entries().unwrap();
        let [root] = imported.kernel_contexts.roots.as_ref() else {
            panic!("one authenticated root")
        };
        let provenance = capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap();

        let projections = mir
            .functions()
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                body.workgroup_epoch_projection().map(|record| {
                    (
                        SemanticFunctionIdV1::from_index(index as u32),
                        body,
                        *record,
                    )
                })
            })
            .collect::<Vec<_>>();
        let [(function, body, record)] = projections.as_slice() else {
            panic!("one retained defined epoch record")
        };
        assert_eq!(record.function(), *function);
        assert_eq!(
            record.source_identity(),
            canonical_function_identities_v1(tcx, *epoch_instance).function()
        );
        assert_eq!(record.provenance(), provenance);
        assert_eq!(record.receiver_argument(), 0);
        assert_eq!(record.source_field(), 2);
        assert_ne!(record.body_identity(), &[0; 32]);
        assert_eq!(
            mir.callables()[function.index() as usize],
            SemanticCallableDeclV1::defined(*function)
        );
        assert_eq!(body.locals().len(), 2);
        assert_eq!(body.blocks().len(), 1);
        assert_eq!(body.blocks()[0].statements().len(), 1);
        let source = || {
            workgroup_epoch_projection_source_v1(
                tcx,
                *epoch_instance,
                body.abi(),
                mir.types(),
                root,
                &imported.kernel_contexts,
            )
            .unwrap()
        };
        let observed = source();
        let receiver = observed.receiver();
        assert_eq!(
            record.types().all(),
            [
                receiver.reference(),
                receiver.workgroup(),
                receiver.output(),
                observed.epoch_type()
            ]
        );
        assert_eq!(record.brand(), receiver.brand());
        assert_eq!(record.epoch(), receiver.epoch());
        shared_edge(mir, record.types().reference, record.types().workgroup);
        shared_edge(
            mir,
            record.types().epoch_reference,
            record.types().epoch_type,
        );
        assert_eq!(
            attach_epoch_projection_v1(*function, (**body).clone(), observed).unwrap(),
            **body,
            "attachment must preserve the entire original semantic function"
        );

        for mutation in 0..3 {
            let mut changed = source();
            match mutation {
                0 => changed.receiver.reference = changed.receiver.workgroup,
                1 => changed.epoch_type = changed.receiver.workgroup,
                2 => changed.receiver.brand = changed.receiver.epoch,
                _ => unreachable!(),
            }
            assert!(
                attach_epoch_projection_v1(*function, (**body).clone(), changed).is_err(),
                "accepted source mutation {mutation}"
            );
        }
        assert!(
            attach_epoch_projection_v1(
                *function,
                mir.functions()[root.selected_root.index() as usize].clone(),
                source()
            )
            .is_err(),
            "getter source must not attach to a different function body"
        );

        let borrowed = mir
            .callables()
            .iter()
            .enumerate()
            .filter_map(|(index, callable)| {
                let SemanticCallableDeclV1::CompilerIntrinsic {
                    binding,
                    operation:
                        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                    ..
                } = callable
                else {
                    return None;
                };
                matches!(
                    contract.operation(),
                    SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { .. }
                )
                .then_some((index, binding, *contract))
            })
            .collect::<Vec<_>>();
        let [(callable_index, binding, contract)] = borrowed.as_slice() else {
            panic!("one borrowed subgroup callable")
        };
        let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
            workgroup_reference,
            workgroup,
            subgroup,
            width,
        } = contract.operation()
        else {
            unreachable!()
        };
        assert_eq!(width, 64);
        assert_eq!(workgroup_reference, record.types().reference);
        assert_eq!(workgroup, record.types().workgroup);
        assert_eq!(contract.provenance(), provenance);
        assert_eq!(contract.workgroup_brand(), Some(record.brand()));
        assert_eq!(contract.epoch_before(), Some(record.epoch()));
        assert_eq!(contract.epoch_after(), None);
        assert_eq!(contract.source_identity(), binding.identity());
        assert_eq!(binding.abi().source_input_types(), [workgroup_reference]);
        assert_eq!(binding.abi().source_output_type(), subgroup);
        assert_eq!(
            binding.abi().source_argument_ownership(),
            [SemanticSourceArgumentOwnershipV1::SharedBorrow]
        );
        assert!(matches!(
            binding.abi().return_value().mode(),
            SemanticAbiPassModeV1::Direct(_)
        ));
        shared_edge(mir, workgroup_reference, workgroup);
        assert_eq!(
            mir.types()[subgroup.index() as usize].layout().size_bytes(),
            Some(4)
        );
        assert!(exact_subgroup_source_layout_v1(mir.types(), subgroup));
        let mut issuer = 0;
        let mut partitions = 0;
        for callable in mir.callables() {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: other },
                ..
            } = callable
            else {
                continue;
            };
            match other.operation() {
                SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
                    workgroup: output,
                    ..
                } => {
                    assert_eq!(output, workgroup);
                    assert_eq!(other.provenance(), provenance);
                    issuer += 1;
                }
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(operation) => {
                    assert_eq!(operation.widths(), (64, 16));
                    assert_eq!(other.provenance(), provenance);
                    let arity = match operation {
                        SemanticSubgroupPartitionOperationV1::Derive { .. } => 2,
                        SemanticSubgroupPartitionOperationV1::ReduceSumF32 { .. }
                        | SemanticSubgroupPartitionOperationV1::ReduceMaxF32 { .. } => 2,
                        SemanticSubgroupPartitionOperationV1::BroadcastF32 { .. } => 3,
                    };
                    assert_eq!(other.signature().arguments().count(), arity);
                    partitions += 1;
                }
                SemanticExecutionCapabilityOperationV1::SubgroupDerive { .. } => {
                    panic!("must not coerce borrowed creation into the legacy owned slot")
                }
                _ => {}
            }
        }
        assert_eq!(issuer, 1);
        assert_eq!(partitions, 3);

        // Hostile mutation of the actual imported contract, never a fabricated positive issuer.
        let changed = SemanticExecutionCapabilityContractV1::new(
            SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                workgroup_reference,
                workgroup: workgroup_reference,
                subgroup,
                width,
            },
            contract.signature(),
            contract.provenance(),
            record.brand(),
            record.epoch(),
            None,
            contract.source_identity(),
        )
        .and_then(|changed| {
            let mut callables = mir.callables().to_vec();
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
                &mut callables[*callable_index]
            else {
                unreachable!()
            };
            *operation =
                SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: changed };
            request(mir, callables)?.admit_exact_v20(SemanticMirLimitsV1::default())
        });
        assert!(
            changed.is_err(),
            "reference-to-owned substitution must reject"
        );
        assert!(
            request(mir, mir.callables().to_vec())
                .unwrap()
                .admit_exact_v19(SemanticMirLimitsV1::default())
                .is_err()
        );

        let decoded = AdmittedInertSemanticMirV1::decode_exact_v20_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        let current = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        for copy in [&decoded, &current] {
            assert_eq!(copy.canonical_encoding(), mir.canonical_encoding());
            assert_eq!(copy.functions(), mir.functions());
            assert_eq!(copy.callables(), mir.callables());
        }
        let expansion =
            SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default())
                .unwrap();
        expansion.verify_replay(&decoded).unwrap();
        let occurrences = expansion.workgroup_epoch_projection_bindings(mir).unwrap();
        assert_eq!(
            occurrences.len(),
            2,
            "both nested call occurrences must remain"
        );
        assert_eq!(
            occurrences,
            expansion
                .workgroup_epoch_projection_bindings(&decoded)
                .unwrap()
        );
        assert_ne!(
            occurrences[0].callee_instance(),
            occurrences[1].callee_instance()
        );
        for occurrence in &occurrences {
            assert_eq!(occurrence.projection(), *record);
            assert_eq!(occurrence.expansion_identity(), expansion.identity());
            assert_eq!(occurrence.root(), root.selected_root);
            assert!(matches!(
                occurrence.receiver(),
                SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
            ));
            let expanded = expansion.root(occurrence.root()).unwrap();
            assert!(
                expanded.body().workgroup_epoch_projection().is_none(),
                "a caller must not inherit its callee's identity contract"
            );
            assert_eq!(
                expanded.body().locals()[occurrence.callee_receiver().index() as usize].ty(),
                workgroup_reference
            );
        }
        lowering_tests::check(imported, typed_roots);
        self.completed = true;
        Compilation::Stop
    }
}

#[path = "lowering_tests.rs"]
mod lowering_tests;

#[path = "ordered_max_import_tests.rs"]
mod ordered_max_import_tests;

#[path = "reusable_phase_import_tests.rs"]
mod reusable_phase_import_tests;
#[path = "phase49_callback_tests.rs"]
mod phase49_callback_tests;

const CRATE_NAME: &str = "workgroup_import_source";
const METADATA: &str = "fe2o3-workgroup-import-v20";
const CHILD_ENV: &str = "FE2O3_WORKGROUP_IMPORT_CHILD";

fn registration_binding_v1() -> reserved_fe2o3_symbols::CrateBindingIdV1 {
    reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
}

fn configured_path(name: &str, directory: bool) -> PathBuf {
    let path =
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
            panic!("set {name} to complete cached metadata; no dependency build")
        }));
    assert!(
        if directory {
            path.is_dir()
        } else {
            path.is_file()
        },
        "{name}: {}",
        path.display()
    );
    path.canonicalize().unwrap()
}

fn run(test: &str, callback: impl FnOnce(&[String])) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let device = configured_path("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host_deps = configured_path("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(observation.as_str()) {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-workgroup-import");
        let device_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        // proc_macro_crate resolves the real dependency from this manifest; no Cargo runs.
        std::fs::write(scratch.path().join("Cargo.toml"), format!(
            "[package]\nname = \"workgroup-import-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device_source:?} }}\n"
        )).unwrap();
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD_ENV, &observation)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", scratch.path())
                .env("CARGO_PKG_NAME", "workgroup-import-source")
                .env(
                    reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
                    registration_binding_v1().to_hex(),
                )
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "92929292929292929292929292929292",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "full-import child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        registration_binding_v1().to_hex(),
        "child macro environment must match its exact crate name and metadata"
    );
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        format!("--out-dir={}", std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx950".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host_deps.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-".into(),
    ];
    callback(&args);
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; never runs Cargo"]
fn workgroup_full_import_gfx950_v20() {
    run("collector::production_importer_v1::subgroup_partition_v1::workgroup_source_v1::canonical_transport_v1::import_tests::workgroup_full_import_gfx950_v20", |args| {
        let mut probe = Probe { completed: false };
        rustc_driver::run_compiler(args, &mut probe);
        assert!(
            probe.completed,
            "must complete canonical import and every custody check"
        );
    });
}

#[test]
fn workgroup_registration_uses_session_derived_binding() {
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
    assert_eq!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1("different_workgroup_crate", [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, ["different_workgroup_metadata"])
    );
    assert!(!include_str!("source.rs").contains("namespace"));
}
