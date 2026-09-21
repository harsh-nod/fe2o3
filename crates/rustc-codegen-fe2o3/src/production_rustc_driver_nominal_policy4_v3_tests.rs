//! Actual ordinary rustc -> original source -> P4 O -> nominal descriptor.
//! Successful source children are not native, protected or GPU qualification.
use super::*;
use crate::compiler_descriptor::nominal_v3::scoped;
use fe2o3_kernel_descriptor::{BuildEvidenceV1, EvidenceDigest, EvidenceIdentity};

const P4_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::policy4::nominal_policy4_source_child";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputObservation {
    case: Case,
    profile: String,
    callbacks: usize,
    erased: bool,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
    descriptor: [u8; 32],
    output: [u8; 32],
    receipt: usize,
    floor: usize,
    work: usize,
    peak: usize,
    native_authority: bool,
    artifact_authority: bool,
}

struct OutputCallbacks {
    args: Vec<String>,
    case: Case,
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
            let report = OutputObservation {
                case: self.case,
                profile: profile.to_owned(),
                callbacks: self.callbacks,
                erased: owner.source_test_is_erased_v3(),
                args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                source: source_stamps(),
                descriptor: digest(owner.canonical_bytes()),
                output: digest(owner.output().canonical().canonical_bytes()),
                receipt: receipt.retained_storage(),
                floor,
                work: budget.work(),
                peak: budget.peak_storage(),
                native_authority: owner.authenticates_execution(),
                artifact_authority: owner.grants_artifact_or_launch_authority(),
            };
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
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
    let result = request_case(&args).and_then(|case| {
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

fn check_output(path: &Path, roots: &[&str]) {
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
    assert_eq!(request_case(&args).unwrap(), report.case);
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
    assert_eq!(report.floor, 53 + report.receipt);
    assert!(report.work > 17 && report.peak > report.floor);
    assert!(!report.native_authority && !report.artifact_authority);
}

#[test]
#[ignore = "requires pinned rust-src and eight actual ordinary AMD rustc children, not protected execution or GPU"]
fn ordinary_rust_nominal_policy4_v3_reaches_actual_o_descriptor() {
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
