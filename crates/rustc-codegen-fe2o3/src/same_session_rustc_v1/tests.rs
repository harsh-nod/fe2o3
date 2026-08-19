use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_rustc_front::{
    CanonicalKernelInstIdV1, CanonicalKernelItemIdV1, FrontendLaunchBoundsV1,
    FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::LocalDefId;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::{Instance, TyCtxt};

use super::*;
use crate::collector::{
    AuthenticatedKernelFrontendContractV1, CollectedFunction, CollectedFunctionRole,
};

const FIXTURE_SOURCE: &str = r#"
#![allow(dead_code)]

#[inline(never)]
fn helper(value: u32) -> u32 {
    value
}

#[inline(never)]
fn kernel(value: u32) {
    let _ = helper(value);
}
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct DriverResults {
    positive: OwnerControlledRustKernelImportV1,
    item_mismatch: SameSessionRustcErrorV1,
    instance_mismatch: SameSessionRustcErrorV1,
    mir_mismatch: SameSessionRustcErrorV1,
    abi_mismatch: SameSessionRustcErrorV1,
    binding_mismatch: SameSessionRustcErrorV1,
    foreign: SameSessionRustcErrorV1,
    stale: SameSessionRustcErrorV1,
}

#[derive(Default)]
struct CustodyCallbacks {
    results: Option<DriverResults>,
}

impl Callbacks for CustodyCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let collection = collection(tcx);

        let positive = import_ordinary_rust_kernel_same_session_v1(tcx, &collection).unwrap();

        let mut item_owner = RustcSessionCustodianV1::new(tcx);
        let mut item = item_owner.capture(tcx, &collection).unwrap();
        item.observed_item = alternate_item(item.observed_item);
        let item_mismatch = expect_error(item_owner.release(item));

        let mut instance_owner = RustcSessionCustodianV1::new(tcx);
        let mut instance = instance_owner.capture(tcx, &collection).unwrap();
        instance.observed_instance = alternate_instance(instance.observed_instance);
        let instance_mismatch = expect_error(instance_owner.release(instance));

        let mut mir_owner = RustcSessionCustodianV1::new(tcx);
        let mut mir = mir_owner.capture(tcx, &collection).unwrap();
        mir.observed_mir_closure[0] ^= 1;
        let mir_mismatch = expect_error(mir_owner.release(mir));

        let mut abi_owner = RustcSessionCustodianV1::new(tcx);
        let mut abi = abi_owner.capture(tcx, &collection).unwrap();
        abi.observed_abi_closure[0] ^= 1;
        let abi_mismatch = expect_error(abi_owner.release(abi));

        let mut binding_owner = RustcSessionCustodianV1::new(tcx);
        let mut binding = binding_owner.capture(tcx, &collection).unwrap();
        binding.custody_binding[0] ^= 1;
        let binding_mismatch = expect_error(binding_owner.release(binding));

        let mut first_owner = RustcSessionCustodianV1::new(tcx);
        let foreign_receipt = first_owner.capture(tcx, &collection).unwrap();
        let mut second_owner = RustcSessionCustodianV1::new(tcx);
        second_owner.pending = true;
        let foreign = expect_error(second_owner.release(foreign_receipt));

        let mut stale_owner = RustcSessionCustodianV1::new(tcx);
        let stale_receipt = stale_owner.capture(tcx, &collection).unwrap();
        let _released = stale_owner.release(stale_receipt).unwrap();
        let stale = expect_error(stale_owner.capture(tcx, &collection));

        self.results = Some(DriverResults {
            positive,
            item_mismatch,
            instance_mismatch,
            mir_mismatch,
            abi_mismatch,
            binding_mismatch,
            foreign,
            stale,
        });
        Compilation::Stop
    }
}

fn expect_error<T>(result: Result<T, SameSessionRustcErrorV1>) -> SameSessionRustcErrorV1 {
    match result {
        Ok(_) => panic!("expected same-session custody rejection"),
        Err(error) => error,
    }
}

fn scalar_contract() -> KernelFrontendContractV1 {
    let one = FrontendWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    KernelFrontendContractV1::new(
        Some(FrontendLaunchBoundsV1::new(Some(one), Some(one), None).unwrap()),
        None,
    )
    .unwrap()
}

