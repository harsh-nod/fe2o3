//! Actual cached AMD core -> collection -> preflight -> semantic producer.
//! This uses the existing compiler fixture root pattern, not launch authority.

use super::*;
use crate::collector::{DeviceCollector, KernelRoot};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticLayoutIdentityV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::TerminatorKind;
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
#![allow(unused_unsafe)]
pub fn byte_left(a: u8, n: u32, out: &mut u8) { *out = a.wrapping_shl(n); }
pub fn wide_right(a: u64, n: u32, out: &mut u64) { *out = a.wrapping_shr(n); }
pub fn signed_right(a: i64, n: u32, out: &mut i64) { *out = a.wrapping_shr(n); }
pub fn word_left(a: usize, n: u32, out: &mut usize) { *out = a.wrapping_shl(n); }
pub fn byte_sub(a: u8, b: u8, out: &mut u8) { *out = a.wrapping_sub(b); }
pub fn u32_sub(a: u32, b: u32, out: &mut u32) { *out = a.wrapping_sub(b); }
pub fn wide_sub(a: u64, b: u64, out: &mut u64) { *out = a.wrapping_sub(b); }
pub fn signed_sub(a: i64, b: i64, out: &mut i64) { *out = a.wrapping_sub(b); }
pub fn word_sub(a: usize, b: usize, out: &mut usize) { *out = a.wrapping_sub(b); }
pub fn raw_left(a: u8, n: u32, out: &mut u8) { *out = unsafe { a.unchecked_shl(n) }; }
pub fn raw_right(a: u64, n: u32, out: &mut u64) { *out = unsafe { a.unchecked_shr(n) }; }
pub fn user_unsafe(a: u8, n: u32, out: &mut u8) { *out = unsafe { a.wrapping_shl(n) }; }
pub fn user_unsafe_sub(a: u8, b: u8, out: &mut u8) { *out = unsafe { a.wrapping_sub(b) }; }
pub fn reachable_panic_sub(a: u32, b: u32, out: &mut u32) {
    *out = a.wrapping_sub(b);
    if b == 7 { panic!("reachable after wrapping subtraction"); }
}
pub fn reachable_panic(a: u64, n: u32, out: &mut u64) {
    *out = a.wrapping_shr(n);
    if n == 7 { panic!("reachable after masked shift"); }
}
"#;

fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
) -> Result<CollectionResult<'tcx>, crate::collector::CollectError> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx950".into());
    collector
        .add_root(KernelRoot {
            target: Instance::mono(tcx, definition.to_def_id()),
            logical_name: name.into(),
            export_name: name.into(),
            generated_host_contract_identity: None,
            kernel_binding: Some(reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(
                [43; 32],
            )),
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector.collect()
}

fn produce<'tcx>(tcx: TyCtxt<'tcx>, name: &str, element: Ty<'tcx>, operation: SemanticBinaryOpV1) {
    let expanded = matches!(
        operation,
        SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight
    );
    let collection = collect(tcx, name)
        .expect("complete safe wrapping proof precedes recursive panic discovery");
    assert_eq!(
        collection.functions.len(),
        2,
        "caller plus defined safe wrapper"
    );
    let mut functions = collection
        .functions
        .iter()
        .map(|function| RetainedSemanticFunctionProducerV1 {
            identities: canonical_function_identities_v1(tcx, function.instance),
            instance: function.instance,
            role: function.role,
            export_name: function
                .is_kernel_entry()
                .then(|| function.export_name.clone()),
            kernel_binding: function.kernel_binding,
            frontend_contract: function.frontend_contract.clone(),
            closure_admission: None,
        })
        .collect::<Vec<_>>();
    functions.sort_unstable_by_key(|function| function.identities.function());
    let root = SemanticFunctionIdV1::from_index(
        functions
            .iter()
            .position(|f| f.role == CollectedFunctionRole::KernelEntry)
            .unwrap() as u32,
    );
    let shift = SemanticFunctionIdV1::from_index(
        functions
            .iter()
            .position(|f| f.role == CollectedFunctionRole::InternalHelper)
            .unwrap() as u32,
    );
    let original = functions[shift.index() as usize].instance;
    let identities = functions[shift.index() as usize].identities;
    let original_hash = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, original);
    if expanded {
        assert!(
            tcx.instance_mir(original.def)
                .basic_blocks
                .iter()
                .any(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
        );
    }
    // The semantic target layout is shared by gfx942/gfx950; no launch or
    // capability terminal is issued by these scalar-only producer fixtures.
    let target =
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([44; 32]));
    let (inventory, _) = identity_inventory_identity_and_transcript_v1(target, &functions, &[root]);
    let plan = build_production_semantic_preflight_plan_v1(
        tcx,
        target,
        functions.into_boxed_slice(),
        vec![root].into_boxed_slice(),
        inventory,
        &BTreeSet::new(),
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("proved masked shift survives both production preflight passes");
    assert_eq!(plan.direct_call_producers().len(), 1);
    assert!(plan.terminal_producers().is_empty());
    assert!(plan.normalized_intrinsic_producers().is_empty());
    let selected = plan.function_mir(shift).unwrap();
    assert_eq!(selected.source.instance, original.def);
    assert_eq!(
        std::ptr::eq(selected, tcx.instance_mir(original.def)),
        !expanded
    );
    assert_eq!(selected.basic_blocks.len(), 1);
    assert_eq!(selected.local_decls.len(), if expanded { 4 } else { 3 });
    for local in &plan.body_producers()[shift.index() as usize].locals {
        let original_identity = crate::rustc_semantic_adapter_v1::rustc_local_identity_v1(
            identities.function(),
            original_hash,
            local.rustc_local,
        );
        if expanded {
            assert_ne!(
                local.identity, original_identity,
                "full closed proof binds expanded local identity"
            );
        } else {
            assert_eq!(
                local.identity, original_identity,
                "plain wrapping arithmetic retains original source identity"
            );
        }
    }
    let abi = &plan.function_abi_producers()[shift.index() as usize];
    assert_eq!(
        &*abi.source_inputs,
        &[element, if expanded { tcx.types.u32 } else { element }]
    );
    assert_eq!(abi.source_output, element);
    assert_eq!(abi.extern_abi, rustc_abi::ExternAbi::Rust);
    assert_eq!(
        plan.function_abi_producers()[root.index() as usize].extern_abi,
        rustc_abi::ExternAbi::GpuKernel
    );
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers()).unwrap();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap();
    let terminals =
        construct_production_semantic_fn_abis_v1(tcx, &[], plan.type_producers()).unwrap();
    let contexts = AuthenticatedProductionKernelContextsV1 {
        frontend_unit_identity: [0; 32],
        target_brand_identity: [0; 32],
        expected_roots: Box::new([]),
        roots: Box::new([]),
        custody_identity: [0; 32],
        transpose_source: None,
        reusable_phase_source: None,
        global_bf16_constructors: None,
    };
    let mir = construct_complete_request_v1(
        tcx,
        target,
        &plan,
        &contexts,
        types.into_records(),
        abis,
        terminals,
    )
    .expect("general shift reaches canonical semantic producer without panic exemption");
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.functions(), mir.functions());
    assert_eq!(decoded.callables(), mir.callables());
    let function = mir
        .functions()
        .iter()
        .find(|function| function.identity() == identities.function())
        .unwrap();
    assert_eq!(function.abi().identity(), abi.identity);
    assert_eq!(function.blocks().len(), 1);
    assert!(matches!(
        function.blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::Return
    ));
    let operations = function.blocks()[0]
        .statements()
        .iter()
        .map(|statement| {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!("masked-shift assignment")
            };
            let SemanticRvalueKindV1::Binary { operation, .. } = assignment.value().kind() else {
                panic!("proved normal binary, not unchecked")
            };
            *operation
        })
        .collect::<Vec<_>>();
    assert_eq!(
        operations,
        if expanded {
            vec![SemanticBinaryOpV1::BitAnd, operation]
        } else {
            vec![operation]
        }
    );
    assert_eq!(
        crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, original),
        original_hash
    );
}

