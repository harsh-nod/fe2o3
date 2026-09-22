//! Actual source through complete history serialization and decoded final native replay.
//! Source-file identity comes from rustc/retained custody, never from F2EPH1 alone.
#![allow(
    clippy::drop_non_drop,
    reason = "End borrowed descriptor queries before refunds."
)]
use super::*;
use fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3;
use fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustc_span::FileName;
use std::io::Read;

const NOOP_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::expanded_native_v3::serialized_history::expanded_noop_source_child";
const REWRITE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::expanded_native_v3::serialized_history::expanded_rewrite_source_child";
// Finite diagnostic headroom for the complete source/history/native route,
// not the ABI-only harness ceiling or a production/performance guarantee.
const WORK: usize = 1usize << 40;
const STORAGE: usize = 256 * 1024 * 1024;
const FLOOR: usize = 8192;
const BASE: &str =
    "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs";
const NOOP: &str = r#"#![no_std]
use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn expanded_raw_noop() {}
"#;
const REWRITE: &str = r#"#![no_std]
use fe2o3_device::{DisjointSlice, kernel, thread};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]), control_flow(loop_bounds(3)))]
pub fn expanded_scalar_loop(seed: usize, delta: isize, wide: u64, signed: i64, mut output: DisjointSlice<u64>) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = wide;
        let mut cursor = 0_usize;
        while cursor < 3 {
            *element = wide ^ (seed as u64) ^ (delta as u64) ^ (signed as u64) ^ (cursor as u64);
            cursor += 1;
        }
    }
}
"#;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Case {
    Noop,
    Rewrite,
}
impl Case {
    fn source(self) -> &'static str {
        match self {
            Self::Noop => NOOP,
            Self::Rewrite => REWRITE,
        }
    }
    fn root(self) -> &'static str {
        match self {
            Self::Noop => "expanded_raw_noop",
            Self::Rewrite => "expanded_scalar_loop",
        }
    }
    fn file(self) -> &'static str {
        match self {
            Self::Noop => "expanded-native-noop.rs",
            Self::Rewrite => "expanded-native-rewrite.rs",
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    phase: String,
    detail: String,
}
type R<T> = Result<T, Failure>;
fn fail(phase: &str, error: impl std::fmt::Debug) -> Failure {
    Failure {
        phase: phase.to_owned(),
        detail: format!("{error:?}"),
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn bounded(path: &Path, cap: u64) -> std::io::Result<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > cap {
        return Err(std::io::Error::other(
            "bounded regular source/report required",
        ));
    }
    let mut bytes = Vec::new();
    file.take(cap + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(std::io::Error::other("stable bounded file required"));
    }
    Ok(bytes)
}
const PINS: &[&str] = &[
    "crates/rustc-codegen-fe2o3/src/production_rustc_driver_expanded_history_v1_tests.rs",
    "crates/rustc-codegen-fe2o3/src/production_expanded_history_serializer_v1.rs",
    "crates/rustc-codegen-fe2o3/src/compiler_descriptor_expanded_decoded_v3.rs",
    "crates/fe2o3-lower-mir-kernel/src/production_expanded_decoded_source_v1.rs",
    "crates/fe2o3-kernel-opt/src/expanded_history_v1.rs",
    "crates/fe2o3-kernel-opt/src/scalar_fixed_point_history_decode_v1.rs",
    "crates/rustc-codegen-fe2o3/src/production_rustc_driver_expanded_native_v3_tests.rs",
    "crates/rustc-codegen-fe2o3/src/production_rustc_driver_checked_output_source_v1_tests.rs",
    "crates/rustc-codegen-fe2o3/src/production_pipeline.rs",
    "crates/rustc-codegen-fe2o3/src/production_pipeline_loop_unroll_native_v1.rs",
    "crates/rustc-codegen-fe2o3/src/production_pipeline_nominal_loop_unroll_native_v3.rs",
    "crates/rustc-codegen-fe2o3/src/production_pipeline_expanded_native_v3.rs",
    "crates/rustc-codegen-fe2o3/src/compiler_descriptor_expanded_nominal_v3.rs",
    "crates/rustc-codegen-fe2o3/src/compiler_descriptor_nominal_loop_unroll_v3.rs",
    "crates/rustc-codegen-fe2o3/src/production_expanded_native_transport_v3.rs",
    "crates/rustc-codegen-fe2o3/src/kernel_ir_codegen_nominal_descriptor_v3.rs",
    "crates/fe2o3-lower-mir-kernel/src/production_checked_output_expanded_policy_v1.rs",
    "crates/fe2o3-lower-mir-kernel/src/production_checked_output_expanded_source_v1.rs",
    "crates/fe2o3-lower-mir-kernel/src/production_source_catalog_callback_v1.rs",
    "crates/fe2o3-compiler-ffi/src/descriptor_source_v3.rs",
    "crates/fe2o3-amdgcn-model/src/native_v12_text_descriptor_replay_v3.rs",
];
fn stamps() -> Vec<(String, [u8; 32])> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    PINS.iter()
        .map(|path| {
            (
                (*path).to_owned(),
                digest(&bounded(&workspace.join(path), 4 * 1024 * 1024).unwrap()),
            )
        })
        .collect()
}
fn arguments(request: &[String], generated: &Path, case: Case) -> Vec<String> {
    let original = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(BASE)
        .canonicalize()
        .unwrap();
    let mut count = 0;
    let mut args: Vec<_> = request
        .iter()
        .map(|arg| {
            if Path::new(arg).canonicalize().ok().as_ref() == Some(&original) {
                count += 1;
                generated.to_str().unwrap().to_owned()
            } else {
                arg.clone()
            }
        })
        .collect();
    assert_eq!(count, 1);
    assert!(!args.iter().any(|s| s.starts_with("-Zmir-opt-level=")));
    if case == Case::Rewrite {
        args.push("-Zmir-opt-level=0".to_owned());
    }
    require_canonical_overflow_checks_v1(&args).unwrap();
    args
}
fn checked_fixture(tcx: TyCtxt<'_>, generated: &Path, case: Case) -> R<[u8; 32]> {
    let path = generated.canonicalize().map_err(|e| fail("source", e))?;
    let bytes = bounded(&path, 8192).map_err(|e| fail("source", e))?;
    if bytes != case.source().as_bytes() {
        return Err(fail("source", "exact generated source bytes"));
    }
    let files = tcx.sess.source_map().files();
    let mut matching = files.iter().filter(|file| match &file.name {
        FileName::Real(name) => name
            .local_path()
            .is_some_and(|p| p.canonicalize().ok().as_ref() == Some(&path)),
        _ => false,
    });
    let file = matching
        .next()
        .ok_or_else(|| fail("source", "compiled fixture absent"))?;
    if matching.next().is_some()
        || file.unnormalized_source_len as usize != bytes.len()
        || !file.src_hash.matches(case.source())
        || !file
            .src
            .as_ref()
            .is_some_and(|text| text.as_bytes() == bytes)
    {
        return Err(fail(
            "source",
            "independent rustc file identity/content join",
        ));
    }
    Ok(digest(&bytes))
}
fn resource(error: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    error
        .downcast_ref::<Resource>()
        .copied()
        .or_else(|| error.source().and_then(resource))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    case: Case,
    profile: String,
    callbacks: usize,
    request: [u8; 32],
    args: [u8; 32],
    fixture: [u8; 32],
    rustc_fixture: [u8; 32],
    source: Vec<(String, [u8; 32])>,
    semantic_roots: Vec<(u32, String)>,
    final_roots: Vec<String>,
    erased: bool,
    rounds: usize,
    changing_rounds: usize,
    unrolled: [u8; 32],
    final_graph: [u8; 32],
    emission_order: (usize, [u8; 4]),
    descriptor: [u8; 32],
    descriptor_pointer_retained: bool,
    native_prefix: [u8; 32],
    embedded_native: [u8; 32],
    source_kinds: Vec<u8>,
    input_receipt: usize,
    transport_addition: usize,
    expected_transport_addition: usize,
    history: [u8; 32],
    history_length: usize,
    history_addition: usize,
    expected_history_addition: usize,
    history_rounds: usize,
    history_final_range: (usize, usize),
    history_payload_exact: bool,
    work: usize,
    peak: usize,
    replay_work: usize,
    replay_peak: usize,
    short_work_accepted: usize,
    short_storage_prior_peak: usize,
    final_sibling_floor: usize,
    artifact_authority: bool,
    execution_authority: bool,
}
struct ExpandedCallbacks {
    request: Vec<String>,
    args: Vec<String>,
    generated: PathBuf,
    case: Case,
    count: usize,
    result: Option<R<Observation>>,
}
impl Callbacks for ExpandedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some((|| {
            if self.count != 1 {
                return Err(fail("rustc", "exactly one source callback"));
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
                    .filter(|a| **a == format!("-Ctarget-cpu={profile}"))
                    .count()
                    != 1
            {
                return Err(fail("request", "actual target matches argv"));
            }
            let rustc_fixture = checked_fixture(tcx, &self.generated, self.case)?;
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail("collection", e))?;
            transaction.reset_expanded_source_trace_v3();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let (mut input, receipt) = transaction
                .prepare_expanded_native_v3(&mut budget)
                .map_err(|e| fail("expanded-final factory", e))?;
            if budget.storage() != FLOOR
                || input.retained_storage_floor_v3() != FLOOR + receipt.retained_storage()
            {
                return Err(fail(
                    "floor",
                    "complete pre-native to final-native transfer",
                ));
            }
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (erased, rounds, changing_rounds, unrolled, final_graph, emission_order) =
                input.expanded_source_observation_v3();
            if emission_order != (4, [1, 2, 3, 4]) || rounds == 0 || rounds > 16 {
                return Err(fail(
                    "order",
                    "actual seed, scalar, descriptor, first emission",
                ));
            }
            match self.case {
                Case::Noop if erased || changing_rounds != 0 || unrolled != final_graph => {
                    return Err(fail("shape", "genuine Direct normal no-op"));
                }
                Case::Rewrite if !erased || changing_rounds == 0 || unrolled == final_graph => {
                    return Err(fail(
                        "shape",
                        "genuine Erased nonvacuous final scalar rewrite",
                    ));
                }
                _ => {}
            }
            let semantic_roots = input.expanded_source_ordered_roots_v3();
            let final_roots: Vec<_> = input
                .output()
                .module()
                .kernels
                .iter()
                .map(|k| k.id.as_str().to_owned())
                .collect();
            if semantic_roots
                .iter()
                .map(|(_, name)| name)
                .ne(final_roots.iter())
                || final_roots != [self.case.root()]
            {
                return Err(fail(
                    "roots",
                    "exact positional actual semantic/final roster",
                ));
            }
            input
                .expanded_source_hostiles_v3(&mut budget)
                .map_err(|e| fail("negative final evidence", e))?;
            let native_prefix = digest(input.llvm_ir().as_bytes());
            let prefix_length = input.llvm_ir().len();
            let descriptor = digest(input.canonical_bytes());
            let pointer = input.canonical_bytes().as_ptr();
            let input_header = std::mem::size_of_val(&input);
            let (mut output, addition) = input
                .into_expanded_descriptor_transport_v3(&mut budget)
                .map_err(|e| fail("transport", e))?;
            if budget.storage() != FLOOR + receipt.retained_storage() {
                return Err(fail("floor", "consuming adapter preserved old paid floor"));
            }
            budget.reserve_storage(addition.retained_storage()).unwrap();
            let module_storage =
                crate::kernel_ir_codegen::nominal_v3::module_storage(output.module(), &mut budget)
                    .map_err(|e| fail("module storage", e))?;
            let expected_transport_addition = std::mem::size_of_val(&output) - input_header
                + module_storage
                - std::mem::size_of::<crate::kernel_ir_codegen::InertCompilerModuleTextV1>();
            if addition.retained_storage() != expected_transport_addition
                || output.retained_storage_floor_v3() != budget.storage()
                || output.descriptor_source().canonical_bytes().as_ptr() != pointer
                || digest(output.descriptor_source().canonical_bytes()) != descriptor
                || *output.output().canonical().identity().digest() != final_graph
                || digest(
                    output
                        .module()
                        .llvm_ir()
                        .get(..prefix_length)
                        .ok_or_else(|| fail("native", "exact full final prefix"))?
                        .as_bytes(),
                ) != native_prefix
            {
                return Err(fail("custody", "exact final graph/backing/module transfer"));
            }
            output
                .expanded_transport_hostiles_v3(&mut budget)
                .map_err(|e| fail("negative native before serialization", e))?;
            let native_floor = budget.storage();
            let native_header = std::mem::size_of_val(&output);
            let native_bytes = digest(output.module().llvm_ir().as_bytes());
            let (output, history_addition) = output
                .into_serialized_expanded_history_v1(&mut budget)
                .map_err(|e| fail("complete history serializer", e))?;
            assert_eq!(budget.storage(), native_floor);
            budget
                .reserve_storage(history_addition.retained_storage())
                .unwrap();
            let mut capacity_reference = Vec::<u8>::new();
            capacity_reference
                .try_reserve_exact(output.history_bytes().len())
                .unwrap();
            let expected_history_addition =
                std::mem::size_of_val(&output) - native_header + capacity_reference.capacity();
            drop(capacity_reference);
            assert_eq!(
                history_addition.retained_storage(),
                expected_history_addition
            );
            assert_eq!(output.retained_storage_floor_v1(), budget.storage());
            assert_eq!(
                output.descriptor_source().canonical_bytes().as_ptr(),
                pointer
            );
            assert_eq!(digest(output.module().llvm_ir().as_bytes()), native_bytes);
            let history = digest(output.history_bytes());
            let history_length = output.history_bytes().len();
            let history_query_floor = budget.storage();
            let history_frame =
                fe2o3_kernel_opt::read_expanded_history_v1(output.history_bytes(), &mut budget)
                    .map_err(|e| fail("history directory", e))?;
            budget
                .reserve_storage(history_frame.storage().retained_storage())
                .unwrap();
            let range = history_frame.final_graph_range();
            let history_final_range = (range.start, range.end);
            let history_rounds = history_frame.scalar().rounds().len();
            let history_payload_exact =
                &output.history_bytes()[range] == output.output().canonical().canonical_bytes();
            assert!(history_payload_exact);
            assert_eq!(history_rounds, rounds);
            drop(history_frame);
            budget
                .release_storage(budget.storage() - history_query_floor)
                .unwrap();
            let query =
                COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3 + 4 * DESCRIPTOR_QUERY_STORAGE_V3;
            budget.reserve_storage(query).unwrap();
            let source = output.descriptor_source();
            let table = source
                .table(
                    source.storage().retained_storage()
                        + COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
                    &mut |n| budget.charge_work(n),
                )
                .map_err(|e| fail("table", e))?;
            if table.kernel_count() != 1 {
                return Err(fail("table", "complete one-root source roster"));
            }
            let row = table
                .kernel(0, &mut |n| budget.charge_work(n))
                .map_err(|e| fail("row", e))?;
            if row.entry_name() != self.case.root() {
                return Err(fail("row", "actual root name"));
            }
            let mut source_kinds = Vec::new();
            let mut cursor = row.arguments();
            while let Some(arg) = cursor
                .next(&mut |n| budget.charge_work(n))
                .map_err(|e| fail("argument", e))?
            {
                if arg.source_index() as usize != source_kinds.len() {
                    return Err(fail("argument", "ordered source ordinal"));
                }
                let ty = table
                    .source_type(arg.source_type(), &mut |n| budget.charge_work(n))
                    .map_err(|e| fail("type", e))?;
                source_kinds.push(match ty.descriptor() {
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Usize => 1,
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Isize => 2,
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Scalar(
                        fe2o3_kernel_descriptor::ScalarTypeV1::U64,
                    ) => 3,
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Scalar(
                        fe2o3_kernel_descriptor::ScalarTypeV1::I64,
                    ) => 4,
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::DisjointSlice(
                        fe2o3_kernel_descriptor::ScalarTypeV1::U64,
                    ) => 5,
                    _ => return Err(fail("type", "exact nominal/fixed source roster")),
                });
            }
            drop(cursor);
            drop(row);
            drop(table);
            budget.release_storage(query).unwrap();
            let retained = budget.storage();
            let replay = |w, p| {
                let mut work = Work::new(w);
                let mut b = Budget::new(&mut work, p);
                b.charge_work(17).unwrap();
                b.reserve_storage(retained).unwrap();
                let result = output.verify_equivalence(&mut b);
                assert_eq!(b.storage(), retained);
                (result, b.work(), b.peak_storage(), b.failed_storage())
            };
            let (full, replay_work, replay_peak, failed) = replay(WORK, STORAGE);
            full.map_err(|e| fail("replay", e))?;
            assert_eq!(failed, None);
            let (exact, w, p, failed) = replay(replay_work, replay_peak);
            exact.map_err(|e| fail("exact replay", e))?;
            assert_eq!((w, p, failed), (replay_work, replay_peak, None));
            let (short, short_work_accepted, _, _) = replay(replay_work - 1, replay_peak);
            let error = short.expect_err("last accepted work boundary");
            assert!(matches!(resource(&error), Some(Resource::Work(_))));
            let (short, _, short_storage_prior_peak, failed) = replay(replay_work, replay_peak - 1);
            let error = short.expect_err("peak storage boundary");
            assert!(matches!(resource(&error), Some(Resource::Storage(_))));
            assert_eq!(failed, Some(replay_peak));
            let report = Observation {
                case: self.case,
                profile: profile.to_owned(),
                callbacks: self.count,
                request: digest(&serde_json::to_vec(&self.request).unwrap()),
                args: digest(&serde_json::to_vec(&self.args).unwrap()),
                fixture: digest(self.case.source().as_bytes()),
                rustc_fixture,
                source: stamps(),
                semantic_roots,
                final_roots,
                erased,
                rounds,
                changing_rounds,
                unrolled,
                final_graph,
                emission_order,
                descriptor,
                descriptor_pointer_retained: output.descriptor_source().canonical_bytes().as_ptr()
                    == pointer,
                native_prefix,
                embedded_native: digest(output.module().llvm_ir().as_bytes()),
                source_kinds,
                input_receipt: receipt.retained_storage(),
                transport_addition: addition.retained_storage(),
                expected_transport_addition,
                history,
                history_length,
                history_addition: history_addition.retained_storage(),
                expected_history_addition,
                history_rounds,
                history_final_range,
                history_payload_exact,
                work: budget.work(),
                peak: budget.peak_storage(),
                replay_work,
                replay_peak,
                short_work_accepted,
                short_storage_prior_peak,
                final_sibling_floor: FLOOR,
                artifact_authority: output.grants_artifact_or_launch_authority(),
                execution_authority: output.authenticates_execution(),
            };
            drop(output);
            budget
                .release_storage(history_addition.retained_storage())
                .unwrap();
            budget.release_storage(addition.retained_storage()).unwrap();
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            Ok(report)
        })());
        Compilation::Stop
    }
}

