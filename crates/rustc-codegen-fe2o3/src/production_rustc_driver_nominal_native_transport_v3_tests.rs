//! Strict actual-rustc transport parents. No synthetic source/catalog factory.
use super::*;
use crate::kernel_ir_codegen::{InertCompilerModuleTextV1, nominal_v3 as module};
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::transport::nominal_transport_source_child";
const MIXED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::transport::nominal_transport_mixed_source_child";
const RESOURCE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::transport::nominal_transport_resource_child";
const RESOURCE_MIXED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::transport::nominal_transport_resource_mixed_child";
const FLOOR: usize = 2048;
const LEAVES: &[&str] = &[
    "crates/rustc-codegen-fe2o3/src/production_rustc_driver_nominal_native_transport_v3_tests.rs",
    "crates/rustc-codegen-fe2o3/src/production_nominal_native_transport_v3.rs",
    "crates/rustc-codegen-fe2o3/src/production_nominal_native_transport_resource_v3_tests.rs",
    "crates/rustc-codegen-fe2o3/src/kernel_ir_codegen.rs",
    "crates/rustc-codegen-fe2o3/src/kernel_ir_codegen_nominal_descriptor_v3.rs",
    "crates/fe2o3-amdgcn-model/src/native_v12_text_descriptor_replay_v3.rs",
    "crates/fe2o3-amdgcn-model/src/descriptor_physical_abi_v3.rs",
    "crates/fe2o3-amdgcn-model/src/descriptor_capability_projection_v3.rs",
    "crates/fe2o3-compiler-ffi/src/descriptor_source_v3.rs",
    "crates/fe2o3-lower-mir-kernel/src/production_source_catalog_callback_v1.rs",
];
fn transport_stamps() -> Vec<Stamp> {
    let mut result = super::stamps();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in LEAVES {
        if !result.iter().any(|stamp| stamp.path == *path) {
            result.push(Stamp {
                path: (*path).to_owned(),
                sha256: digest(&std::fs::read(workspace.join(path)).unwrap()),
            });
        }
    }
    result
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportObservation {
    case: Case,
    mixed: bool,
    profile: String,
    callback_count: usize,
    request_sha256: [u8; 32],
    actual_args_sha256: [u8; 32],
    generated_source_sha256: Option<[u8; 32]>,
    source: Vec<Stamp>,
    source_kinds: [u8; 5],
    component_counts: [usize; 5],
    descriptor: [u8; 32],
    descriptor_bytes: usize,
    descriptor_identity: [u8; 32],
    descriptor_capacity: usize,
    descriptor_backing_transferred: bool,
    original: [u8; 32],
    forwarding: [u8; 32],
    unrolled: [u8; 32],
    native_prefix: [u8; 32],
    embedded_native: [u8; 32],
    embedded_bytes: usize,
    erased: bool,
    selected_iterations: Option<u8>,
    binding_version: Option<u16>,
    artifact_authority: bool,
    ffi_authority: bool,
    input_receipt: usize,
    addition: usize,
    expected_addition: usize,
    floor: usize,
    work: usize,
    peak: usize,
    replay_work: usize,
    replay_peak: usize,
    short_work_accepted: usize,
    short_storage_work: usize,
    short_storage_prior_peak: usize,
    #[serde(deserialize_with = "required_resource_option")]
    resource_oracle: Option<Vec<u8>>,
}
fn resource(error: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    if let Some(error) = error.downcast_ref::<Resource>() {
        return Some(*error);
    }
    error.source().and_then(resource)
}
struct TransportCallbacks {
    args: Vec<String>,
    request: Vec<String>,
    case: Case,
    mixed: bool,
    qualify_resources: bool,
    callbacks: usize,
    result: Option<R<TransportObservation>>,
}
impl Callbacks for TransportCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            if self.callbacks != 1 {
                return Err(fail(Phase::Rustc, "exactly one transport callback"));
            }
            let profile = tcx
                .sess
                .opts
                .cg
                .target_cpu
                .as_deref()
                .unwrap_or(tcx.sess.target.cpu.as_ref());
            if !["gfx942", "gfx950"].contains(&profile)
                || self
                    .args
                    .iter()
                    .filter(|arg| **arg == format!("-Ctarget-cpu={profile}"))
                    .count()
                    != 1
            {
                return Err(fail(Phase::Request, "actual target/invocation mismatch"));
            }
            let resource_oracle =
                if self.qualify_resources {
                    let mut seed_work = Work::new(WORK);
                    let mut seed_budget = Budget::new(&mut seed_work, STORAGE);
                    seed_budget.reserve_storage(FLOOR).unwrap();
                    let seed_transaction = transaction_in_active_session_v1(
                        tcx,
                        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                    )
                    .map_err(|e| fail(Phase::Collection, e))?;
                    let (seed, receipt) = seed_transaction
                        .prepare_nominal_loop_unroll_native_v3(
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            &mut seed_budget,
                        )
                        .map_err(|e| fail(Phase::Factory, e))?;
                    assert_eq!(seed_budget.storage(), FLOOR);
                    seed_budget
                        .reserve_storage(receipt.retained_storage())
                        .unwrap();
                    let result = seed.qualify_resource_oracle_v3(
                        receipt,
                        WORK,
                        STORAGE,
                        self.mixed,
                        |budget| {
                            transaction_in_active_session_v1(
                        tcx, crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                    ).and_then(|transaction| {
                        transaction.prepare_nominal_loop_unroll_native_v3(
                            Default::default(), Default::default(), Default::default(), budget,
                        ).map_err(|error| format!("{error:?}"))
                    })
                        },
                    );
                    seed_budget
                        .release_storage(receipt.retained_storage())
                        .unwrap();
                    assert_eq!(seed_budget.storage(), FLOOR);
                    Some(result.map_err(|e| fail(Phase::Replay, e))?)
                } else {
                    None
                };
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail(Phase::Collection, e))?;
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(17).unwrap();
            let (mut input, input_receipt) = transaction
                .prepare_nominal_loop_unroll_native_v3(
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    &mut budget,
                )
                .map_err(|e| fail(Phase::Factory, e))?;
            if budget.storage() != FLOOR
                || input.retained_storage_floor_v1() != FLOOR + input_receipt.retained_storage()
            {
                return Err(fail(Phase::Floor, "original U transfer floor"));
            }
            budget
                .reserve_storage(input_receipt.retained_storage())
                .map_err(|e| fail(Phase::Floor, e))?;
            input
                .source_test_mutations_v3(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            let incoming = budget.storage();
            let old_header = std::mem::size_of_val(&input);
            let old_wire_pointer = input.canonical_bytes().as_ptr();
            let old_wire_capacity = input.source_test_wire_capacity_v3();
            let descriptor = digest(input.canonical_bytes());
            let native_prefix = digest(input.llvm_ir().as_bytes());
            let prefix_bytes = input.llvm_ir().len();
            let original = *input
                .original()
                .map_err(|e| fail(Phase::Rows, e))?
                .canonical()
                .identity()
                .digest();
            let forwarding = *input.forwarding_output().canonical().identity().digest();
            let unrolled = *input.output().canonical().identity().digest();
            let (erased, selected_iterations) = input.source_test_selection_v3();
            if self.mixed && (selected_iterations != Some(3) || forwarding == unrolled) {
                return Err(fail(
                    Phase::Rows,
                    (
                        "literal mixed source must genuinely change F into U",
                        erased,
                        selected_iterations,
                        input.forwarding_output().module(),
                    ),
                ));
            }
            let (mut owner, additional) = input
                .into_nominal_descriptor_transport_v3(&mut budget)
                .map_err(|e| fail(Phase::Factory, e))?;
            if budget.storage() != incoming
                || owner.retained_storage_floor_v3() != incoming + additional.retained_storage()
            {
                return Err(fail(Phase::Floor, "consuming transport addition floor"));
            }
            budget
                .reserve_storage(additional.retained_storage())
                .map_err(|e| fail(Phase::Floor, e))?;
            let floor = budget.storage();
            let module_storage = module::module_storage(owner.module(), &mut budget)
                .map_err(|e| fail(Phase::Rows, e))?;
            let expected_addition = (std::mem::size_of_val(&owner) - old_header)
                + (module_storage - std::mem::size_of::<InertCompilerModuleTextV1>());
            if additional.retained_storage() != expected_addition {
                return Err(fail(
                    Phase::Rows,
                    "no duplicate A1 Vec or embedded Module header credit",
                ));
            }
            if owner.descriptor_source().canonical_bytes().as_ptr() != old_wire_pointer
                || owner.descriptor_source().storage().retained_storage()
                    - COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3
                    != old_wire_capacity
                || digest(owner.descriptor_source().canonical_bytes()) != descriptor
                || *owner.output().canonical().identity().digest() != unrolled
                || *owner.forwarding_output().canonical().identity().digest() != forwarding
            {
                return Err(fail(
                    Phase::Rows,
                    "actual backing/source/F/U custody transfer",
                ));
            }
            let text = owner.module().llvm_ir();
            if digest(
                text.get(..prefix_bytes)
                    .ok_or_else(|| fail(Phase::Rows, "complete native prefix"))?
                    .as_bytes(),
            ) != native_prefix
                || text.matches(".section .fe2o3.kd.v3").count() != 1
                || text.contains(".fe2o3.kd.v1")
                || owner.module().descriptor_binding_version_for_test_v3() != Some(3)
            {
                return Err(fail(
                    Phase::Rows,
                    "distinct V3 native section and exact actual-U prefix",
                ));
            }
            let source = owner.descriptor_source();
            const CALLER: usize = 4 * DESCRIPTOR_QUERY_STORAGE_V3
                + std::mem::size_of::<[u8; 5]>()
                + std::mem::size_of::<[usize; 5]>();
            budget
                .reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3 + CALLER)
                .map_err(|e| fail(Phase::Floor, e))?;
            let table = source
                .table(
                    source.storage().retained_storage()
                        + COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
                    &mut |n| budget.charge_work(n),
                )
                .map_err(|e| fail(Phase::Rows, e))?;
            if table.kernel_count() != 1
                || table.device_target().as_amd_target_id().processor() != profile
            {
                return Err(fail(Phase::Rows, "complete target/root table"));
            }
            let row = table
                .kernel(0, &mut |n| budget.charge_work(n))
                .map_err(|e| fail(Phase::Rows, e))?;
            let name = if self.mixed {
                MIXED_ROOT
            } else {
                self.case.roots()[0]
            };
            let count = if self.mixed { 5 } else { 3 };
            if row.entry_name() != name || row.argument_count() != count {
                return Err(fail(Phase::Rows, "actual source signature"));
            }
            let mut source_kinds = [0u8; 5];
            let mut component_counts = [0usize; 5];
            let mut cursor = row.arguments();
            for index in 0..count {
                let argument = cursor
                    .next(&mut |n| budget.charge_work(n))
                    .map_err(|e| fail(Phase::Rows, e))?
                    .ok_or_else(|| fail(Phase::Rows, "complete argument cursor"))?;
                if argument.source_index() as usize != index {
                    return Err(fail(Phase::Rows, "actual source ordinal"));
                }
                let ty = table
                    .source_type(argument.source_type(), &mut |n| budget.charge_work(n))
                    .map_err(|e| fail(Phase::Rows, e))?;
                source_kinds[index] = match ty.descriptor() {
                    Kind::SharedSlice(ScalarTypeV1::U32) => 2,
                    Kind::DisjointSlice(ScalarTypeV1::U32) => 3,
                    Kind::Usize => 5,
                    Kind::Isize => 6,
                    Kind::Scalar(ScalarTypeV1::U64) => 7,
                    Kind::Scalar(ScalarTypeV1::I64) => 8,
                    Kind::DisjointSlice(ScalarTypeV1::U64) => 9,
                    _ => return Err(fail(Phase::Rows, "nominal and fixed source kinds")),
                };
                component_counts[index] = argument.component_count();
            }
            if cursor
                .next(&mut |n| budget.charge_work(n))
                .map_err(|e| fail(Phase::Rows, e))?
                .is_some()
            {
                return Err(fail(Phase::Rows, "no extra argument"));
            }
            drop(cursor);
            drop(row);
            drop(table);
            budget
                .release_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3 + CALLER)
                .unwrap();
            if source_kinds != expected_kinds(self.mixed)
                || component_counts != expected_components(self.mixed)
                || budget.storage() != floor
            {
                return Err(fail(
                    Phase::Rows,
                    "complete nominal signature and caller query floor",
                ));
            }
            owner
                .source_test_mutations_v3(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            let replay = |w, p| {
                let mut work = Work::new(w);
                let mut b = Budget::new(&mut work, p);
                b.reserve_storage(floor).unwrap();
                b.charge_work(17).unwrap();
                let result = owner.verify_equivalence(&mut b);
                assert_eq!(b.storage(), floor);
                (result, b.work(), b.peak_storage(), b.failed_storage())
            };
            let (full, replay_work, replay_peak, failed) = replay(WORK, STORAGE);
            full.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!(failed, None);
            let (exact, ew, ep, failed) = replay(replay_work, replay_peak);
            exact.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!((ew, ep, failed), (replay_work, replay_peak, None));
            let (short, short_work_accepted, _, _) = replay(replay_work - 1, replay_peak);
            let error = short.expect_err("bounded transport work must refuse");
            let Some(Resource::Work(limit)) = resource(&error) else {
                return Err(fail(Phase::Replay, ("typed nested work refusal", &error)));
            };
            assert_eq!(limit.limit(), replay_work - 1);
            assert!(limit.actual() > limit.limit() && short_work_accepted <= limit.limit());
            let (short, short_storage_work, short_storage_prior_peak, failed) =
                replay(replay_work, replay_peak - 1);
            let error = short.expect_err("bounded transport storage must refuse");
            let Some(Resource::Storage(limit)) = resource(&error) else {
                return Err(fail(
                    Phase::Replay,
                    (
                        "typed nested storage refusal",
                        &error,
                        replay_work,
                        replay_peak,
                        failed,
                    ),
                ));
            };
            assert_eq!(
                (limit.actual(), limit.limit(), failed),
                (replay_peak, replay_peak - 1, Some(replay_peak))
            );
            owner
                .verify_equivalence(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            let source = owner.descriptor_source();
            let result = TransportObservation {
                case: self.case,
                mixed: self.mixed,
                profile: profile.to_owned(),
                callback_count: self.callbacks,
                request_sha256: digest(&serde_json::to_vec(&self.request).unwrap()),
                actual_args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                generated_source_sha256: self.mixed.then(|| digest(MIXED_SOURCE.as_bytes())),
                source: transport_stamps(),
                source_kinds,
                component_counts,
                descriptor,
                descriptor_bytes: source.canonical_bytes().len(),
                descriptor_identity: *source.identity().sha256(),
                descriptor_capacity: source.storage().retained_storage()
                    - COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3,
                descriptor_backing_transferred: source.canonical_bytes().as_ptr()
                    == old_wire_pointer,
                original,
                forwarding,
                unrolled,
                native_prefix,
                embedded_native: digest(owner.module().llvm_ir().as_bytes()),
                embedded_bytes: owner.module().llvm_ir().len(),
                erased,
                selected_iterations,
                binding_version: owner.module().descriptor_binding_version_for_test_v3(),
                artifact_authority: owner.grants_artifact_or_launch_authority(),
                ffi_authority: source.authenticates_compiler_origin()
                    || source.grants_link_authority()
                    || source.grants_load_authority()
                    || source.grants_launch_authority(),
                input_receipt: input_receipt.retained_storage(),
                addition: additional.retained_storage(),
                expected_addition,
                floor,
                work: budget.work(),
                peak: budget.peak_storage(),
                replay_work,
                replay_peak,
                short_work_accepted,
                short_storage_work,
                short_storage_prior_peak,
                resource_oracle,
            };
            drop(owner);
            budget
                .release_storage(additional.retained_storage())
                .unwrap();
            budget
                .release_storage(input_receipt.retained_storage())
                .unwrap();
            if budget.storage() != FLOOR {
                return Err(fail(
                    Phase::Floor,
                    "dependent-first drop and exactly one old receipt refund",
                ));
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}

fn run_transport_child(mixed: bool, qualify_resources: bool) {
    let request_path =
        PathBuf::from(env::var_os(CHILD_ARGS).expect("actual transport parent request"));
    let request: Vec<String> =
        serde_json::from_slice(&std::fs::read(&request_path).unwrap()).unwrap();
    let result = request_case(&request).and_then(|case| {
        if mixed && case != Case::Control {
            return Err(fail(Phase::Request, "exact mixed parent case"));
        }
        let generated = request_path.with_file_name("nominal-literal-mixed-transport.rs");
        let args = if mixed {
            assert!(!generated.exists(), "never overwrite another agent fixture");
            std::fs::write(&generated, MIXED_SOURCE).map_err(|e| fail(Phase::Request, e))?;
            if std::fs::read(&generated).map_err(|e| fail(Phase::Request, e))?
                != MIXED_SOURCE.as_bytes()
            {
                return Err(fail(Phase::Request, "exact generated source"));
            }
            generated_args(&request, &generated)
        } else {
            request.clone()
        };
        require_canonical_overflow_checks_v1(&args).map_err(|e| fail(Phase::Request, e))?;
        let mut callback = TransportCallbacks {
            args: args.clone(),
            request,
            case,
            mixed,
            qualify_resources,
            callbacks: 0,
            result: None,
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rustc_driver::run_compiler(&args, &mut callback)
        })) {
            Ok(()) => callback
                .result
                .unwrap_or_else(|| Err(fail(Phase::Rustc, "transport callback absent"))),
            Err(_) => Err(fail(Phase::Rustc, "actual transport compiler panicked")),
        }
    });
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    std::fs::write(
        PathBuf::from(env::var_os(CHILD_RESULT).expect("actual transport response")),
        &bytes,
    )
    .unwrap();
    println!(
        "NOMINAL_TRANSPORT_V3_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("NOMINAL_TRANSPORT_V3_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source refusal is not transport qualification: {result:?}"
    );
}
#[test]
#[ignore = "actual parent-managed ordinary rustc transport child"]
fn nominal_transport_source_child() {
    run_transport_child(false, false);
}
#[test]
#[ignore = "actual parent-managed mixed literal rustc transport child"]
fn nominal_transport_mixed_source_child() {
    run_transport_child(true, false);
}

