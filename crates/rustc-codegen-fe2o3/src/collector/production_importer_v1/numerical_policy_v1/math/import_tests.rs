//! Opt-in real AMD source -> authenticated collection -> canonical MIR V21.
//! Separate SSA callbacks check bridge results; neither boundary is launch proof.

use super::super::super::{
    build_identity_inventory_v1, canonical_target_layout_v1, construct_production_semantic_mir_v1,
    validate_execution_terminal_carriage_v1,
};
use super::validate_policy_math_source_v1;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use crate::trusted_device_items::{self, TrustedDeviceItem};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::ty::TyCtxt;
use rustc_session::config::Input;
use rustc_span::FileName;
use std::collections::BTreeSet;
use std::path::PathBuf;

mod defined_sources;
mod ssa_results;
mod lowering;
mod ordered_context;
mod tiled_context;
mod gfx950_transpose;
mod combined_v24;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, StrictIeee, kernel};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn policy_math_all13(context: KernelContext<'_>, x: f32, y: f32, z: f32) {
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let _values = [
        math.sqrt_f32(x),
        math.mul_add_f32(x, y, z),
        math.floor_f32(x),
        math.ceil_f32(x),
        math.trunc_f32(x),
        math.round_ties_even_f32(x),
        math.sin_f32(x),
        math.cos_f32(x),
        math.exp_f32(x),
        math.exp2_f32(x),
        math.ln_f32(x),
        math.log2_f32(x),
        math.log10_f32(x),
    ];
}
"#;

const FUNCTIONS: [SemanticF32MathFunctionV1; 13] = [
    SemanticF32MathFunctionV1::Sqrt,
    SemanticF32MathFunctionV1::FusedMultiplyAdd,
    SemanticF32MathFunctionV1::Floor,
    SemanticF32MathFunctionV1::Ceil,
    SemanticF32MathFunctionV1::Truncate,
    SemanticF32MathFunctionV1::RoundTiesEven,
    SemanticF32MathFunctionV1::Sin,
    SemanticF32MathFunctionV1::Cos,
    SemanticF32MathFunctionV1::Exp,
    SemanticF32MathFunctionV1::Exp2,
    SemanticF32MathFunctionV1::Ln,
    SemanticF32MathFunctionV1::Log2,
    SemanticF32MathFunctionV1::Log10,
];

fn with_contract(
    original: SemanticNumericalPolicyMathContractV1,
    types: SemanticNumericalPolicyMathTypesV1,
    policy: SemanticTypeIdentityV1,
    brand: SemanticTypeIdentityV1,
    function: SemanticF32MathFunctionV1,
) -> SemanticCompilerIntrinsicOperationV1 {
    SemanticCompilerIntrinsicOperationV1::PolicyMathF32 {
        contract: SemanticNumericalPolicyMathContractV1::new(
            types,
            policy,
            brand,
            function,
            SemanticNumericalModeV1::StrictIeee,
            function.required_implementation(),
            original.provenance(),
            original.source_identity(),
        )
        .expect("mutation remains a constructible inert contract"),
    }
}

fn admit_changed(
    original: &AdmittedInertSemanticMirV1,
    change: impl FnMut(usize, &mut SemanticCompilerIntrinsicOperationV1),
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    admit_changed_with_functions(original, original.functions().to_vec(), change)
}

fn admit_changed_with_functions(
    original: &AdmittedInertSemanticMirV1,
    functions: Vec<SemanticFunctionDeclV1>,
    mut change: impl FnMut(usize, &mut SemanticCompilerIntrinsicOperationV1),
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let mut callables = original.callables().to_vec();
    for (index, callable) in callables.iter_mut().enumerate() {
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
            change(index, operation);
        }
    }
    InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        original.allocations().to_vec(),
        original.statics().to_vec(),
        original.vtables().to_vec(),
        functions,
        callables,
        original.roots().to_vec(),
    )?
    .admit_exact_v21(SemanticMirLimitsV1::default())
}

