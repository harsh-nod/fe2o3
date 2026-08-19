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

#[inline(never)]
fn empty_kernel() {}
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct SendCustodyCheck<T: ?Sized>(std::marker::PhantomData<T>);

trait AmbiguousIfSend<Marker> {
    fn marker() {}
}

impl<T: ?Sized> AmbiguousIfSend<()> for SendCustodyCheck<T> {}
impl<T: ?Sized + Send> AmbiguousIfSend<u8> for SendCustodyCheck<T> {}

struct DriverResults {
    supported_function_count: usize,
    supported_mir_matches: bool,
    supported_abi_is_bound: bool,
    supported_custody_is_bound: bool,
    supported_import_is_bound: bool,
    supported_graph_is_live: bool,
    supported_join_is_exact: bool,
    supported_join_rejects_substitution: bool,
    supported_rewrite_count: usize,
    supported_grants_no_authority: bool,
    unsupported_code: UnsupportedRustMirDiagnosticCodeV1,
    unsupported_is_terminal: bool,
    unsupported_custody_is_bound: bool,
    item_mismatch: SameSessionRustcErrorV1,
    instance_mismatch: SameSessionRustcErrorV1,
    mir_mismatch: SameSessionRustcErrorV1,
    abi_mismatch: SameSessionRustcErrorV1,
    binding_mismatch: SameSessionRustcErrorV1,
    semantic_identity_mismatch: SameSessionRustcErrorV1,
    semantic_order_mismatch: SameSessionRustcErrorV1,
    semantic_span_mismatch: SameSessionRustcErrorV1,
    semantic_successor_mismatch: SameSessionRustcErrorV1,
    foreign: SameSessionRustcErrorV1,
    stale: SameSessionRustcErrorV1,
    deterministic_mir_import: bool,
}

#[derive(Default)]
struct CustodyCallbacks {
    results: Option<DriverResults>,
}

impl Callbacks for CustodyCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let collection = collection(tcx);
        let supported_collection = supported_collection(tcx);

        let mut supported = expect_supported(
            import_ordinary_rust_kernel_same_session_v1(tcx, &supported_collection).unwrap(),
        );
        let unsupported = expect_unsupported(
            import_ordinary_rust_kernel_same_session_v1(tcx, &collection).unwrap(),
        );
        let supported_function_count = supported.function_count();
        let supported_mir_matches =
            supported.mir_closure() == supported.imported().mir_closure_identity();
        let supported_abi_is_bound = supported.abi_closure() != &[0; 32];
        let supported_custody_is_bound = supported.custody_binding() != &[0; 32];
        let supported_import_is_bound = supported.mir_import_identity() != &[0; 32];
        let supported_graph_is_live = supported.retained_graph_is_live();
        let supported_join_is_exact = supported.source_identity_join_is_exact();
        let supported_rewrite_count = supported.lowering_record().rewrite_count();
        let supported_grants_no_authority = !supported.grants_compiler_authority()
            && !supported.imported().grants_execution_authority();
        let supported_join_rejects_substitution =
            identity_join_rejects_every_substitution(&mut supported);
        let unsupported_code = unsupported.code();
        let unsupported_is_terminal = unsupported.is_terminal();
        let unsupported_custody_is_bound = unsupported.custody_binding() != &[0; 32];

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

        let mut semantic_identity_owner = RustcSessionCustodianV1::new(tcx);
        let mut semantic_identity = semantic_identity_owner.capture(tcx, &collection).unwrap();
        semantic_identity.semantic.functions[0].ordered_blocks[0].operations[0].identity[0] ^= 1;
        let semantic_identity_mismatch =
            expect_error(semantic_identity_owner.release(semantic_identity));

        let mut semantic_order_owner = RustcSessionCustodianV1::new(tcx);
        let mut semantic_order = semantic_order_owner.capture(tcx, &collection).unwrap();
        if let Some(operations) = semantic_order
            .semantic
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.ordered_blocks)
            .map(|block| &mut block.operations)
            .find(|operations| operations.len() >= 2)
        {
            operations.swap(0, 1);
        } else if let Some(blocks) = semantic_order
            .semantic
            .functions
            .iter_mut()
            .map(|function| &mut function.ordered_blocks)
            .find(|blocks| blocks.len() >= 2)
        {
            blocks.swap(0, 1);
        } else {
            semantic_order.semantic.functions.swap(0, 1);
        }
        let semantic_order_mismatch = expect_error(semantic_order_owner.release(semantic_order));

        let mut semantic_span_owner = RustcSessionCustodianV1::new(tcx);
        let mut semantic_span = semantic_span_owner.capture(tcx, &collection).unwrap();
        let operation = &mut semantic_span.semantic.functions[0].ordered_blocks[0].operations[0];
        let expansion = operation.provenance.expansion();
        let [start_line, start_column, end_line, end_column] = expansion.coordinates();
        let substituted_expansion = MirSemanticSourceSpan::new(
            expansion.file_identity(),
            start_line,
            start_column,
            end_line,
            end_column.saturating_add(1),
        )
        .unwrap();
        operation.provenance =
            MirSemanticSpanProvenance::new(substituted_expansion, operation.provenance.call_site())
                .unwrap();
        let semantic_span_mismatch = expect_error(semantic_span_owner.release(semantic_span));

        let mut semantic_successor_owner = RustcSessionCustodianV1::new(tcx);
        let mut semantic_successor = semantic_successor_owner.capture(tcx, &collection).unwrap();
        let operation = semantic_successor
            .semantic
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.ordered_blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| !operation.successors.is_empty())
            .expect("fixture requires one MIR successor");
        operation.successors[0] ^= 1;
        let semantic_successor_mismatch =
            expect_error(semantic_successor_owner.release(semantic_successor));

        let deterministic_left = import_ordinary_rust_kernel_same_session_v1(tcx, &collection)
            .expect("first deterministic import");
        let deterministic_right = import_ordinary_rust_kernel_same_session_v1(tcx, &collection)
            .expect("second deterministic import");
        let deterministic_mir_import =
            outcome_mir_import(&deterministic_left) == outcome_mir_import(&deterministic_right);

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
            supported_function_count,
            supported_mir_matches,
            supported_abi_is_bound,
            supported_custody_is_bound,
            supported_import_is_bound,
            supported_graph_is_live,
            supported_join_is_exact,
            supported_join_rejects_substitution,
            supported_rewrite_count,
            supported_grants_no_authority,
            unsupported_code,
            unsupported_is_terminal,
            unsupported_custody_is_bound,
            item_mismatch,
            instance_mismatch,
            mir_mismatch,
            abi_mismatch,
            binding_mismatch,
            semantic_identity_mismatch,
            semantic_order_mismatch,
            semantic_span_mismatch,
            semantic_successor_mismatch,
            foreign,
            stale,
            deterministic_mir_import,
        });
        Compilation::Stop
    }
}