fn run_child(case: Case) {
    let request_path = PathBuf::from(env::var_os(CHILD_ARGS).expect("managed source request"));
    let request: Vec<String> =
        serde_json::from_slice(&bounded(&request_path, 128 * 1024).unwrap()).unwrap();
    let generated = request_path.with_file_name(case.file());
    assert!(!generated.exists(), "never overwrite a shared fixture");
    std::fs::write(&generated, case.source()).unwrap();
    let args = arguments(&request, &generated, case);
    let mut callback = ExpandedCallbacks {
        request,
        args: args.clone(),
        generated,
        case,
        count: 0,
        result: None,
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callback);
    })) {
        Ok(()) => callback
            .result
            .unwrap_or_else(|| Err(fail("rustc", "missing callback"))),
        Err(_) => Err(fail("rustc", "compiler/source callback panic")),
    };
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 128 * 1024);
    std::fs::write(
        PathBuf::from(env::var_os(CHILD_RESULT).expect("managed response")),
        &bytes,
    )
    .unwrap();
    println!(
        "EXPANDED_HISTORY_V1_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("EXPANDED_HISTORY_V1_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source refusal is not positive qualification: {result:?}"
    );
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceObservation {
    profile: String,
    callbacks: usize,
    request: [u8; 32],
    args: [u8; 32],
    fixture: [u8; 32],
    source: Vec<(String, [u8; 32])>,
    roots: Vec<(u32, String)>,
    storage_cut: bool,
    probe: [usize; 7],
    setup_work: usize,
    setup_peak: usize,
    final_floor: usize,
}
struct ResourceCallbacks {
    request: Vec<String>,
    args: Vec<String>,
    generated: PathBuf,
    storage_cut: bool,
    count: usize,
    result: Option<R<ResourceObservation>>,
}
impl Callbacks for ResourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some((|| {
            if self.count != 1 {
                return Err(fail("resource rustc", "one callback"));
            }
            let fixture = checked_fixture(tcx, &self.generated, Case::Noop)?;
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
                return Err(fail("resource target", "actual rustc target/argv"));
            }
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail("resource collection", e))?;
            transaction.reset_expanded_source_trace_v3();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let (input, receipt) = transaction
                .prepare_expanded_native_v3(&mut budget)
                .map_err(|e| fail("resource genuine factory", e))?;
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (erased, rounds, changes, unrolled, final_graph, order) =
                input.expanded_source_observation_v3();
            if erased
                || rounds == 0
                || rounds > 16
                || changes != 0
                || unrolled != final_graph
                || order != (4, [1, 2, 3, 4])
            {
                return Err(fail("resource source shape", "actual normal Direct no-op"));
            }
            let roots = input.expanded_source_ordered_roots_v3();
            if roots.len() != 1 || roots[0].1 != Case::Noop.root() {
                return Err(fail("resource roots", "exact actual source roster"));
            }
            let (transport, addition) = input
                .into_expanded_descriptor_transport_v3(&mut budget)
                .map_err(|e| fail("resource actual transport", e))?;
            budget.reserve_storage(addition.retained_storage()).unwrap();
            let live = budget.storage();
            let probe = transport.expanded_history_first_denial_for_test(live, self.storage_cut);
            // The consuming denial retired no caller credits; both old receipts
            // are retired once, now that their genuine owner has been dropped.
            assert_eq!(budget.storage(), live);
            budget.release_storage(addition.retained_storage()).unwrap();
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            Ok(ResourceObservation {
                profile: profile.to_owned(),
                callbacks: self.count,
                request: digest(&serde_json::to_vec(&self.request).unwrap()),
                args: digest(&serde_json::to_vec(&self.args).unwrap()),
                fixture,
                source: stamps(),
                roots,
                storage_cut: self.storage_cut,
                probe,
                setup_work: budget.work(),
                setup_peak: budget.peak_storage(),
                final_floor: budget.storage(),
            })
        })());
        Compilation::Stop
    }
}
const RESOURCE_WORK_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::expanded_native_v3::serialized_history::expanded_history_resource_work_child";
const RESOURCE_STORAGE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::expanded_native_v3::serialized_history::expanded_history_resource_storage_child";
fn resource_child(storage_cut: bool) {
    let path = PathBuf::from(env::var_os(CHILD_ARGS).expect("managed resource request"));
    let request: Vec<String> =
        serde_json::from_slice(&bounded(&path, 128 * 1024).unwrap()).unwrap();
    let generated = path.with_file_name(Case::Noop.file());
    assert!(!generated.exists());
    std::fs::write(&generated, Case::Noop.source()).unwrap();
    let args = arguments(&request, &generated, Case::Noop);
    let mut callback = ResourceCallbacks {
        request,
        args: args.clone(),
        generated,
        storage_cut,
        count: 0,
        result: None,
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callback)
    })) {
        Ok(()) => callback
            .result
            .unwrap_or_else(|| Err(fail("resource rustc", "no callback"))),
        Err(_) => Err(fail("resource rustc", "panic")),
    };
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 128 * 1024);
    std::fs::write(
        PathBuf::from(env::var_os(CHILD_RESULT).expect("managed response")),
        &bytes,
    )
    .unwrap();
    println!(
        "EXPANDED_HISTORY_RESOURCE_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("EXPANDED_HISTORY_RESOURCE_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source admission cannot be substituted with a refusal: {result:?}"
    );
}
#[test]
#[ignore = "managed actual source consuming-work oracle"]
fn expanded_history_resource_work_child() {
    resource_child(false);
}
#[test]
#[ignore = "managed actual source consuming-storage oracle"]
fn expanded_history_resource_storage_child() {
    resource_child(true);
}
fn check_resource(path: &Path, roots: &[&str], storage_cut: bool) {
    assert_eq!(roots, guarded_loop_read::Case::Control.roots());
    let result: R<ResourceObservation> =
        serde_json::from_slice(&bounded(path, 128 * 1024).unwrap()).unwrap();
    let r = result.expect("genuine source admission plus exact typed consuming denial");
    let request_path = path.with_file_name(format!(
        "{}-args.json",
        guarded_loop_read::Case::Control.feature()
    ));
    let request: Vec<String> =
        serde_json::from_slice(&bounded(&request_path, 128 * 1024).unwrap()).unwrap();
    let generated = path.with_file_name(Case::Noop.file());
    assert_eq!(
        bounded(&generated, 8192).unwrap(),
        Case::Noop.source().as_bytes()
    );
    let args = arguments(&request, &generated, Case::Noop);
    assert_eq!(r.callbacks, 1);
    assert_eq!(r.request, digest(&serde_json::to_vec(&request).unwrap()));
    assert_eq!(r.args, digest(&serde_json::to_vec(&args).unwrap()));
    assert_eq!(r.fixture, digest(Case::Noop.source().as_bytes()));
    assert_eq!(r.source, stamps());
    assert_eq!(
        args.iter()
            .filter(|a| **a == format!("-Ctarget-cpu={}", r.profile))
            .count(),
        1
    );
    assert_eq!(r.roots.len(), 1);
    assert_eq!(r.roots[0].1, Case::Noop.root());
    assert_eq!(r.storage_cut, storage_cut);
    let [guard, floor, work, peak, failed, limit, work_limit] = r.probe;
    assert!(guard > 0 && floor > FLOOR);
    assert_eq!(work, 0);
    if storage_cut {
        assert_eq!(
            (peak, failed, limit, work_limit),
            (floor, floor + guard, floor + guard - 1, 100)
        );
    } else {
        assert_eq!(
            (peak, failed, limit, work_limit),
            (floor + guard, 0, STORAGE, 2)
        );
    }
    assert!(r.setup_work > 0 && r.setup_work <= WORK && r.setup_peak <= STORAGE);
    assert_eq!(r.final_floor, FLOOR);
}
fn check_resource_work(path: &Path, roots: &[&str]) {
    check_resource(path, roots, false);
}
fn check_resource_storage(path: &Path, roots: &[&str]) {
    check_resource(path, roots, true);
}
#[test]
#[ignore = "genuine source consuming first-failure oracle on both profiles"]
fn ordinary_rust_expanded_history_resource_prefix() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for storage in [false, true] {
            ordinary_rust_source_cases(
                &[OrdinarySourceCase::GuardedLoopRead(
                    guarded_loop_read::Case::Control,
                )],
                profile,
                false,
                Some(SourceObserver {
                    child_test: if storage {
                        RESOURCE_STORAGE_CHILD
                    } else {
                        RESOURCE_WORK_CHILD
                    },
                    check: if storage {
                        check_resource_storage
                    } else {
                        check_resource_work
                    },
                }),
            );
        }
    }
}
#[test]
fn expanded_history_resource_protocol_requires_complete_fields_and_distinct_children() {
    assert!(serde_json::from_value::<ResourceObservation>(serde_json::json!({})).is_err());
    assert_ne!(RESOURCE_WORK_CHILD, RESOURCE_STORAGE_CHILD);
    assert_ne!(RESOURCE_WORK_CHILD, NOOP_CHILD);
    assert_ne!(RESOURCE_STORAGE_CHILD, REWRITE_CHILD);
}
#[test]
#[ignore = "managed actual Direct normal source parent"]
fn expanded_noop_source_child() {
    run_child(Case::Noop);
}
#[test]
#[ignore = "managed actual Erased opt0 rewrite source parent"]
fn expanded_rewrite_source_child() {
    run_child(Case::Rewrite);
}