fn check_consumer_mutations(
    mir: &AdmittedInertSemanticMirV1,
    callable_index: usize,
    contract: SemanticNumericalPolicyMathContractV1,
) {
    let function = contract.function();
    // MIR V19 tag 79 has seven u32 type edges, two nominal identities,
    // function, mode, then implementation. Match its unique typed prefix,
    // rather than relying on whole-document byte offsets or textual hashes.
    let mut prefix = vec![79];
    for id in contract.types().all() {
        prefix.extend(id.index().to_le_bytes());
    }
    prefix.extend(contract.policy().as_bytes());
    prefix.extend(contract.kernel_brand().as_bytes());
    prefix.push(FUNCTIONS.iter().position(|f| *f == function).unwrap() as u8);
    let offsets = mir
        .canonical_encoding()
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == prefix).then_some(offset + prefix.len()))
        .collect::<Vec<_>>();
    let [mode_offset] = offsets.as_slice() else {
        panic!("{function:?}: expected one complete V19 consumer record, got {offsets:?}");
    };
    let implementation = match function.required_implementation() {
        SemanticF32MathImplementationV1::ConstrainedLlvm => 0,
        SemanticF32MathImplementationV1::OcmlAbiV1 => 1,
        SemanticF32MathImplementationV1::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1 => 2,
    };
    assert_eq!(mir.canonical_encoding()[*mode_offset], 0);
    assert_eq!(mir.canonical_encoding()[*mode_offset + 1], implementation);
    for (label, offset, value) in [
        ("unknown mode", *mode_offset, 1),
        (
            "wrong implementation",
            *mode_offset + 1,
            (implementation + 1) % 3,
        ),
        ("unknown implementation", *mode_offset + 1, 255),
    ] {
        let mut bytes = mir.canonical_encoding().to_vec();
        bytes[offset] = value;
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                &bytes,
                SemanticMirLimitsV1::default(),
            )
            .is_err(),
            "{function:?}: {label} must not acquire a default numerical contract",
        );
    }

    for arity in [false, true] {
        let result = admit_changed(mir, |index, operation| {
            if index != callable_index {
                return;
            }
            let mut types = contract.types();
            let changed_function = if arity {
                if function == SemanticF32MathFunctionV1::FusedMultiplyAdd {
                    SemanticF32MathFunctionV1::Sqrt
                } else {
                    SemanticF32MathFunctionV1::FusedMultiplyAdd
                }
            } else {
                std::mem::swap(&mut types.math_reference, &mut types.policy_reference);
                function
            };
            *operation = with_contract(
                contract,
                types,
                contract.policy(),
                contract.kernel_brand(),
                changed_function,
            );
        });
        assert!(
            result.is_err(),
            "{function:?}: {} substitution must fail against the actual imported type graph/ABI",
            if arity { "arity" } else { "reference" },
        );
    }
}

#[derive(Clone, Copy)]
enum SourceCase {
    Math,
    OrderedContext,
    TiledContext,
    TransposeFp4,
    TransposeFp8,
    TransposeOwned(bool),
    CombinedV24(combined_v24::Case),
}

