//! Actual ordinary Rust-to-U-to-LLVM and nominal V3, never a signed artifact.
use super::*;
use fe2o3_kernel_descriptor::{PhysicalAbiComponentKind, ScalarTypeV1};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::nominal_unroll_source_child";
const MIXED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::native_unroll::nominal_unroll_mixed_source_child";
const MIXED_ROOT: &str = "nominal_literal_mixed";
const MIXED_SOURCE: &str = r#"#![no_std]
use fe2o3_device::{DisjointSlice, kernel, thread};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]), control_flow(loop_bounds(3)))]
pub fn nominal_literal_mixed(seed: usize, delta: isize, wide: u64, signed: i64, mut output: DisjointSlice<u64>) {
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeObservation {
    case: Case,
    mixed: bool,
    profile: String,
    callback_count: usize,
    request_sha256: [u8; 32],
    actual_args_sha256: [u8; 32],
    generated_source_sha256: Option<[u8; 32]>,
    source: Vec<Stamp>,
    descriptor: [u8; 32],
    descriptor_bytes: usize,
    llvm: [u8; 32],
    llvm_bytes: usize,
    original: [u8; 32],
    forwarding: [u8; 32],
    unrolled: [u8; 32],
    erased: bool,
    selected_iterations: Option<u8>,
    source_kinds: [u8; 5],
    component_counts: [usize; 5],
    work: usize,
    peak: usize,
    receipt: usize,
    floor: usize,
    artifact_authority: bool,
    execution_authority: bool,
    replay_work: usize,
    replay_peak: usize,
    short_storage_work: usize,
    short_storage_prior_peak: usize,
    short_storage_error: String,
}

const TEST_LEAF: &str =
    "crates/rustc-codegen-fe2o3/src/production_rustc_driver_nominal_loop_unroll_v3_tests.rs";
fn stamps() -> Vec<Stamp> {
    let mut stamps = source_stamps();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    stamps.push(Stamp {
        path: TEST_LEAF.to_owned(),
        sha256: digest(&std::fs::read(workspace.join(TEST_LEAF)).unwrap()),
    });
    stamps
}

fn expected_kinds(mixed: bool) -> [u8; 5] {
    if mixed {
        [5, 6, 7, 8, 9]
    } else {
        [2, 5, 3, 0, 0]
    }
}
fn expected_components(mixed: bool) -> [usize; 5] {
    if mixed {
        [1, 1, 1, 1, 2]
    } else {
        [2, 1, 2, 0, 0]
    }
}

struct NativeCallbacks {
    args: Vec<String>,
    request: Vec<String>,
    case: Case,
    mixed: bool,
    callbacks: usize,
    result: Option<R<NativeObservation>>,
}
impl Callbacks for NativeCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            if self.callbacks != 1 {
                return Err(fail(Phase::Rustc, "exactly one actual callback"));
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
                return Err(fail(Phase::Request, "actual target/invocation mismatch"));
            }
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail(Phase::Collection, e))?;
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(17).unwrap();
            // The returned two observation arrays and topology fit in this
            // explicit caller floor, with a nonzero sibling retained as well.
            const FLOOR: usize = 113;
            assert!(
                FLOOR
                    > std::mem::size_of::<[u8; 5]>()
                        + std::mem::size_of::<[usize; 5]>()
                        + std::mem::size_of::<(bool, Option<u8>)>()
            );
            budget.reserve_storage(FLOOR).unwrap();
            let (mut owner, receipt) = transaction
                .prepare_nominal_loop_unroll_native_v3(
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    &mut budget,
                )
                .map_err(|e| fail(Phase::Factory, e))?;
            if budget.storage() != FLOOR
                || owner.retained_storage_floor_v1() != FLOOR + receipt.retained_storage()
            {
                return Err(fail(
                    Phase::Floor,
                    "complete factory floor/receipt mismatch",
                ));
            }
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(|e| fail(Phase::Floor, e))?;
            let floor = budget.storage();
            owner
                .verify_equivalence(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            if !owner.llvm_ir().contains("define amdgpu_kernel")
                || owner.llvm_ir().contains(".fe2o3.kd.v1")
            {
                return Err(fail(
                    Phase::Rows,
                    "inert final-U LLVM without V1 descriptor laundering",
                ));
            }
            let (source_kinds, component_counts) = owner
                .with_checked_table(&mut budget, |table, budget| {
                    const CALLER: usize =
                        std::mem::size_of::<
                            fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>,
                        >() + std::mem::size_of::<
                            fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>,
                        >() + std::mem::size_of::<
                            fe2o3_kernel_descriptor::LogicalArgumentRefV3<'static, 'static>,
                        >() + std::mem::size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV3>()
                            + std::mem::size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
                            + std::mem::size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>()
                            + std::mem::size_of::<[u8; 5]>()
                            + std::mem::size_of::<[usize; 5]>();
                    budget.reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3 + CALLER)?;
                    if table.kernel_count() != 1
                        || table.device_target().as_amd_target_id().processor() != profile
                    {
                        return Err(E::Mismatch("actual root/target descriptor roster"));
                    }
                    let kernel = table
                        .kernel(0, &mut |w| budget.charge_work(w))
                        .map_err(E::Wire)?;
                    let count = if self.mixed { 5 } else { 3 };
                    let name = if self.mixed {
                        MIXED_ROOT
                    } else {
                        self.case.roots()[0]
                    };
                    if kernel.entry_name() != name || kernel.argument_count() != count {
                        return Err(E::Mismatch("actual fixture root/signature"));
                    }
                    let mut kinds = [0; 5];
                    let mut counts = [0; 5];
                    let mut cursor = kernel.arguments();
                    for i in 0..count {
                        let argument = cursor
                            .next(&mut |w| budget.charge_work(w))
                            .map_err(E::Wire)?
                            .ok_or(E::Mismatch("complete actual argument cursor"))?;
                        if argument.source_index() as usize != i {
                            return Err(E::Mismatch("actual source ordinal"));
                        }
                        let source = table
                            .source_type(argument.source_type(), &mut |w| budget.charge_work(w))
                            .map_err(E::Wire)?;
                        kinds[i] = match source.descriptor() {
                            Kind::SharedSlice(ScalarTypeV1::U32) => 2,
                            Kind::DisjointSlice(ScalarTypeV1::U32) => 3,
                            Kind::Usize => 5,
                            Kind::Isize => 6,
                            Kind::Scalar(ScalarTypeV1::U64) => 7,
                            Kind::Scalar(ScalarTypeV1::I64) => 8,
                            Kind::DisjointSlice(ScalarTypeV1::U64) => 9,
                            _ => {
                                return Err(E::Mismatch("genuine nominal/fixed/slice source kind"));
                            }
                        };
                        counts[i] = argument.component_count();
                        if matches!(kinds[i], 5..=8) {
                            let layout = table
                                .device_layout(argument.device_layout(), &mut |w| {
                                    budget.charge_work(w)
                                })
                                .map_err(E::Wire)?;
                            let component = argument
                                .component(0, &mut |w| budget.charge_work(w))
                                .map_err(E::Wire)?;
                            let scalar = if matches!(kinds[i], 6 | 8) {
                                ScalarTypeV1::I64
                            } else {
                                ScalarTypeV1::U64
                            };
                            let offset = if self.mixed { i as u32 * 8 } else { 16 };
                            if layout.descriptor().size_bytes() != 8
                                || layout.descriptor().alignment_bytes() != 8
                                || component.kind != PhysicalAbiComponentKind::ScalarByValue(scalar)
                                || component.offset != offset
                                || component.size != 8
                                || component.alignment != 8
                            {
                                return Err(E::Mismatch(
                                    "nominal source and exact physical packing",
                                ));
                            }
                        }
                    }
                    if cursor
                        .next(&mut |w| budget.charge_work(w))
                        .map_err(E::Wire)?
                        .is_some()
                    {
                        return Err(E::Mismatch("no extra argument"));
                    }
                    Ok((kinds, counts))
                })
                .map_err(|e| fail(Phase::Rows, e))?;
            if source_kinds != expected_kinds(self.mixed)
                || component_counts != expected_components(self.mixed)
                || budget.storage() != floor
            {
                return Err(fail(Phase::Rows, "complete shape and preserved floor"));
            }
            let (erased, selected_iterations) = owner.source_test_selection_v3();
            if self.mixed
                && (selected_iterations != Some(3)
                    || owner.forwarding_output().canonical().identity()
                        == owner.output().canonical().identity())
            {
                return Err(fail(
                    Phase::Rows,
                    "literal-trip actual source must select three nonzero copies",
                ));
            }
            owner
                .source_test_mutations_v3(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            let replay = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.charge_work(17).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = owner.verify_equivalence(&mut budget);
                assert_eq!(budget.storage(), floor);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            let (full, replay_work, replay_peak, failed) = replay(WORK, STORAGE);
            full.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!(failed, None);
            assert!(replay_peak > floor);
            let (exact, ew, ep, failed) = replay(replay_work, replay_peak);
            exact.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!((ew, ep, failed), (replay_work, replay_peak, None));
            let (short, accepted, _, failed) = replay(replay_work - 1, replay_peak);
            let Err(E::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(e),
            )) = short
            else {
                return Err(fail(
                    Phase::Replay,
                    "typed final one-unit replay Work refusal",
                ));
            };
            assert_eq!(
                (e.actual(), e.limit(), accepted, failed),
                (replay_work, replay_work - 1, replay_work - 1, None)
            );
            let (short, short_storage_work, short_storage_prior_peak, failed) =
                replay(replay_work, replay_peak - 1);
            assert!(short.is_err());
            assert_eq!(failed, Some(replay_peak));
            // Record the observed phase. A later strict oracle must name it.
            let short_storage_error = format!("{short:?}");
            let result = NativeObservation {
                case: self.case,
                mixed: self.mixed,
                profile: profile.to_owned(),
                callback_count: self.callbacks,
                request_sha256: digest(&serde_json::to_vec(&self.request).unwrap()),
                actual_args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                generated_source_sha256: self.mixed.then(|| digest(MIXED_SOURCE.as_bytes())),
                source: stamps(),
                descriptor: digest(owner.canonical_bytes()),
                descriptor_bytes: owner.canonical_bytes().len(),
                llvm: digest(owner.llvm_ir().as_bytes()),
                llvm_bytes: owner.llvm_ir().len(),
                original: *owner
                    .original()
                    .map_err(|e| fail(Phase::Rows, e))?
                    .canonical()
                    .identity()
                    .digest(),
                forwarding: *owner.forwarding_output().canonical().identity().digest(),
                unrolled: *owner.output().canonical().identity().digest(),
                erased,
                selected_iterations,
                source_kinds,
                component_counts,
                work: budget.work(),
                peak: budget.peak_storage(),
                receipt: receipt.retained_storage(),
                floor,
                artifact_authority: owner.grants_artifact_or_launch_authority(),
                execution_authority: owner.authenticates_execution(),
                replay_work,
                replay_peak,
                short_storage_work,
                short_storage_prior_peak,
                short_storage_error,
            };
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            if budget.storage() != FLOOR {
                return Err(fail(Phase::Floor, "drop before receipt refund"));
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}

