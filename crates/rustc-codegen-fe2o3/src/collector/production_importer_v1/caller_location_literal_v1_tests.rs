//! Candidate opt-in AMD canonical-import/expansion regression; parent mounts.
//! Source collection and canonical replay precede every assertion below.

use super::{
    build_identity_inventory_v1, canonical_target_layout_v1,
    construct_production_semantic_mir_v1, validate_execution_terminal_carriage_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{
    SemanticCallExpansionErrorV1, SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::ty::TyCtxt;
use rustc_session::config::Input;
use rustc_span::FileName;
use std::path::PathBuf;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn literal_quotient(_context: KernelContext<'_>, numerator: usize, divisor: usize) {
    let _first = numerator.div_ceil(16);
    let _second = numerator.div_ceil(SECOND);
}
"#;

#[derive(Clone, Copy)]
enum Case {
    Literal,
    Zero,
    Dynamic,
}
struct Probe {
    cpu: &'static str,
    case: Case,
    completed: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("caller_location_literal_import.rs".into()),
            input: SOURCE.replace(
                "SECOND",
                match self.case {
                    Case::Literal => "2",
                    Case::Zero => "0",
                    Case::Dynamic => "divisor",
                },
            ),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("retain registered physical root and complete original call closure");
        let observed =
            crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
                .unwrap()
                .authenticate_import_session(tcx)
                .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots)
                .unwrap();
        assert!(
            closure
                .collection
                .functions
                .iter()
                .all(|function| function.closure_plan.is_none())
        );
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &std::collections::BTreeSet::new(),
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("retain the live source plan for exact producer and carriage checks");
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect(
            "actual source must complete authenticated canonical import before the expansion gate",
        );
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let source = &imported.semantic_mir;
        // A diagnostic selector in a test, never an acceptance rule. Production
        // collection already authenticated the exact original core definition.
        let helpers = plan
            .function_producers()
            .iter()
            .enumerate()
            .filter(|(_, function)| {
                tcx.def_path_str(function.instance.def_id())
                    .ends_with("::div_ceil")
            })
            .collect::<Vec<_>>();
        let [(index, producer)] = helpers.as_slice() else {
            panic!("exact retained core quotient helper: {}", helpers.len())
        };
        let function_id = SemanticFunctionIdV1::from_index(*index as u32);
        let function = &source.functions()[*index];
        assert_eq!(producer.identities.function(), function.identity());
        let raw = plan.function_mir(function_id).unwrap();
        assert!(std::ptr::eq(raw, tcx.instance_mir(producer.instance.def)));
        assert_eq!(raw.arg_count, 2);
        assert_eq!(
            raw.local_decls[rustc_middle::mir::Local::from_usize(1)].ty,
            tcx.types.usize
        );
        assert_eq!(
            raw.local_decls[rustc_middle::mir::Local::from_usize(2)].ty,
            tcx.types.usize
        );
        assert_eq!(
            raw.basic_blocks
                .iter()
                .filter(|block| matches!(
                    block.terminator().kind,
                    rustc_middle::mir::TerminatorKind::Assert { .. }
                ))
                .count(),
            3
        );
        assert_eq!(function.abi().hidden_arguments().len(), 1);
        assert_eq!(
            function.abi().hidden_arguments()[0].role(),
            SemanticAbiArgumentRoleV1::Hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation)
        );
        let body = &plan.body_producers()[*index];
        assert_eq!(body.function, function_id);
        assert_eq!(body.entry, function.entry());
        assert_eq!(body.source.provenance, function.source());
        assert_eq!(body.blocks.len(), raw.basic_blocks.len());
        assert_eq!(body.blocks.len(), function.blocks().len());
        assert_eq!(body.locals.len(), raw.local_decls.len());
        assert_eq!(body.locals.len(), function.locals().len());
        assert_eq!(body.raw_to_semantic_blocks.len(), body.blocks.len());
        assert_eq!(body.raw_to_semantic_locals.len(), body.locals.len());
        // The production tables are identity-sorted, not raw rustc MIR order.
        for (raw_index, semantic) in body.raw_to_semantic_blocks.iter().enumerate() {
            let binding = &body.blocks[semantic.index() as usize];
            let canonical = &function.blocks()[semantic.index() as usize];
            assert_eq!(binding.rustc_block as usize, raw_index);
            assert_eq!(canonical.identity(), binding.identity);
            assert_eq!(canonical.source(), binding.source.provenance);
            assert_eq!(
                canonical.terminator().source(),
                binding.terminator.provenance
            );
            assert_eq!(canonical.statements().len(), binding.statements.len());
            for (statement, retained) in canonical.statements().iter().zip(&binding.statements) {
                assert_eq!(statement.source(), retained.provenance);
            }
        }
        for (raw_index, semantic) in body.raw_to_semantic_locals.iter().enumerate() {
            let binding = &body.locals[semantic.index() as usize];
            let canonical = &function.locals()[semantic.index() as usize];
            assert_eq!(binding.rustc_local as usize, raw_index);
            assert_eq!(canonical.identity(), binding.identity);
            assert_eq!(canonical.ty(), binding.ty);
            assert_eq!(canonical.source(), binding.source.provenance);
        }
        let raw_assertion_blocks = raw
            .basic_blocks
            .iter_enumerated()
            .filter_map(|(bb, block)| {
                matches!(
                    block.terminator().kind,
                    rustc_middle::mir::TerminatorKind::Assert { .. }
                )
                .then_some(bb.as_usize())
            })
            .collect::<Vec<_>>();
        assert_eq!(raw_assertion_blocks, [0, 1, 3]);
        let mut assertion_blocks = raw_assertion_blocks
            .iter()
            .map(|&raw| body.raw_to_semantic_blocks[raw])
            .collect::<Vec<_>>();
        assertion_blocks.sort_by_key(|block| block.index());
        assert_eq!(
            function
                .blocks()
                .iter()
                .enumerate()
                .filter_map(|(bb, block)| {
                    matches!(
                        block.terminator().kind(),
                        SemanticTerminatorKindV1::Assert { .. }
                    )
                    .then_some(SemanticBlockIdV1::from_index(bb as u32))
                })
                .collect::<Vec<_>>(),
            assertion_blocks,
        );
        let first_assertion_block = assertion_blocks[0];
        let mapped_place = |raw: usize| {
            let local = body.raw_to_semantic_locals[raw];
            SemanticPlaceV1::new(
                local,
                vec![],
                function.locals()[local.index() as usize].ty(),
            )
            .unwrap()
        };
        let raw_add = &raw.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(3)].statements[0];
        let rustc_middle::mir::StatementKind::Assign(raw_assignment) = &raw_add.kind else {
            panic!("original checked-add assignment");
        };
        assert_eq!(raw_assignment.0.local.as_usize(), 8);
        assert!(raw_assignment.0.projection.is_empty());
        assert!(matches!(
            &raw_assignment.1,
            rustc_middle::mir::Rvalue::BinaryOp(rustc_middle::mir::BinOp::AddWithOverflow, _)
        ));
        let add_block = &function.blocks()[body.raw_to_semantic_blocks[3].index() as usize];
        let SemanticStatementKindV1::Assign(assignment) = add_block.statements()[0].kind() else {
            panic!("mapped original checked-add assignment");
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
            panic!("mapped checked add must not become an unchecked operation");
        };
        assert_eq!(assignment.destination(), &mapped_place(8));
        assert_eq!(assignment.value().result_type(), mapped_place(8).ty());
        assert_eq!(checked.operation(), SemanticCheckedBinaryOpV1::Add);
        assert_eq!(checked.left(), &SemanticOperandV1::Copy(mapped_place(3)));
        let word = mapped_place(1).ty();
        assert_eq!(
            source.types()[word.index() as usize].shape(),
            &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        );
        assert_eq!(
            checked.right(),
            &SemanticOperandV1::Constant(SemanticConstantV1::new(
                word,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 8).unwrap()),
            ))
        );
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target,
            unwind,
        } = add_block.terminator().kind()
        else {
            panic!("the original checked addition must retain its assertion");
        };
        let boolean = mapped_place(4).ty();
        let overflow_flag = SemanticPlaceV1::new(
            body.raw_to_semantic_locals[8],
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), boolean).unwrap()],
            boolean,
        )
        .unwrap();
        assert_eq!(condition, &SemanticOperandV1::Move(overflow_flag));
        assert!(!expected);
        assert!(matches!(message,
            SemanticAssertMessageV1::Overflow { operation: SemanticBinaryOpV1::Add, left, right }
                if left == checked.left() && right == checked.right()));
        assert_eq!(target.role(), SemanticEdgeRoleV1::AssertSuccess);
        assert_eq!(target.target(), body.raw_to_semantic_blocks[4]);
        assert_eq!(*unwind, SemanticUnwindActionV1::Unreachable);
        let calls = source.functions().iter().flat_map(|f| f.blocks()).filter_map(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { return None };
            matches!(source.callables()[call.callee().index() as usize], SemanticCallableDeclV1::Defined { function } if function == function_id).then_some(call)
        }).collect::<Vec<_>>();
        assert_eq!(
            calls.len(),
            2,
            "safe and second call must share the retained definition"
        );
        let bytes = source.canonical_encoding().to_vec();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.functions(), source.functions());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        for mir in [source, &decoded] {
            let result =
                SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default());
            match self.case {
                Case::Literal => {
                    let expansion = result.expect(
                        "both literal calls must establish nonobservability before expansion",
                    );
                    expansion.verify_replay(mir).unwrap();
                    let root = expansion.root(mir.roots()[0]).unwrap();
                    let instances = root
                        .instances()
                        .iter()
                        .filter(|instance| instance.function() == function_id)
                        .collect::<Vec<_>>();
                    assert_eq!(instances.len(), 2);
                    let retained = root
                        .body()
                        .blocks()
                        .iter()
                        .zip(root.block_origins())
                        .filter(|(block, origin)| {
                            root.instances()[origin.instance().index() as usize].function()
                                == function_id
                                && matches!(
                                    block.terminator().kind(),
                                    SemanticTerminatorKindV1::Assert { .. }
                                )
                        })
                        .count();
                    assert_eq!(
                        retained, 6,
                        "no assertion or original helper instance is terminalized"
                    );
                    for instance in instances {
                        let mut mapped_assertions = Vec::new();
                        for (block, origin) in root.body().blocks().iter().zip(root.block_origins())
                        {
                            if !std::ptr::eq(
                                &root.instances()[origin.instance().index() as usize],
                                instance,
                            ) || !matches!(
                                block.terminator().kind(),
                                SemanticTerminatorKindV1::Assert { .. }
                            ) {
                                continue;
                            }
                            let original = &mir.functions()[function_id.index() as usize].blocks()
                                [origin.block().index() as usize];
                            assert_eq!(origin.function(), function_id);
                            assert_eq!(
                                origin.terminator(),
                                fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source,
                            );
                            assert_eq!(block.terminator().source(), original.terminator().source());
                            mapped_assertions.push(origin.block());
                        }
                        mapped_assertions.sort_by_key(|block| block.index());
                        assert_eq!(mapped_assertions, assertion_blocks);
                    }
                }
                Case::Zero | Case::Dynamic => assert!(
                    matches!(result,
                    Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block), reason: "caller-location frame contains an observing assertion" })
                        if function == function_id && block == first_assertion_block),
                    "the earlier safe call cannot authorize the second invocation"
                ),
            }
        }
        assert_eq!(source.canonical_encoding(), bytes);
        assert_eq!(tcx.sess.opts.cg.target_cpu.as_deref(), Some(self.cpu));
        self.completed = true;
        Compilation::Stop
    }
}

