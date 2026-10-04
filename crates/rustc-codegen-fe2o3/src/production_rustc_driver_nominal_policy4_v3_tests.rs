//! Actual ordinary rustc -> original source -> P4 O -> nominal native transport.
//! Source-to-native-text success is not protected execution or GPU qualification.
use super::*;
use crate::compiler_descriptor::nominal_v3::scoped;
use fe2o3_kernel_descriptor::{BuildEvidenceV1, EvidenceDigest, EvidenceIdentity};

const P4_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::policy4::nominal_policy4_source_child";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum P4Case {
    Direct(Case),
    ErasedControl,
}
impl P4Case {
    fn feature(self) -> &'static str {
        match self {
            Self::Direct(case) => case.feature(),
            Self::ErasedControl => "nominal-policy4-unitlocal",
        }
    }
    fn baseline(self) -> Case {
        match self {
            Self::Direct(case) => case,
            Self::ErasedControl => Case::Control,
        }
    }
    fn roots(self) -> &'static [&'static str] {
        self.baseline().roots()
    }
}
const ERASED_FLAGS: [&str; 3] = [
    "--cfg=feature=\"nominal-policy4-unitlocal\"",
    "-Zinline-mir=no",
    "-Zmir-opt-level=0",
];
fn p4_request_case(args: &[String]) -> R<P4Case> {
    if !args.iter().any(|arg| arg == ERASED_FLAGS[0]) {
        return super::request_case(args).map(P4Case::Direct);
    }
    if ERASED_FLAGS
        .iter()
        .any(|flag| args.iter().filter(|arg| arg.as_str() == *flag).count() != 1)
    {
        return Err(fail(Phase::Request, "exact retained-MIR P4 extension"));
    }
    // Normalize only for the unchanged request validator. The original args
    // are metadata-bound, hashed, and passed to rustc without modification.
    let mut base: Vec<_> = args
        .iter()
        .filter(|arg| !ERASED_FLAGS.contains(&arg.as_str()))
        .cloned()
        .collect();
    base.push(format!("--cfg=feature=\"{}\"", Case::Control.feature()));
    match super::request_case(&base)? {
        Case::Control => Ok(P4Case::ErasedControl),
        _ => Err(fail(Phase::Request, "P4 control baseline")),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputObservation {
    case: P4Case,
    profile: String,
    callbacks: usize,
    erased: bool,
    erasure: Option<[[u8; 32]; 2]>,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
    descriptor: [u8; 32],
    output: [u8; 32],
    native_module: [u8; 32],
    native_addition: usize,
    factory_failures: usize,
    receipt: usize,
    floor: usize,
    work: usize,
    peak: usize,
    native_authority: bool,
    artifact_authority: bool,
    unsigned_source_proof_refused: bool,
}

struct OutputCallbacks {
    args: Vec<String>,
    case: P4Case,
    callbacks: usize,
    result: Option<R<OutputObservation>>,
}
impl Callbacks for OutputCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            if self.callbacks != 1 {
                return Err(fail(Phase::Rustc, "one actual callback"));
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
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail(Phase::Collection, e))?;
            let work_limit =
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
            let storage_limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(53).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (owner, receipt) = transaction
                .prepare_nominal_policy4_abi_v3(&mut budget)
                .map_err(|e| fail(Phase::Factory, e))?;
            assert_eq!(budget.storage(), 53);
            assert_eq!(
                owner.retained_storage_floor_v1(),
                53 + receipt.retained_storage()
            );
            assert!(budget.work_ledger_identity_v1() == ledger);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            owner
                .verify_equivalence(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!(budget.storage(), floor);
            let erased = owner.source_test_is_erased_v3();
            assert!(!owner.grants_artifact_or_launch_authority());
            assert!(!owner.authenticates_execution());
            assert_eq!(erased, self.case == P4Case::ErasedControl);
            let erasure = erased.then(|| owner.source_test_erasure_v3());
            let (source, obligations) = owner.source_test_formal_views_v3();
            super::super::guarded_loop_read::check_nominal_output(
                self.case.baseline(),
                source,
                owner.output(),
                obligations,
                floor,
            )
            .map_err(|e| fail(Phase::Rows, e))?;
            owner
                .with_checked_table(&mut budget, |table, budget| {
                    scoped(budget, |budget| {
                        budget.reserve_storage(
                            DESCRIPTOR_QUERY_STORAGE_V3
                                + std::mem::size_of::<
                                    fe2o3_kernel_descriptor::KernelDescriptorRefV3<
                                        'static,
                                        'static,
                                    >,
                                >()
                                + std::mem::size_of::<
                                    fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>,
                                >()
                                + std::mem::size_of::<
                                    fe2o3_kernel_descriptor::LogicalArgumentRefV3<'static, 'static>,
                                >()
                                + std::mem::size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV3>()
                                + std::mem::size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
                                + std::mem::size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>()
                                + std::mem::size_of::<Sha256>()
                                + 2 * std::mem::size_of::<BuildEvidenceV1>()
                                + 2 * std::mem::size_of::<[u8; 32]>()
                                + 3 * std::mem::size_of::<[u8; 8]>(),
                        )?;
                        if table.kernel_count() != 1
                            || table.device_target().as_amd_target_id().processor() != profile
                        {
                            return Err(E::Mismatch("actual P4 target/root roster"));
                        }
                        let kernel = table
                            .kernel(0, &mut |w| budget.charge_work(w))
                            .map_err(E::Wire)?;
                        if kernel.entry_name() != self.case.roots()[0]
                            || kernel.argument_count() != 3
                        {
                            return Err(E::Mismatch("actual P4 source root/signature"));
                        }
                        let output = owner.output().canonical().canonical_bytes();
                        let domain = b"FE2O3/NOMINAL-POLICY4-OUTPUT-EXECUTABLE-ABI/V3\0";
                        budget.charge_work(domain.len() + 184 + output.len())?;
                        let binding = kernel.kernel_id();
                        let mut hash = Sha256::new();
                        hash.update(domain);
                        hash.update(2u64.to_le_bytes());
                        hash.update(32u64.to_le_bytes());
                        hash.update(binding.as_bytes());
                        hash.update((output.len() as u64).to_le_bytes());
                        hash.update(output);
                        let expected: [u8; 32] = hash.finalize().into();
                        if kernel.executable_ir_evidence() != BuildEvidenceV1::new(
                            EvidenceIdentity::from_opaque_bytes(expected),
                            EvidenceDigest::from_sha256_bytes(expected),
                        ) {
                            return Err(E::Mismatch("independently hashed actual P4 O evidence"));
                        }
                        let expected = [
                            Kind::SharedSlice(fe2o3_kernel_descriptor::ScalarTypeV1::U32),
                            Kind::Usize,
                            Kind::DisjointSlice(fe2o3_kernel_descriptor::ScalarTypeV1::U32),
                        ];
                        let mut args = kernel.arguments();
                        for (index, expected) in expected.into_iter().enumerate() {
                            let argument = args
                                .next(&mut |w| budget.charge_work(w))
                                .map_err(E::Wire)?
                                .ok_or(E::Mismatch("complete P4 source arguments"))?;
                            let ty = table
                                .source_type(argument.source_type(), &mut |w| budget.charge_work(w))
                                .map_err(E::Wire)?;
                            if argument.source_index() as usize != index
                                || ty.descriptor() != expected
                                || argument.component_count() != [2, 1, 2][index]
                            {
                                return Err(E::Mismatch(
                                    "actual P4 nominal/physical source argument",
                                ));
                            }
                            if index == 1 {
                                let layout = table.device_layout(argument.device_layout(),
                                    &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                                let component = argument.component(0,
                                    &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                                if layout.descriptor().size_bytes() != 8
                                    || layout.descriptor().alignment_bytes() != 8
                                    || component.kind != fe2o3_kernel_descriptor::PhysicalAbiComponentKind::ScalarByValue(
                                        fe2o3_kernel_descriptor::ScalarTypeV1::U64)
                                    || component.offset != 16 || component.size != 8
                                    || component.alignment != 8
                                {
                                    return Err(E::Mismatch("actual P4 usize exact physical packing"));
                                }
                            }
                        }
                        if args
                            .next(&mut |w| budget.charge_work(w))
                            .map_err(E::Wire)?
                            .is_some()
                        {
                            return Err(E::Mismatch("no extra P4 arguments"));
                        }
                        Ok(())
                    })
                })
                .map_err(|e| fail(Phase::Rows, e))?;
            assert_eq!(budget.storage(), floor);
            let mut short_work = Work::new(work_limit);
            let mut short = Budget::new(&mut short_work, storage_limit);
            short.reserve_storage(floor - 1).unwrap();
            assert!(matches!(
                owner.verify_equivalence(&mut short),
                Err(E::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                ))
            ));
            assert_eq!(short.work(), 0);
            assert_eq!(short.storage(), floor - 1);
            let descriptor = digest(owner.canonical_bytes());
            let descriptor_ptr = owner.canonical_bytes().as_ptr();
            let descriptor_len = owner.canonical_bytes().len();
            let output = digest(owner.output().canonical().canonical_bytes());
            let output_ptr = owner.output().canonical().canonical_bytes().as_ptr();
            let output_len = owner.output().canonical().canonical_bytes().len();
            let (mut native, native_receipt) = owner
                .into_nominal_native_transport_v3(&mut budget)
                .map_err(|e| fail(Phase::Factory, e))?;
            assert_eq!(budget.storage(), floor);
            native.source_test_receipt_v3(
                native_receipt,
                floor,
                [(descriptor_ptr, descriptor_len), (output_ptr, output_len)],
            );
            assert_eq!(
                native.descriptor_source().canonical_bytes().as_ptr(),
                descriptor_ptr
            );
            assert_eq!(
                native.output().canonical().canonical_bytes().as_ptr(),
                output_ptr
            );
            assert_eq!(
                digest(native.descriptor_source().canonical_bytes()),
                descriptor
            );
            assert_eq!(
                digest(native.output().canonical().canonical_bytes()),
                output
            );
            budget
                .reserve_storage(native_receipt.retained_storage())
                .unwrap();
            native
                .verify_equivalence(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            native.source_test_rejections_v3(&mut budget);
            let total_receipt = receipt
                .retained_storage()
                .checked_add(native_receipt.retained_storage())
                .unwrap();
            assert_eq!(native.retained_storage_floor_v3(), 53 + total_receipt);
            assert_eq!(budget.storage(), native.retained_storage_floor_v3());
            assert_eq!(
                native.module().descriptor_binding_version_for_test_v3(),
                Some(3)
            );
            assert_eq!(native.module().kernel_entries().len(), 1);
            assert_eq!(native.module().kernel_entries()[0], self.case.roots()[0]);
            assert!(native.module().llvm_ir().contains(".fe2o3.kd.v3"));
            let factory_failures = if self.case.baseline() == Case::Control {
                native.source_test_consuming_failures_v3(
                    |budget| {
                        let transaction = transaction_in_active_session_v1(
                            tcx,
                            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                        )
                        .expect("fresh genuine source transaction for consuming failure");
                        let (input, receipt) = transaction
                            .prepare_nominal_policy4_abi_v3(budget)
                            .expect("fresh genuine nominal P4 input for consuming failure");
                        (input, receipt.retained_storage())
                    },
                    work_limit,
                    storage_limit,
                )
            } else {
                0
            };
            let mut report = OutputObservation {
                case: self.case,
                profile: profile.to_owned(),
                callbacks: self.callbacks,
                erased,
                erasure,
                args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                source: source_stamps(),
                descriptor,
                output,
                native_module: digest(native.module().llvm_ir().as_bytes()),
                native_addition: native_receipt.retained_storage(),
                factory_failures,
                receipt: total_receipt,
                floor: budget.storage(),
                work: budget.work(),
                peak: budget.peak_storage(),
                native_authority: native.authenticates_execution(),
                artifact_authority: native.grants_artifact_or_launch_authority(),
                unsigned_source_proof_refused: false,
            };
            native.source_test_unsigned_source_proof_refusal_v3(erased, &mut budget);
            report.unsigned_source_proof_refused = true;
            report.work = budget.work();
            report.peak = budget.peak_storage();
            budget.release_storage(total_receipt).unwrap();
            assert_eq!(budget.storage(), 53);
            assert!(budget.work_ledger_identity_v1() == ledger);
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "genuine parent-managed P4 rustc child, never a no-environment success"]
fn nominal_policy4_source_child() {
    let path = PathBuf::from(env::var_os(CHILD_ARGS).expect("actual parent request"));
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let result = p4_request_case(&args).and_then(|case| {
        let mut callback = OutputCallbacks {
            args: args.clone(),
            case,
            callbacks: 0,
            result: None,
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rustc_driver::run_compiler(&args, &mut callback)
        })) {
            Ok(()) => callback
                .result
                .unwrap_or_else(|| Err(fail(Phase::Rustc, "P4 callback absent"))),
            Err(_) => Err(fail(Phase::Rustc, "actual P4 compiler panicked")),
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
        "NOMINAL_POLICY4_V3_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("NOMINAL_POLICY4_V3_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source refusal is not qualification: {result:?}"
    );
}

fn check_output_common(path: &Path, roots: &[&str], erased: bool) {
    let result: R<OutputObservation> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let report = result.expect("actual source-to-P4-O nominal continuation must succeed");
    assert_eq!(report.case.roots(), roots);
    assert_eq!(report.callbacks, 1);
    assert_eq!(report.source, source_stamps());
    let args: Vec<String> = serde_json::from_slice(
        &std::fs::read(path.with_file_name(format!("{}-args.json", report.case.feature())))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(p4_request_case(&args).unwrap(), report.case);
    assert_eq!(report.erased, erased);
    assert_eq!(report.case == P4Case::ErasedControl, erased);
    if erased {
        let [original, transformed] = report.erasure.expect("original N and erased E");
        assert_ne!(original, [0; 32]);
        assert_ne!(transformed, [0; 32]);
        assert_ne!(original, transformed);
    } else {
        assert!(report.erasure.is_none());
    }
    assert_eq!(
        report.args_sha256,
        digest(&serde_json::to_vec(&args).unwrap())
    );
    assert_eq!(
        args.iter()
            .filter(|arg| **arg == format!("-Ctarget-cpu={}", report.profile))
            .count(),
        1
    );
    assert_ne!(report.descriptor, [0; 32]);
    assert_ne!(report.output, [0; 32]);
    assert_ne!(report.native_module, [0; 32]);
    assert!(report.native_addition > 0 && report.native_addition < report.receipt);
    assert_eq!(
        report.factory_failures,
        if report.case.baseline() == Case::Control {
            4
        } else {
            0
        }
    );
    assert_eq!(report.floor, 53 + report.receipt);
    assert!(report.work > 17 && report.peak > report.floor);
    assert!(!report.native_authority && !report.artifact_authority);
    assert!(report.unsigned_source_proof_refused);
}
fn check_output(path: &Path, roots: &[&str]) {
    check_output_common(path, roots, false);
}
fn check_erased_output(path: &Path, roots: &[&str]) {
    check_output_common(path, roots, true);
}

#[test]
#[ignore = "requires pinned rust-src and eight actual ordinary AMD rustc children, not protected execution or GPU"]
fn ordinary_rust_nominal_policy4_v3_reaches_actual_o_native_transport() {
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
                child_test: P4_CHILD,
                check: check_output,
            }),
        );
    }
}

#[test]
#[ignore = "requires pinned rust-src and two retained-MIR UnitLocal rustc children; not protected execution or GPU"]
fn ordinary_rust_nominal_policy4_v3_retained_helper_reaches_erased_native_transport() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[OrdinarySourceCase::NominalPolicy4ErasedControl],
            profile,
            false,
            Some(SourceObserver {
                child_test: P4_CHILD,
                check: check_erased_output,
            }),
        );
    }
}