fn generated_args(request: &[String], generated: &Path) -> Vec<String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original = workspace
        .join(format!("{BASE}/src/lib.rs"))
        .canonicalize()
        .unwrap();
    let mut replacements = 0;
    let args = request
        .iter()
        .map(|arg| {
            if Path::new(arg).canonicalize().ok().as_ref() == Some(&original) {
                replacements += 1;
                generated.display().to_string()
            } else {
                arg.clone()
            }
        })
        .collect();
    assert_eq!(replacements, 1);
    args
}

fn run_child(mixed: bool) {
    let request_path = PathBuf::from(env::var_os(CHILD_ARGS).expect("actual parent request"));
    let request: Vec<String> =
        serde_json::from_slice(&std::fs::read(&request_path).unwrap()).unwrap();
    let result = request_case(&request).and_then(|case| {
        if mixed && case != Case::Control {
            return Err(fail(Phase::Request, "exact mixed parent case"));
        }
        let generated = request_path.with_file_name("nominal-literal-mixed.rs");
        let args = if mixed {
            assert!(
                !generated.exists(),
                "never overwrite a pre-existing fixture"
            );
            std::fs::write(&generated, MIXED_SOURCE).map_err(|e| fail(Phase::Request, e))?;
            if std::fs::read(&generated).map_err(|e| fail(Phase::Request, e))?
                != MIXED_SOURCE.as_bytes()
            {
                return Err(fail(Phase::Request, "exact generated Rust source"));
            }
            generated_args(&request, &generated)
        } else {
            request.clone()
        };
        require_canonical_overflow_checks_v1(&args).map_err(|e| fail(Phase::Request, e))?;
        let mut callback = NativeCallbacks {
            args: args.clone(),
            request,
            case,
            mixed,
            callbacks: 0,
            result: None,
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rustc_driver::run_compiler(&args, &mut callback)
        })) {
            Ok(()) => callback
                .result
                .unwrap_or_else(|| Err(fail(Phase::Rustc, "callback absent"))),
            Err(_) => Err(fail(Phase::Rustc, "actual compiler panicked")),
        }
    });
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    std::fs::write(
        PathBuf::from(env::var_os(CHILD_RESULT).expect("actual parent response")),
        &bytes,
    )
    .unwrap();
    println!(
        "NOMINAL_UNROLL_V3_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("NOMINAL_UNROLL_V3_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source refusal is not qualification: {result:?}"
    );
}