fn expect_supported(outcome: SameSessionRustKernelOutcomeV1) -> OwnerControlledRustKernelImportV1 {
    match outcome {
        SameSessionRustKernelOutcomeV1::Supported(supported) => *supported,
        SameSessionRustKernelOutcomeV1::Unsupported(diagnostic) => {
            panic!("expected supported MIR admission, got {diagnostic}")
        }
    }
}

fn expect_unsupported(outcome: SameSessionRustKernelOutcomeV1) -> UnsupportedRustMirDiagnosticV1 {
    match outcome {
        SameSessionRustKernelOutcomeV1::Supported(_) => {
            panic!("expected terminal unsupported MIR observation")
        }
        SameSessionRustKernelOutcomeV1::Unsupported(diagnostic) => diagnostic,
    }
}

fn outcome_mir_import(outcome: &SameSessionRustKernelOutcomeV1) -> &[u8; 32] {
    match outcome {
        SameSessionRustKernelOutcomeV1::Supported(supported) => supported.mir_import_identity(),
        SameSessionRustKernelOutcomeV1::Unsupported(diagnostic) => diagnostic.mir_import_identity(),
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

fn supported_collection(tcx: TyCtxt<'_>) -> CollectionResult<'_> {
    CollectionResult {
        functions: vec![collected(tcx, "empty_kernel", true)],
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

fn identity_join_rejects_every_substitution(
    supported: &mut OwnerControlledRustKernelImportV1,
) -> bool {
    let original_item = supported.identity_join.item;
    supported.identity_join.item = alternate_item(original_item);
    let item_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.item = original_item;

    let original_instance = supported.identity_join.instance;
    supported.identity_join.instance = alternate_instance(original_instance);
    let instance_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.instance = original_instance;

    supported.identity_join.source_closure_identity[0] ^= 1;
    let source_closure_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.source_closure_identity[0] ^= 1;

    supported.identity_join.frontend_import_identity[0] ^= 1;
    let frontend_import_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.frontend_import_identity[0] ^= 1;

    supported.identity_join.mir_closure[0] ^= 1;
    let mir_closure_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.mir_closure[0] ^= 1;

    supported.identity_join.abi_closure[0] ^= 1;
    let abi_closure_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.abi_closure[0] ^= 1;

    supported.identity_join.mir_import[0] ^= 1;
    let mir_import_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.mir_import[0] ^= 1;

    supported.identity_join.pliron_graph[0] ^= 1;
    let pliron_graph_rejected = !supported.source_identity_join_is_exact();
    supported.identity_join.pliron_graph[0] ^= 1;

    item_rejected
        && instance_rejected
        && source_closure_rejected
        && frontend_import_rejected
        && mir_closure_rejected
        && abi_closure_rejected
        && mir_import_rejected
        && pliron_graph_rejected
        && supported.source_identity_join_is_exact()
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
    assert_eq!(results.supported_function_count, 1);
    assert!(results.supported_mir_matches);
    assert!(results.supported_abi_is_bound);
    assert!(results.supported_custody_is_bound);
    assert!(results.supported_import_is_bound);
    assert!(results.supported_graph_is_live);
    assert!(results.supported_join_is_exact);
    assert!(results.supported_join_rejects_substitution);
    assert_eq!(results.supported_rewrite_count, 1);
    assert_eq!(
        results.unsupported_code,
        UnsupportedRustMirDiagnosticCodeV1::UnsupportedSemanticOperation
    );
    assert!(results.unsupported_is_terminal);
    assert!(results.unsupported_custody_is_bound);
    assert!(results.deterministic_mir_import);
    assert!(results.supported_grants_no_authority);
}

#[test]
fn retained_owner_graph_is_compile_time_thread_affine() {
    let _ = <SendCustodyCheck<OwnerControlledRustKernelImportV1> as AmbiguousIfSend<_>>::marker;
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
        results.semantic_identity_mismatch,
        SameSessionRustcErrorV1::CustodyBindingMismatch
    ));
    assert!(matches!(
        results.semantic_order_mismatch,
        SameSessionRustcErrorV1::CustodyBindingMismatch
    ));
    assert!(matches!(
        results.semantic_span_mismatch,
        SameSessionRustcErrorV1::CustodyBindingMismatch
    ));
    assert!(matches!(
        results.semantic_successor_mismatch,
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