fn check(path: &Path, roots: &[&str], case: Case) {
    assert_eq!(roots, guarded_loop_read::Case::Control.roots());
    let result: R<Observation> =
        serde_json::from_slice(&bounded(path, 128 * 1024).unwrap()).unwrap();
    let report = result.expect("genuine actual-source final native success required");
    let request_path = path.with_file_name(format!(
        "{}-args.json",
        guarded_loop_read::Case::Control.feature()
    ));
    let request: Vec<String> =
        serde_json::from_slice(&bounded(&request_path, 128 * 1024).unwrap()).unwrap();
    let generated = path.with_file_name(case.file());
    assert_eq!(bounded(&generated, 8192).unwrap(), case.source().as_bytes());
    let actual = arguments(&request, &generated, case);
    assert_eq!(report.case, case);
    assert_eq!(report.callbacks, 1);
    assert_eq!(
        report.request,
        digest(&serde_json::to_vec(&request).unwrap())
    );
    assert_eq!(report.args, digest(&serde_json::to_vec(&actual).unwrap()));
    assert_eq!(report.fixture, digest(case.source().as_bytes()));
    assert_eq!(report.rustc_fixture, report.fixture);
    assert_eq!(report.source, stamps());
    assert_eq!(
        actual
            .iter()
            .filter(|s| **s == format!("-Ctarget-cpu={}", report.profile))
            .count(),
        1
    );
    assert_eq!(report.semantic_roots.len(), 1);
    assert_eq!(report.semantic_roots[0].1, case.root());
    assert_eq!(report.final_roots, [case.root()]);
    assert_eq!(report.emission_order, (4, [1, 2, 3, 4]));
    assert!(report.rounds > 0 && report.rounds <= 16);
    match case {
        Case::Noop => {
            assert!(!report.erased);
            assert_eq!(report.changing_rounds, 0);
            assert_eq!(report.unrolled, report.final_graph);
            assert!(report.source_kinds.is_empty());
            assert!(!actual.iter().any(|s| s.starts_with("-Zmir-opt-level=")));
        }
        Case::Rewrite => {
            assert!(report.erased && report.changing_rounds > 0);
            assert_ne!(report.unrolled, report.final_graph);
            assert_eq!(report.source_kinds, [1, 2, 3, 4, 5]);
            assert_eq!(
                actual
                    .iter()
                    .filter(|s| s.as_str() == "-Zmir-opt-level=0")
                    .count(),
                1
            );
        }
    }
    for hash in [
        report.descriptor,
        report.native_prefix,
        report.embedded_native,
        report.final_graph,
        report.unrolled,
    ] {
        assert_ne!(hash, [0; 32]);
    }
    assert_ne!(report.native_prefix, report.embedded_native);
    assert!(report.descriptor_pointer_retained);
    assert_eq!(
        report.transport_addition,
        report.expected_transport_addition
    );
    assert!(report.input_receipt > 0 && report.transport_addition > 0);
    assert_ne!(report.history, [0; 32]);
    assert!(report.history_length > 48);
    assert_eq!(report.history_rounds, report.rounds);
    assert_eq!(report.history_addition, report.expected_history_addition);
    assert!(report.history_addition > report.history_length);
    assert!(report.history_payload_exact);
    assert!(report.history_final_range.0 < report.history_final_range.1);
    assert!(report.history_final_range.1 <= report.history_length);
    assert!(report.work > 17 && report.work <= WORK && report.peak <= STORAGE);
    assert!(report.replay_work > 17 && report.short_work_accepted < report.replay_work);
    assert!(report.short_storage_prior_peak < report.replay_peak && report.replay_peak <= STORAGE);
    assert_eq!(report.final_sibling_floor, FLOOR);
    assert!(!report.artifact_authority && !report.execution_authority);
}
fn check_noop(path: &Path, roots: &[&str]) {
    check(path, roots, Case::Noop);
}
fn check_rewrite(path: &Path, roots: &[&str]) {
    check(path, roots, Case::Rewrite);
}
fn parent(case: Case) {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[OrdinarySourceCase::GuardedLoopRead(
                guarded_loop_read::Case::Control,
            )],
            profile,
            false,
            Some(SourceObserver {
                child_test: if case == Case::Noop {
                    NOOP_CHILD
                } else {
                    REWRITE_CHILD
                },
                check: if case == Case::Noop {
                    check_noop
                } else {
                    check_rewrite
                },
            }),
        );
    }
}
#[test]
#[ignore = "pinned rust-src and actual normal Direct source on both AMD profiles"]
fn ordinary_rust_expanded_history_noop_roundtrip() {
    parent(Case::Noop);
}
#[test]
#[ignore = "pinned rust-src and actual Erased nonvacuous scalar-final source on both AMD profiles"]
fn ordinary_rust_expanded_history_rewrite_roundtrip() {
    parent(Case::Rewrite);
}