fn collection(tcx: TyCtxt<'_>) -> CollectionResult<'_> {
    CollectionResult {
        functions: vec![
            collected(tcx, "kernel", true),
            collected(tcx, "helper", false),
        ],
        ..CollectionResult::default()
    }
}

fn collected<'tcx>(tcx: TyCtxt<'tcx>, name: &str, kernel: bool) -> CollectedFunction<'tcx> {
    CollectedFunction {
        instance: Instance::mono(tcx, local_function(tcx, name).to_def_id()),
        role: if kernel {
            CollectedFunctionRole::KernelEntry
        } else {
            CollectedFunctionRole::InternalHelper
        },
        export_name: format!("fe2o3_test_{name}"),
        logical_name: kernel.then(|| name.to_owned()),
        typed_profile: None,
        kernel_binding: None,
        typed_layout_identities: None,
        general_typed_contract: None,
        frontend_contract: kernel
            .then(|| AuthenticatedKernelFrontendContractV1::for_test(scalar_contract())),
        dead_branches: None,
    }
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture function `{name}`"))
}

fn alternate_item(item: CanonicalKernelItemIdV1) -> CanonicalKernelItemIdV1 {
    let mut bytes = *item.as_bytes();
    bytes[16] ^= 1;
    CanonicalKernelItemIdV1::new(bytes).unwrap()
}

fn alternate_instance(instance: CanonicalKernelInstIdV1) -> CanonicalKernelInstIdV1 {
    let mut bytes = *instance.as_bytes();
    bytes[128] ^= 1;
    CanonicalKernelInstIdV1::new(bytes).unwrap()
}

struct CompilerFixture {
    source: PathBuf,
    output: PathBuf,
}

impl CompilerFixture {
    fn create() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stem = format!("fe2o3-rustc-custody-{}-{sequence}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.rs"));
        let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
        fs::write(&source, FIXTURE_SOURCE).expect("write same-session fixture");
        Self { source, output }
    }
}

impl Drop for CompilerFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.output);
    }
}

fn compiler_results() -> DriverResults {
    let fixture = CompilerFixture::create();
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("query rustc sysroot");
    assert!(sysroot.status.success(), "rustc --print sysroot failed");
    let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 rustc sysroot");
    let args = vec![
        "rustc".to_owned(),
        "--crate-name".to_owned(),
        "fe2o3_rustc_custody_fixture".to_owned(),
        "--crate-type".to_owned(),
        "lib".to_owned(),
        "--edition".to_owned(),
        "2024".to_owned(),
        "--emit".to_owned(),
        "metadata".to_owned(),
        "-Cpanic=abort".to_owned(),
        "--sysroot".to_owned(),
        sysroot.trim().to_owned(),
        "-o".to_owned(),
        fixture.output.display().to_string(),
        fixture.source.display().to_string(),
    ];
    let mut callbacks = CustodyCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    callbacks
        .results
        .expect("same-session custody callback did not run")
}

#[test]
fn releases_only_owned_authenticated_data_after_same_session_join() {
    let results = compiler_results();
    assert_eq!(results.positive.function_count(), 2);
    assert_eq!(
        results.positive.mir_closure(),
        results.positive.imported().mir_closure_identity()
    );
    assert_ne!(results.positive.abi_closure(), &[0; 32]);
    assert_ne!(results.positive.custody_binding(), &[0; 32]);
    assert!(!results.positive.grants_compiler_authority());
    assert!(!results.positive.imported().grants_execution_authority());
}

#[test]
fn rejects_every_session_custody_substitution_axis() {
    let results = compiler_results();
    assert!(matches!(
        results.item_mismatch,
        SameSessionRustcErrorV1::ItemMismatch
    ));
    assert!(matches!(
        results.instance_mismatch,
        SameSessionRustcErrorV1::InstanceMismatch
    ));
    assert!(matches!(
        results.mir_mismatch,
        SameSessionRustcErrorV1::MirMismatch
    ));
    assert!(matches!(
        results.abi_mismatch,
        SameSessionRustcErrorV1::AbiMismatch
    ));
    assert!(matches!(
        results.binding_mismatch,
        SameSessionRustcErrorV1::CustodyBindingMismatch
    ));
    assert!(matches!(
        results.foreign,
        SameSessionRustcErrorV1::ForeignCustodian
    ));
    assert!(matches!(
        results.stale,
        SameSessionRustcErrorV1::StaleCustodian
    ));
}