struct Probe {
    negative: bool,
    arithmetic: bool,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("general_shift_production.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        if self.negative {
            for name in [
                "raw_left",
                "raw_right",
                "reachable_panic",
                "reachable_panic_sub",
            ] {
                let error = collect(tcx, name)
                    .expect_err("no standalone unchecked or panic admission")
                    .to_string();
                assert!(error.contains("panic path"), "{name}: {error}");
            }
            for name in ["user_unsafe", "user_unsafe_sub"] {
                let error = collect(tcx, name)
                    .expect_err("safe core proof does not authenticate a user's unsafe block")
                    .to_string();
                assert!(error.contains("[FE2O3-CAP-SOURCE006]"), "{error}");
            }
        } else if self.arithmetic {
            for (name, element) in [
                ("byte_sub", tcx.types.u8),
                ("u32_sub", tcx.types.u32),
                ("wide_sub", tcx.types.u64),
                ("signed_sub", tcx.types.i64),
                ("word_sub", tcx.types.usize),
            ] {
                produce(tcx, name, element, SemanticBinaryOpV1::Subtract);
            }
        } else {
            for (name, element, operation) in [
                ("byte_left", tcx.types.u8, SemanticBinaryOpV1::ShiftLeft),
                ("wide_right", tcx.types.u64, SemanticBinaryOpV1::ShiftRight),
                (
                    "signed_right",
                    tcx.types.i64,
                    SemanticBinaryOpV1::ShiftRight,
                ),
                ("word_left", tcx.types.usize, SemanticBinaryOpV1::ShiftLeft),
            ] {
                produce(tcx, name, element, operation);
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(negative: bool, arithmetic: bool, mir_opt: Option<u8>) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=general_shift_production".into(),
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
    ];
    for (variable, name) in [
        ("FE2O3_WRAPPING_AMDGPU_CORE", "core"),
        ("FE2O3_WRAPPING_AMDGPU_BUILTINS", "compiler_builtins"),
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| {
            panic!("set {variable} to complete cached AMDGPU metadata; no Cargo or host fallback")
        }));
        assert!(path.is_file());
        args.extend([
            "--extern".into(),
            format!("noprelude,nounused:{name}={}", path.display()),
            "-L".into(),
            format!("dependency={}", path.parent().unwrap().display()),
        ]);
    }
    if let Some(level) = mir_opt {
        assert_eq!(level, 0);
        args.push("-Zmir-opt-level=0".into());
    }
    args.push("-".into());
    let mut probe = Probe {
        negative,
        arithmetic,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_collection_and_semantic_producer() {
    run(false, false, None);
    run(false, false, Some(0));
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_collection_rejects_unsafe_and_panicking_sources() {
    run(true, false, None);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn core_wrapping_arithmetic_actual_amdgpu_collection_and_semantic_producer() {
    run(false, true, None);
    run(false, true, Some(0));
}
