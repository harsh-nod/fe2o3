//! Genuine original nominal Rust through bound reports, ranked/native checks and LLVM.
use super::*;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::target_llvm_tests::bound_target_tests::bound_target_child";

fn program(body: &str) -> String {
    format!(
        r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn bound_first(mut seed: usize) {{ {body} }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn bound_second(mut seed: usize) {{ {body} }}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct BoundOutcome {
    llvm: Outcome,
    physical_extents: Vec<[u64; 3]>,
    nominal_roots: usize,
    callback_refusal: bool,
    foreign_ledger_refusal: bool,
    target_result_teardown_refusal: bool,
    source_result_teardown_refusal: bool,
}

#[derive(Default)]
struct BoundCallbacks {
    result: Option<Result<BoundOutcome, String>>,
}

struct PanickingBoundResult<'a>(&'a Cell<bool>);
impl Drop for PanickingBoundResult<'_> {
    fn drop(&mut self) {
        self.0.set(true);
        panic!("bound result teardown must not replace selected settlement refusal");
    }
}
impl Callbacks for BoundCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let expected = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
                .and_then(|target| target.authenticate_import_session(tcx))
                .map_err(|error| format!("actual target: {error:?}"))?.profile();
            let (llvm, physical_extents, nominal_roots) = transaction()?
                .with_original_source_bound_scalar_target_llvm_v19(
                    |source, handoff, native, budget| {
                        let original = source.canonical(budget)?;
                        let output = handoff.output(budget)?;
                        assert_eq!(
                            source.source_semantic(budget)?.wire_version(),
                            SemanticMirWireVersionV1::V35
                        );
                        assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                        assert_eq!(output.owner().module().kernels.len(), 2);
                        assert!(original.module().kernels.iter().all(|root| {
                            root.domain
                                .extents()
                                .any(|axis| matches!(axis, fe2o3_kernel_ir::LaunchExtent::Dynamic))
                        }));
                        let nominal_roots = original
                            .module()
                            .functions
                            .iter()
                            .filter(|function| {
                                function.signature.parameters
                                    == [fe2o3_kernel_ir::Type::Scalar(
                                        fe2o3_kernel_ir::ScalarType::Index,
                                    )]
                            })
                            .count();
                        assert_eq!(nominal_roots, 2);
                        let (launches, width) = handoff.formal_context_v19(budget)?;
                        assert_eq!(width, fe2o3_kernel_ir::FormalIndexWidth::Bits64);
                        let mut physical_extents =
                            crate::production_pipeline::source_owned_v29::paid_vec(
                                launches.len(),
                                budget,
                            )?;
                        for launch in launches {
                            let fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::PhysicalEnvelope(
                                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact { rank: 1, extents },
                            ) = launch
                            else {
                                panic!("authenticated physical envelope required")
                            };
                            assert_eq!(*extents, [192, 1, 1]);
                            physical_extents.push(*extents);
                        }
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        assert_eq!(native.target(budget)?, expected);
                        let text = paid_text(native.llvm_ir(budget)?, budget)?;
                        let target = paid_text(expected.device_target(), budget)?;
                        let report = Outcome::Llvm {
                            source: *source.source_ssa(budget)?.source_semantic_sha256(),
                            original: Sha256::digest(original.canonical_bytes()).into(),
                            optimized: Sha256::digest(output.owner().canonical_bytes()).into(),
                            optimized_identity: *output.owner().identity().digest(),
                            optimized_bytes: output.owner().canonical_bytes().len(),
                            changed: output.owner().canonical_bytes() != output.input_audit_bytes(),
                            target,
                            text,
                            work: budget.work(),
                            peak: budget.peak_storage(),
                        };
                        Ok((report, physical_extents, nominal_roots))
                    },
                )
                .map_err(|error| format!("bound original target: {error:?}"))?;

            let completed = Cell::new(false);
            let error = transaction()?
                .with_original_source_bound_scalar_target_llvm_v19::<(), _>(
                    |_, handoff, native, budget| {
                        assert_eq!(handoff.formal_context_v19(budget)?.0.len(), 2);
                        assert!(!native.llvm_ir(budget)?.is_empty());
                        completed.set(true);
                        Err(Error::Unsupported("exact bound target callback refusal"))
                    },
                )
                .unwrap_err();
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Unsupported("exact bound target callback refusal")
            ));
            let foreign_completed = Cell::new(false);
            let error = transaction()?
                .with_original_source_bound_scalar_target_llvm_v19::<(), _>(|_, _, native, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    let error = native.llvm_ir(&foreign).unwrap_err();
                    assert!(matches!(
                        error,
                        SourceError::Resource(ResourceError::Accounting)
                    ));
                    foreign_completed.set(true);
                    Err(Error::from(error))
                })
                .unwrap_err();
            assert!(foreign_completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            let target_completed = Cell::new(false);
            let target_dropped = Cell::new(false);
            let result = transaction()?.with_original_source_bound_scalar_target_llvm_v19(
                |_, _, native, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    assert!(matches!(
                        native.llvm_ir(&foreign),
                        Err(SourceError::Resource(ResourceError::Accounting))
                    ));
                    target_completed.set(true);
                    Ok(PanickingBoundResult(&target_dropped))
                },
            );
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("damaged target settlement cannot succeed"),
            };
            assert!(target_completed.get() && target_dropped.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));

            let source_completed = Cell::new(false);
            let source_dropped = Cell::new(false);
            let result =
                transaction()?.with_original_source_integer_custody_v18(|_, handoff, _, _, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    assert!(matches!(
                        handoff.output(&foreign),
                        Err(SourceError::Resource(ResourceError::Accounting))
                    ));
                    source_completed.set(true);
                    Ok(PanickingBoundResult(&source_dropped))
                });
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("damaged source settlement cannot succeed"),
            };
            assert!(source_completed.get() && source_dropped.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            Ok(BoundOutcome {
                llvm,
                physical_extents,
                nominal_roots,
                callback_refusal: completed.get(),
                foreign_ledger_refusal: foreign_completed.get(),
                target_result_teardown_refusal: target_completed.get() && target_dropped.get(),
                source_result_teardown_refusal: source_completed.get() && source_dropped.get(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn bound_target_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = BoundCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("bound target source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("bound target result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "bound target: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and explicit FE2O3_OPT"]
fn actual_bound_scalar_changed_and_noop_nominal_dynamic_roots_reach_verified_target_llvm() {
    run_actual_sources::<BoundOutcome>(
        &[("bound_changed", "seed = 7_usize;"), ("bound_noop", "")],
        &[(0, 0)],
        CHILD,
        "BOUND_SOURCE_TARGET_LLVM_V19",
        program,
        |_, _, label, result, _| {
            let changed = match label {
                "bound_changed" => true,
                "bound_noop" => false,
                _ => panic!("unrequested bound-target case"),
            };
            assert_eq!(result.nominal_roots, 2);
            assert_eq!(result.physical_extents, vec![[192, 1, 1]; 2]);
            assert!(result.callback_refusal && result.foreign_ledger_refusal);
            assert!(result.target_result_teardown_refusal && result.source_result_teardown_refusal);
            let Outcome::Llvm {
                original,
                optimized,
                optimized_identity,
                optimized_bytes,
                changed: actual_changed,
                text,
                target,
                work,
                peak,
                ..
            } = &result.llvm
            else {
                panic!("actual target text required")
            };
            assert_eq!(*actual_changed, changed);
            assert_eq!(original != optimized, changed);
            assert!(*work < 500_000_000 && *peak <= 20_000_000);
            assert!(two_root_absence_matches(
                text,
                optimized_identity,
                *optimized_bytes,
                target
            ));
            assert!(!two_root_absence_matches(
                text,
                optimized,
                *optimized_bytes,
                target
            ));
            parse_and_verify_target_llvm(text);
        },
    );
}

#[test]
fn bound_target_actual_fixture_retains_nominal_types_and_nontrivial_physical_grid() {
    let source = program("seed = 7_usize;");
    assert_eq!(source.matches("mut seed: usize").count(), 2);
    assert_eq!(source.matches("max_grid = [3, 1, 1]").count(), 2);
    assert_eq!(source.matches("seed = 7_usize;").count(), 2);
    assert!(CHILD.ends_with("::bound_target_tests::bound_target_child"));
}