#[test]
fn nominal_p4_requests_keep_retained_controls_local_and_exact() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(format!("{BASE}/src/lib.rs"));
    let mut direct = feature_args(Case::Control);
    direct.push(source.display().to_string());
    assert_eq!(
        p4_request_case(&direct).unwrap(),
        P4Case::Direct(Case::Control)
    );
    let mut erased = direct.clone();
    erased[1] = ERASED_FLAGS[0].into();
    erased.extend(ERASED_FLAGS[1..].iter().map(|flag| flag.to_string()));
    assert_eq!(p4_request_case(&erased).unwrap(), P4Case::ErasedControl);
    for flag in ERASED_FLAGS {
        let mut missing = erased.clone();
        missing.retain(|arg| arg != flag);
        assert!(p4_request_case(&missing).is_err());
        let mut duplicate = erased.clone();
        duplicate.push(flag.into());
        assert!(p4_request_case(&duplicate).is_err());
        let mut split = erased.clone();
        split.retain(|arg| arg != flag);
        let offset = if flag.starts_with("--cfg") { 5 } else { 2 };
        split.push(flag[..offset].into());
        split.push(flag[offset..].trim_start_matches('=').into());
        assert!(p4_request_case(&split).is_err());
    }
    for extra in [
        "-Zmir-opt-level=1",
        "-Zinline-mir=yes",
        "--cfg=feature=\"guarded-loop-read\"",
    ] {
        let mut hostile = erased.clone();
        hostile.push(extra.into());
        assert!(p4_request_case(&hostile).is_err());
    }
    for flag in &ERASED_FLAGS[1..] {
        let mut hostile = direct.clone();
        hostile.push(flag.to_string());
        assert!(p4_request_case(&hostile).is_err());
    }
}