struct Probe {
    cpu: &'static str,
    completed: bool,
    check_ssa: bool,
    check_kir: bool,
    source: SourceCase,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = match self.source {
            SourceCase::CombinedV24(case) => Input::Str {
                name: FileName::Custom("combined_v24_source.rs".into()),
                input: combined_v24::source(case),
            },
            SourceCase::TiledContext => tiled_context::input(),
            SourceCase::TransposeOwned(fp8) => Input::Str {
                name: FileName::Custom("gfx950_transpose_owned_source.rs".into()),
                input: gfx950_transpose::source(fp8),
            },
            SourceCase::TransposeFp4 | SourceCase::TransposeFp8 => Input::Str {
                name: FileName::Custom("gfx950_transpose_source.rs".into()),
                input: gfx950_transpose::source(matches!(self.source, SourceCase::TransposeFp8)),
            },
            source => Input::Str {
                name: FileName::Custom("policy_math_import_source.rs".into()),
                input: match source {
                    SourceCase::Math => SOURCE,
                    SourceCase::OrderedContext => ordered_context::SOURCE,
                    SourceCase::TiledContext | SourceCase::TransposeFp4 | SourceCase::TransposeFp8 | SourceCase::TransposeOwned(_) | SourceCase::CombinedV24(_) => unreachable!(),
                }.into(),
            },
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;

        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .expect("actual production AMD target, not a host layout labelled AMD");
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("collect the registered kernel and original policy binding constructor");
        match self.source {
            SourceCase::OrderedContext => {
                ordered_context::check(tcx, closure);
                self.completed = true;
                return Compilation::Stop;
            }
            SourceCase::TiledContext => {
                tiled_context::check(tcx, closure);
                self.completed = true;
                return Compilation::Stop;
            }
            SourceCase::TransposeOwned(fp8) => {
                gfx950_transpose::check_owned_source(tcx, closure, fp8);
                self.completed = true;
                return Compilation::Stop;
            }
            SourceCase::TransposeFp4 | SourceCase::TransposeFp8 => {
                gfx950_transpose::check(tcx, closure, matches!(self.source, SourceCase::TransposeFp8));
                self.completed = true;
                return Compilation::Stop;
            }
            SourceCase::CombinedV24(case) => {
                combined_v24::check(tcx, closure, case);
                self.completed = true;
                return Compilation::Stop;
            }
            SourceCase::Math => {}
        }

        // Retain a second observation of the real preflight plan for negative
        // carriage checks. The production importer still consumes the original
        // move-only closure and authenticates its context issuance itself.
        let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        assert_eq!(observed_target.contract().cpu(), self.cpu);
        let inventory =
            build_identity_inventory_v1(tcx, &observed_target, &closure.collection, &closure.roots)
                .unwrap();
        assert!(
            closure
                .collection
                .functions
                .iter()
                .all(|f| f.closure_plan.is_none())
        );
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed_target.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &BTreeSet::new(),
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("preflight the actual collected source");
        let typed_roots = self.check_kir.then(|| closure.rederive_typed_descriptor_roots(tcx).unwrap());
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap_or_else(|error| {
            for (index, terminal) in plan.terminal_producers().iter().enumerate() {
                eprintln!(
                    "policy import terminal {index}: {:?} {} args={:?} output={:?}",
                    terminal.expansion,
                    tcx.def_path_str(terminal.instance.def_id()),
                    terminal.abi.source_inputs,
                    terminal.abi.source_output,
                );
            }
            panic!("all 13 operations must complete canonical import before SSA: {error}");
        });
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript(),
            "mutation checks must use the same source plan as production import",
        );
        let mir = &imported.semantic_mir;
        assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V21);
        assert_eq!(mir.roots().len(), 1);
        mir.require_complete_external_entries().unwrap();
        for decoded in [
            AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                mir.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            ),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                mir.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            ),
        ] {
            let decoded = decoded.expect("actual V21 import must round-trip canonically");
            assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
            assert_eq!(decoded.functions(), mir.functions());
            assert_eq!(decoded.callables(), mir.callables());
            validate_execution_terminal_carriage_v1(
                tcx,
                &plan,
                &imported.kernel_contexts,
                &decoded,
            )
            .expect("decoded type IDs and nominal identities retain exact source custody");
        }

        defined_sources::check(tcx, &plan, &imported);
        if self.check_ssa {
            ssa_results::check(mir);
        }

        let mut seen = BTreeSet::new();
        for (terminal_index, terminal) in plan.terminal_producers().iter().enumerate() {
            let ProductionTerminalExpansionV1::PolicyMathF32(function) = terminal.expansion else {
                continue;
            };
            let index = plan.function_producers().len() + terminal_index;
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
                ..
            } = &mir.callables()[index]
            else {
                panic!("{function:?}: policy authority erased during import");
            };
            assert!(seen.insert(contract.function()), "duplicate consumer");
            let source = validate_policy_math_source_v1(tcx, terminal.instance, function).unwrap();
            assert_eq!(
                contract.policy(),
                rustc_type_identity_v1(tcx, source.policy)
            );
            assert_eq!(
                contract.kernel_brand(),
                rustc_type_identity_v1(tcx, source.kernel_brand)
            );
            for (id, ty) in contract.types().all().into_iter().zip(source.types) {
                assert_eq!(
                    mir.types()[id.index() as usize].identity(),
                    rustc_type_identity_v1(tcx, ty)
                );
            }
            let abi = binding.abi();
            let arity = function.arity() + 1;
            assert_eq!(abi.source_input_types().len(), arity);
            assert_eq!(abi.arguments().len(), arity);
            assert_eq!(abi.fixed_count() as usize, arity);
            assert_eq!(
                abi.source_argument_ownership()[0],
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            );
            assert!(
                abi.source_argument_ownership()[1..]
                    .iter()
                    .all(|v| *v == SemanticSourceArgumentOwnershipV1::ByValue)
            );
            assert!(
                abi.arguments()
                    .iter()
                    .all(|v| matches!(v.value().mode(), SemanticAbiPassModeV1::Direct(_)))
            );
            assert!(matches!(
                abi.return_value().mode(),
                SemanticAbiPassModeV1::Direct(_)
            ));
            assert_eq!(
                abi.source_input_types()[0],
                contract.types().bound_reference
            );
            assert!(
                abi.source_input_types()[1..]
                    .iter()
                    .all(|ty| *ty == contract.types().element)
            );
            assert_eq!(abi.source_output_type(), contract.types().element);
            assert_eq!(
                contract.numerical_requirements(),
                (
                    SemanticNumericalModeV1::StrictIeee,
                    contract.function().required_implementation()
                )
            );
            assert_eq!(
                contract.obligations().bits(),
                SemanticExecutionSafetyObligationsV1::TARGET_SUPPORT
                    | SemanticExecutionSafetyObligationsV1::NUMERICAL_POLICY
            );
            let calls = mir
                .functions()
                .iter()
                .flat_map(|f| f.blocks())
                .filter_map(|block| {
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return None;
                    };
                    (call.callee().index() as usize == index).then_some(call)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                1,
                "{function:?}: retain a real source call, not an unused callable"
            );
            assert_eq!(calls[0].arguments().len(), arity);
            check_consumer_mutations(mir, index, *contract);
        }
        assert_eq!(seen, FUNCTIONS.into_iter().collect());
        assert!(
            !mir.callables().iter().any(|callable| matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::MathF32 { .. },
                    ..
                }
            )),
            "no policy consumer may fall back to bare math"
        );
        let binds = plan
            .function_producers()
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                trusted_device_items::classify(tcx, f.instance.def_id())
                    == Some(TrustedDeviceItem::PolicyMathBind)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            binds.len(),
            1,
            "retain the constructor as an ordinary defined function"
        );
        let (index, bind) = binds[0];
        assert!(std::ptr::eq(
            plan.function_mir(SemanticFunctionIdV1::from_index(index as u32))
                .unwrap(),
            tcx.instance_mir(bind.instance.def)
        ));
        assert!(
            matches!(mir.callables()[index], SemanticCallableDeclV1::Defined { function } if function.index() as usize == index)
        );

        // Keep all claims internally consistent. Inert admission must succeed;
        // only replay against the authenticated source may reject these changes.
        for policy_change in [false, true] {
            let replacement = SemanticTypeIdentityV1::from_sha256([0xe7; 32]);
            assert!(mir.types().iter().all(|ty| ty.identity() != replacement));
            let mut consumers = 0;
            let mut issuances = 0;
            let changed_functions = defined_sources::with_nominals(mir, policy_change, replacement);
            let changed = admit_changed_with_functions(mir, changed_functions, |_, operation| {
                match *operation {
                    SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract } => {
                        assert_ne!(contract.policy(), replacement);
                        assert_ne!(contract.kernel_brand(), replacement);
                        consumers += 1;
                        *operation = with_contract(
                            contract,
                            contract.types(),
                            if policy_change {
                                replacement
                            } else {
                                contract.policy()
                            },
                            if policy_change {
                                contract.kernel_brand()
                            } else {
                                replacement
                            },
                            contract.function(),
                        );
                    }
                    SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }
                        if policy_change =>
                    {
                        if let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                            context,
                            capability,
                            ..
                        } = contract.operation()
                        {
                            issuances += 1;
                            *operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
                            contract: SemanticExecutionCapabilityContractV1::new_kernel_scoped(
                                SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                                    context,
                                    capability,
                                    policy: replacement,
                                },
                                contract.signature(),
                                contract.provenance(),
                                contract.source_identity(),
                            )
                            .unwrap(),
                        };
                        }
                    }
                    _ => {}
                }
            })
            .expect("consistent nominal substitution is inert, not source-authenticated");
            assert_eq!(consumers, 13);
            assert_eq!(issuances, usize::from(policy_change));
            let decoded = AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                changed.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            )
            .unwrap();
            defined_sources::reject_changed_attachment(tcx, &plan, &imported, &decoded);
            let error = validate_execution_terminal_carriage_v1(
                tcx,
                &plan,
                &imported.kernel_contexts,
                &decoded,
            )
            .expect_err("a self-consistent replacement identity must fail full source carriage")
            .to_string();
            assert!(
                error.contains("policy FP32 complete source contract carriage"),
                "{error}"
            );
        }
        if let Some(typed_roots) = typed_roots {
            lowering::check(imported, typed_roots);
        }
        self.completed = true;
        Compilation::Stop
    }
}