#[test]
#[ignore = "genuine parent-managed ordinary rustc child, not a no-environment success"]
fn nominal_unroll_source_child() {
    run_child(false);
}
#[test]
#[ignore = "genuine parent-managed literal/mixed rustc child, not a no-environment success"]
fn nominal_unroll_mixed_source_child() {
    run_child(true);
}

fn check_native(path: &Path, roots: &[&str], mixed: bool) {
    let result: R<NativeObservation> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let report = result.expect("actual nominal source-to-U-to-native continuation must succeed");
    assert_eq!(report.mixed, mixed);
    assert_eq!(report.case.roots(), roots);
    assert_eq!(report.callback_count, 1);
    assert_eq!(report.source, stamps());
    let args_path = path.with_file_name(format!("{}-args.json", report.case.feature()));
    let request: Vec<String> = serde_json::from_slice(&std::fs::read(&args_path).unwrap()).unwrap();
    assert_eq!(request_case(&request).unwrap(), report.case);
    assert_eq!(
        report.request_sha256,
        digest(&serde_json::to_vec(&request).unwrap())
    );
    let actual = if mixed {
        let generated = path.with_file_name("nominal-literal-mixed.rs");
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
            .filter(|a| **a == format!("-Ctarget-cpu={}", report.profile))
            .count(),
        1
    );
    assert_eq!(report.source_kinds, expected_kinds(mixed));
    assert_eq!(report.component_counts, expected_components(mixed));
    for hash in [
        report.descriptor,
        report.llvm,
        report.original,
        report.forwarding,
        report.unrolled,
    ] {
        assert_ne!(hash, [0; 32]);
    }
    assert!(report.descriptor_bytes > 48 && report.llvm_bytes > 100);
    assert!(report.work > 17 && report.peak >= report.floor);
    assert_eq!(report.floor, 113 + report.receipt);
    assert!(!report.artifact_authority && !report.execution_authority);
    assert!(report.replay_work > 17 && report.replay_peak > report.floor);
    assert!(report.short_storage_error.starts_with("Err("));
}
fn check_ordinary(path: &Path, roots: &[&str]) {
    check_native(path, roots, false);
}
fn check_mixed(path: &Path, roots: &[&str]) {
    check_native(path, roots, true);
}