#[test]
#[ignore = "actual parent-managed ordinary-source consuming resource oracle"]
fn nominal_transport_resource_child() {
    run_transport_child(false, true);
}
#[test]
#[ignore = "actual parent-managed changing mixed-source consuming resource oracle"]
fn nominal_transport_resource_mixed_child() {
    run_transport_child(true, true);
}

// Independent parent schema, not a deserialize/reexport of the child's oracle
// types. This is bounded test evidence, never a production authority token.
fn required_resource_option<'de, D, T>(d: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceDenial {
    kind: u8,
    actual: usize,
    limit: usize,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceOutcome {
    work: usize,
    peak: usize,
    #[serde(deserialize_with = "required_resource_option")]
    failed_storage: Option<usize>,
    #[serde(deserialize_with = "required_resource_option")]
    failed_work: Option<usize>,
    returned_input_floor: usize,
    final_sibling_floor: usize,
    #[serde(deserialize_with = "required_resource_option")]
    additional: Option<usize>,
    #[serde(deserialize_with = "required_resource_option")]
    denial: Option<ResourceDenial>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceCut {
    work_limit: usize,
    storage_limit: usize,
    prior_failures: bool,
    #[serde(deserialize_with = "required_resource_option")]
    predicted_phase: Option<String>,
    expected: ResourceOutcome,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceReport {
    key_sha256: [u8; 32],
    erased: bool,
    #[serde(deserialize_with = "required_resource_option")]
    selected: Option<u8>,
    work: usize,
    peak: usize,
    phases: usize,
    actual_factories: usize,
    reachable_storage_events: usize,
    masked_storage_events: usize,
    shared_work_boundaries: usize,
    cuts: Vec<ResourceCut>,
}
#[derive(Serialize)]
struct ResourceKey {
    source: [u8; 32],
    forwarding: [u8; 32],
    unrolled: [u8; 32],
    wire: [u8; 32],
    wire_bytes: usize,
    capacity: usize,
    erased: bool,
    selected: Option<u8>,
    receipt: usize,
}
fn check_resource_report(report: &TransportObservation) {
    let bytes = report.resource_oracle.as_ref().unwrap();
    assert!(bytes.len() <= 128 * 1024);
    let resource: ResourceReport = serde_json::from_slice(bytes).unwrap();
    let key = ResourceKey {
        source: report.original,
        forwarding: report.forwarding,
        unrolled: report.unrolled,
        wire: report.descriptor,
        wire_bytes: report.descriptor_bytes,
        capacity: report.descriptor_capacity,
        erased: report.erased,
        selected: report.selected_iterations,
        receipt: report.input_receipt,
    };
    assert_eq!(
        resource.key_sha256,
        digest(&serde_json::to_vec(&key).unwrap())
    );
    assert_eq!(resource.erased, report.erased);
    assert_eq!(resource.selected, report.selected_iterations);
    assert!(resource.work > 17 && resource.work <= WORK);
    assert!(resource.peak > 1 + report.input_receipt && resource.peak <= STORAGE);
    assert_eq!(resource.phases, 19);
    assert!(resource.reachable_storage_events > 0 && resource.masked_storage_events > 0);
    assert!(resource.reachable_storage_events + resource.masked_storage_events <= resource.phases);
    assert!(
        resource.shared_work_boundaries > 0 && resource.shared_work_boundaries < resource.phases
    );
    assert!(resource.cuts.len() > 6 && resource.cuts.len() <= 46);
    // One isolated probe, one independent full composition, one retry, and two
    // fresh genuine factories for every recorded oracle/SUT comparison.
    assert_eq!(resource.actual_factories, 3 + 2 * resource.cuts.len());
    assert!(resource.actual_factories <= 96);
    let phases = [
        "Guard",
        "Entry",
        "OldReplay",
        "NewHeader",
        "ValidationReserve",
        "Validation",
        "ValidationDrop",
        "ModuleBuild",
        "ModuleRetain",
        "VerifyGuard",
        "VerifyEntry",
        "ModuleExtent",
        "CustodyReplay",
        "MetadataReplay",
        "TableReserve",
        "TableRead",
        "ReaderDrop",
        "CatalogAndModel",
        "TableDrop",
    ];
    let mut seen = std::collections::BTreeSet::new();
    for cut in &resource.cuts {
        assert!(seen.insert((cut.work_limit, cut.storage_limit, cut.prior_failures)));
        assert!(cut.work_limit >= 17 && cut.work_limit <= resource.work);
        assert!(
            cut.storage_limit >= 1 + report.input_receipt && cut.storage_limit <= resource.peak
        );
        let result = &cut.expected;
        assert!(result.work <= cut.work_limit && result.peak <= cut.storage_limit);
        assert_eq!(result.returned_input_floor, 1 + report.input_receipt);
        assert_eq!(result.final_sibling_floor, 1);
        assert_eq!(result.additional.is_some(), result.denial.is_none());
        assert_eq!(cut.predicted_phase.is_some(), result.denial.is_some());
        if cut.prior_failures {
            assert_eq!(result.failed_work, Some(cut.work_limit + 1));
            assert_eq!(result.failed_storage, Some(cut.storage_limit + 1));
        }
        match &result.denial {
            None => {
                assert_eq!(
                    (cut.work_limit, cut.storage_limit),
                    (resource.work, resource.peak)
                );
                assert_eq!((result.work, result.peak), (resource.work, resource.peak));
                assert_eq!(result.additional, Some(report.addition));
                if !cut.prior_failures {
                    assert_eq!((result.failed_work, result.failed_storage), (None, None));
                }
            }
            Some(denial) => {
                assert!(phases.contains(&cut.predicted_phase.as_deref().unwrap()));
                assert!(denial.actual > denial.limit);
                match denial.kind {
                    1 => {
                        assert_eq!(denial.limit, cut.work_limit);
                        assert!(result.work < resource.work);
                        if !cut.prior_failures {
                            assert_eq!(result.failed_work, Some(denial.actual));
                            assert_eq!(result.failed_storage, None);
                        }
                    }
                    2 => {
                        assert_eq!(denial.limit, cut.storage_limit);
                        if !cut.prior_failures {
                            assert_eq!(result.failed_storage, Some(denial.actual));
                            assert_eq!(result.failed_work, None);
                        }
                    }
                    other => panic!("typed work/storage cause required, not {other}"),
                }
            }
        }
    }
    for prior in [false, true] {
        for (work, peak) in [
            (resource.work, resource.peak),
            (resource.work - 1, resource.peak),
            (resource.work, resource.peak - 1),
        ] {
            assert!(seen.contains(&(work, peak, prior)));
        }
    }
    assert_eq!(
        resource
            .cuts
            .iter()
            .filter(|cut| cut.prior_failures)
            .count(),
        3
    );
}

fn check_transport(path: &Path, roots: &[&str], mixed: bool, resources: bool) {
    let response: R<TransportObservation> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let report = response.expect("actual nominal transport must succeed, not merely refuse safely");
    assert_eq!(report.mixed, mixed);
    assert_eq!(report.case.roots(), roots);
    assert_eq!(report.callback_count, 1);
    assert_eq!(report.source, transport_stamps());
    let request: Vec<String> = serde_json::from_slice(
        &std::fs::read(path.with_file_name(format!("{}-args.json", report.case.feature())))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request_case(&request).unwrap(), report.case);
    assert_eq!(
        report.request_sha256,
        digest(&serde_json::to_vec(&request).unwrap())
    );
    let actual = if mixed {
        let generated = path.with_file_name("nominal-literal-mixed-transport.rs");
        assert_eq!(std::fs::read(&generated).unwrap(), MIXED_SOURCE.as_bytes());
        assert_eq!(
            report.generated_source_sha256,
            Some(digest(MIXED_SOURCE.as_bytes()))
        );
        assert_eq!(report.selected_iterations, Some(3));
        assert_ne!(report.forwarding, report.unrolled);
        generated_args(&request, &generated)
    } else {
        assert_eq!(report.generated_source_sha256, None);
        request
    };
    assert_eq!(
        report.actual_args_sha256,
        digest(&serde_json::to_vec(&actual).unwrap())
    );
    assert_eq!(
        actual
            .iter()
            .filter(|arg| **arg == format!("-Ctarget-cpu={}", report.profile))
            .count(),
        1
    );
    assert_eq!(report.source_kinds, expected_kinds(mixed));
    assert_eq!(report.component_counts, expected_components(mixed));
    assert_eq!(report.binding_version, Some(3));
    assert!(!report.artifact_authority && !report.ffi_authority);
    assert!(report.descriptor_backing_transferred);
    assert!(report.descriptor_capacity >= report.descriptor_bytes && report.descriptor_bytes > 48);
    for hash in [
        report.descriptor,
        report.descriptor_identity,
        report.original,
        report.forwarding,
        report.unrolled,
        report.native_prefix,
        report.embedded_native,
    ] {
        assert_ne!(hash, [0; 32]);
    }
    assert_ne!(report.native_prefix, report.embedded_native);
    assert!(report.embedded_bytes > 100);
    assert_eq!(report.addition, report.expected_addition);
    assert_eq!(report.floor, FLOOR + report.input_receipt + report.addition);
    assert!(report.work > 17 && report.work <= WORK && report.peak >= report.floor);
    assert!(
        report.replay_work > 17 && report.replay_work <= WORK && report.replay_peak > report.floor
    );
    assert!(
        report.short_work_accepted < report.replay_work
            && report.short_storage_work < report.replay_work
    );
    assert!(report.short_storage_prior_peak < report.replay_peak);
    assert_eq!(report.resource_oracle.is_some(), resources);
    if resources {
        check_resource_report(&report);
    }
}
fn check_ordinary(path: &Path, roots: &[&str]) {
    check_transport(path, roots, false, false);
}
fn check_mixed(path: &Path, roots: &[&str]) {
    check_transport(path, roots, true, false);
}
fn check_resource_ordinary(path: &Path, roots: &[&str]) {
    check_transport(path, roots, false, true);
}
fn check_resource_mixed(path: &Path, roots: &[&str]) {
    check_transport(path, roots, true, true);
}
#[test]
#[ignore = "bounded consuming resource qualification with ten actual AMD rustc children"]
fn ordinary_rust_nominal_transport_v3_resource_oracle() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let cases: Vec<_> = CASES
            .into_iter()
            .map(OrdinarySourceCase::GuardedLoopRead)
            .collect();
        ordinary_rust_source_cases(
            &cases,
            profile,
            false,
            Some(SourceObserver {
                child_test: RESOURCE_CHILD,
                check: check_resource_ordinary,
            }),
        );
        ordinary_rust_source_cases(
            &[OrdinarySourceCase::GuardedLoopRead(Case::Control)],
            profile,
            false,
            Some(SourceObserver {
                child_test: RESOURCE_MIXED_CHILD,
                check: check_resource_mixed,
            }),
        );
    }
}
#[test]
#[ignore = "requires pinned rust-src and eight actual AMD rustc transport children"]
fn ordinary_rust_nominal_transport_v3_reaches_common_native_transport() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let cases: Vec<_> = CASES
            .into_iter()
            .map(OrdinarySourceCase::GuardedLoopRead)
            .collect();
        ordinary_rust_source_cases(
            &cases,
            profile,
            false,
            Some(SourceObserver {
                child_test: CHILD,
                check: check_ordinary,
            }),
        );
    }
}
#[test]
#[ignore = "requires pinned rust-src and two genuinely changing mixed literal AMD rustc transport children"]
fn ordinary_rust_nominal_transport_v3_literal_mixed_keeps_actual_final_u() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[OrdinarySourceCase::GuardedLoopRead(Case::Control)],
            profile,
            false,
            Some(SourceObserver {
                child_test: MIXED_CHILD,
                check: check_mixed,
            }),
        );
    }
}
#[test]
fn nominal_transport_v3_request_roster_is_ten_distinct_actual_source_children() {
    let mut roster = std::collections::BTreeSet::new();
    for profile in ["gfx942", "gfx950"] {
        for case in CASES {
            assert!(roster.insert((profile, CHILD, case.feature())));
        }
        assert!(roster.insert((profile, MIXED_CHILD, Case::Control.feature())));
    }
    assert_eq!(roster.len(), 10);
    assert_ne!(CHILD, super::CHILD);
    assert_ne!(MIXED_CHILD, super::MIXED_CHILD);
    assert_ne!(CHILD, MIXED_CHILD);
    assert_eq!(expected_kinds(true), [5, 6, 7, 8, 9]);
}