const CRATE_NAME: &str = "policy_math_import_source";
const METADATA: &str = "fe2o3-policy-math-import-v19";
const CHILD_ENV: &str = "FE2O3_POLICY_MATH_IMPORT_CHILD";

fn registration_binding_v1() -> reserved_fe2o3_symbols::CrateBindingIdV1 {
    reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-policy-math-import-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create exclusively owned test scratch directory");
        let scratch = Self(path);
        // proc_macro_crate reads this manifest to resolve the real device import.
        // No Cargo process or dependency build is run.
        let device = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        std::fs::write(scratch.0.join("Cargo.toml"), format!(
            "[package]\nname = \"policy-math-import-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n"
        )).unwrap();
        scratch
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn configured_path(name: &str, directory: bool) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
        panic!("set {name} to complete cached metadata; this test never builds dependencies")
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
    path.canonicalize()
        .expect("canonical cached dependency path")
}

fn run(cpu: &'static str, test_name: &str) {
    run_with_ssa(cpu, test_name, false);
}

fn run_with_ssa(cpu: &'static str, test_name: &str, check_ssa: bool) {
    run_with_lowering(cpu, test_name, check_ssa, false);
}

fn run_with_lowering(cpu: &'static str, test_name: &str, check_ssa: bool, check_kir: bool) {
    run_source(cpu, test_name, check_ssa, check_kir, false);
}