const CRATE_NAME: &str = "caller_location_literal_import";
const METADATA: &str = "fe2o3-caller-location-literal-import-v1";
const CHILD_ENV: &str = "FE2O3_CALLER_LOCATION_LITERAL_IMPORT_CHILD";

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
            "fe2o3-caller-location-literal-import-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create exclusively owned test scratch directory");
        let scratch = Self(path);
        // proc_macro_crate reads this manifest to resolve the real device import.
        // No Cargo process or dependency build is run.
        let device = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        std::fs::write(scratch.0.join("Cargo.toml"), format!(
            "[package]\nname = \"caller-location-literal-import\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n"
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

fn run(cpu: &'static str, test_name: &str, case: Case) {
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
            "collector::production_importer_v1::caller_location_literal_v1_tests::{test_name}"
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
                .env("CARGO_PKG_NAME", "caller-location-literal-import")
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
        case,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(
        probe.completed,
        "must reach canonical import and exact caller-location expansion checks"
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS"]
fn caller_location_literal_full_import_gfx942() {
    run(
        "gfx942",
        "caller_location_literal_full_import_gfx942",
        Case::Literal,
    );
}
#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS"]
fn caller_location_literal_full_import_gfx950() {
    run(
        "gfx950",
        "caller_location_literal_full_import_gfx950",
        Case::Literal,
    );
}
#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS"]
fn caller_location_literal_full_import_zero_does_not_reuse_safe_call() {
    run(
        "gfx942",
        "caller_location_literal_full_import_zero_does_not_reuse_safe_call",
        Case::Zero,
    );
}
#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS"]
fn caller_location_literal_full_import_dynamic_does_not_reuse_safe_call() {
    run(
        "gfx942",
        "caller_location_literal_full_import_dynamic_does_not_reuse_safe_call",
        Case::Dynamic,
    );
}