#[test]
fn expanded_source_roster_has_four_required_children_and_no_fallback_result() {
    let mut roster = std::collections::BTreeSet::new();
    for profile in ["gfx942", "gfx950"] {
        assert!(roster.insert((profile, NOOP_CHILD)));
        assert!(roster.insert((profile, REWRITE_CHILD)));
    }
    assert_eq!(roster.len(), 4);
    assert_ne!(NOOP_CHILD, REWRITE_CHILD);
    assert!(NOOP.contains("pub fn expanded_raw_noop()"));
    assert!(REWRITE.contains("while cursor < 3"));
    assert_ne!(digest(NOOP.as_bytes()), digest(REWRITE.as_bytes()));
}
#[test]
fn expanded_source_protocol_rejects_missing_and_unknown_fields() {
    assert!(serde_json::from_value::<Observation>(serde_json::json!({})).is_err());
    assert!(
        serde_json::from_value::<Failure>(serde_json::json!({
            "phase": "source", "detail": "refused", "accepted": true,
        }))
        .is_err()
    );
    let refusal = Failure {
        phase: "source".to_owned(),
        detail: "missing callback".to_owned(),
    };
    let bytes = serde_json::to_vec(&Err::<Observation, _>(refusal)).unwrap();
    assert!(
        serde_json::from_slice::<R<Observation>>(&bytes)
            .unwrap()
            .is_err()
    );
}