fn run_source(cpu: &'static str, test_name: &str, check_ssa: bool, check_kir: bool, ordered_context: bool) {
    let source = if ordered_context { SourceCase::OrderedContext } else { SourceCase::Math };
    run_registered_source(cpu, test_name, check_ssa, check_kir, source);
}

fn run_registered_source(cpu: &'static str, test_name: &str, check_ssa: bool, check_kir: bool, source: SourceCase) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };

    let device = configured_path("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host_deps = configured_path("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let child_key = format!("{cpu}:{observation}");
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(child_key.as_str()) {
        let test_name = format!(
            "collector::production_importer_v1::numerical_policy_v1::math::import_tests::{test_name}"
        );
        let scratch = Scratch::new();
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test_name,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD_ENV, child_key)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", &scratch.0)
                .env("CARGO_PKG_NAME", "policy-math-import-source")
                .env(
                    reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
                    registration_binding_v1().to_hex(),
                )
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "91919191919191919191919191919191",
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
        format!("-Ctarget-cpu={cpu}"),
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
    let mut probe = Probe {
        cpu,
        completed: false,
        check_ssa,
        check_kir,
        source,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(
        probe.completed,
        "must reach successful V21 import and every negative check"
    );
}

#[test]
fn policy_math_registration_uses_session_derived_binding() {
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;

    assert_eq!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1("different_policy_math_crate", [METADATA])
    );
    assert_ne!(
        registration_binding_v1(),
        derive_crate_binding_id_v1(CRATE_NAME, ["different_policy_math_metadata"])
    );
    assert!(!SOURCE.contains("namespace"));
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; never runs Cargo"]
fn policy_math_all13_full_import_gfx942_v21() {
    run("gfx942", "policy_math_all13_full_import_gfx942_v21");
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; never runs Cargo"]
fn policy_math_all13_full_import_gfx950_v21() {
    run("gfx950", "policy_math_all13_full_import_gfx950_v21");
}