#[test]
fn nominal_transport_v3_resource_roster_preserves_all_ten_source_obligations() {
    let mut roster = std::collections::BTreeSet::new();
    for profile in ["gfx942", "gfx950"] {
        for case in CASES {
            assert!(roster.insert((profile, RESOURCE_CHILD, case.feature())));
        }
        assert!(roster.insert((profile, RESOURCE_MIXED_CHILD, Case::Control.feature())));
    }
    assert_eq!(roster.len(), 10);
    assert_eq!(
        [CHILD, MIXED_CHILD, RESOURCE_CHILD, RESOURCE_MIXED_CHILD]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn nominal_transport_v3_resource_report_rejects_missing_and_unknown_fields() {
    let key = [0u8; 32];
    let mut value = serde_json::json!({
        "key_sha256": key,
        "erased": false, "selected": null,
        "work": 23, "peak": 29, "phases": 19, "actual_factories": 3,
        "reachable_storage_events": 1, "masked_storage_events": 1,
        "shared_work_boundaries": 1, "cuts": []
    });
    assert!(serde_json::from_value::<ResourceReport>(value.clone()).is_ok());
    let mut missing_nullable = value.clone();
    missing_nullable.as_object_mut().unwrap().remove("selected");
    assert!(serde_json::from_value::<ResourceReport>(missing_nullable).is_err());
    value["unknown"] = serde_json::json!(1);
    assert!(serde_json::from_value::<ResourceReport>(value.clone()).is_err());
    value.as_object_mut().unwrap().remove("unknown");
    value.as_object_mut().unwrap().remove("peak");
    assert!(serde_json::from_value::<ResourceReport>(value).is_err());
    for value in [
        serde_json::json!({"kind": 1, "actual": 24}),
        serde_json::json!({"kind": 1, "actual": 24, "limit": 23, "unknown": 1}),
    ] {
        assert!(serde_json::from_value::<ResourceDenial>(value).is_err());
    }
    let mut outcome = serde_json::json!({
        "work": 23, "peak": 29, "failed_storage": null, "failed_work": null,
        "returned_input_floor": 19, "final_sibling_floor": 1,
        "additional": 3, "denial": null
    });
    assert!(serde_json::from_value::<ResourceOutcome>(outcome.clone()).is_ok());
    outcome.as_object_mut().unwrap().remove("failed_work");
    assert!(serde_json::from_value::<ResourceOutcome>(outcome).is_err());
}