#[test]
#[ignore = "requires pinned rust-src and eight actual ordinary AMD rustc children"]
fn ordinary_rust_nominal_unroll_v3_reaches_actual_final_native() {
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
#[ignore = "requires pinned rust-src and two literal-trip mixed usize/isize AMD rustc children"]
fn ordinary_rust_nominal_unroll_v3_literal_mixed_signature_is_nonzero() {
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
fn nominal_unroll_native_request_has_separate_eight_ordinary_and_two_literal_children() {
    let mut roster = std::collections::BTreeSet::new();
    for profile in ["gfx942", "gfx950"] {
        for case in CASES {
            assert!(roster.insert((profile, CHILD, case.feature())));
        }
        assert!(roster.insert((profile, MIXED_CHILD, Case::Control.feature())));
    }
    assert_eq!(roster.len(), 10);
    assert_ne!(CHILD, MIXED_CHILD);
    assert!(MIXED_SOURCE.contains("delta: isize") && MIXED_SOURCE.contains("while cursor < 3"));
}

#[test]
fn nominal_unroll_native_request_does_not_conflate_nominal_and_fixed_types() {
    assert_eq!(expected_kinds(true), [5, 6, 7, 8, 9]);
    assert_eq!(expected_components(true), [1, 1, 1, 1, 2]);
    assert_eq!(expected_kinds(false), [2, 5, 3, 0, 0]);
    for case in CASES {
        assert_eq!(selected_feature(&feature_args(case)).unwrap(), case);
    }
}
