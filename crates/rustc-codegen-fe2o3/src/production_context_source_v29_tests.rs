//! Same-transaction source observations, not callback execution or proof authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use fe2o3_lower_mir_kernel::{ProductionCheckedContextRootV29, ProductionContextRootErrorV29};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::fmt::Write as _;

const ARGS: &str = "FE2O3_TEST_CONTEXT_SOURCE_ARGS_V29";
const RESULT: &str = "FE2O3_TEST_CONTEXT_SOURCE_RESULT_V29";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::context_source_child";
const REPORT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct SourceObservation {
    source: [u8; 32],
    root: u32,
    helper: u32,
    derives: usize,
    rust_call_functions: usize,
    borrows: usize,
    helper_stores: usize,
    helper_stored_u32: Option<u32>,
    provider: Option<u32>,
    provider_calls: usize,
    provider_returns: usize,
    provider_returned_u32: Option<u32>,
    structure: String,
}

struct MeteredText<'a, 'w> {
    text: String,
    budget: &'a mut Budget<'w>,
    error: Option<ResourceError>,
}

impl std::fmt::Write for MeteredText<'_, '_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let remaining = REPORT_BYTES.saturating_sub(self.text.len());
        if text.len() > remaining {
            self.error = Some(ResourceError::Arithmetic);
            return Err(std::fmt::Error);
        }
        if let Err(error) = self.budget.charge_work(text.len()) {
            self.error = Some(error);
            return Err(std::fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

#[test]
fn context_observation_text_preserves_prefix_at_work_and_output_limits() {
    for limit in [13, 14] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 7 + REPORT_BYTES);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(7 + REPORT_BYTES).unwrap();
        let mut writer = MeteredText {
            text: String::with_capacity(REPORT_BYTES),
            budget: &mut budget,
            error: None,
        };
        assert_eq!(writer.write_str("abc").is_ok(), limit == 14);
        assert_eq!(writer.text, if limit == 14 { "abc" } else { "" });
        assert_eq!(writer.budget.work(), if limit == 14 { 14 } else { 11 });
        assert_eq!(writer.budget.storage(), 7 + REPORT_BYTES);
    }
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = Budget::new(&mut work, REPORT_BYTES);
    budget.reserve_storage(REPORT_BYTES).unwrap();
    let mut writer = MeteredText {
        text: "x".repeat(REPORT_BYTES - 1),
        budget: &mut budget,
        error: None,
    };
    writer.text.reserve_exact(1);
    writer.write_str("a").unwrap();
    assert!(writer.write_str("b").is_err());
    assert_eq!(writer.text.len(), REPORT_BYTES);
    assert!(writer.text.ends_with('a'));
    assert_eq!(writer.budget.work(), 1);
}

// Observe only unique direct return writers, not general value equivalence.
fn literal_return_through_defined_calls(
    source: &AdmittedInertSemanticMirV1,
    mut function: usize,
    budget: &mut Budget<'_>,
) -> Result<Option<u32>, ResourceError> {
    for _ in 0..source.functions().len() {
        let body = &source.functions()[function];
        budget.charge_work(1 + body.locals().len())?;
        let result = body
            .locals()
            .iter()
            .position(|local| local.role() == SemanticLocalRoleV1::Return)
            .unwrap();
        let mut writers = 0;
        let mut literal = None;
        let mut callee = None;
        for block in body.blocks() {
            budget.charge_work(1)?;
            for statement in block.statements() {
                budget.charge_work(1)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().local().index() as usize == result
                {
                    writers += 1;
                    if assignment.destination().projections().is_empty()
                        && let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value)) =
                            assignment.value().kind()
                        && let SemanticConstantValueV1::Scalar(scalar) = value.value()
                        && source.types()[value.ty().index() as usize].shape()
                            == &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                                signed: false,
                                bits: 32,
                            })
                        && scalar.size_bytes() == 4
                    {
                        literal = Some(u32::try_from(scalar.bits()).unwrap());
                    }
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.place().local().index() as usize == result
            {
                writers += 1;
                if destination.place().projections().is_empty()
                    && let SemanticCallableDeclV1::Defined { function } =
                        source.callables()[call.callee().index() as usize]
                {
                    callee = Some(function.index() as usize);
                }
            }
        }
        if writers != 1 {
            return Ok(None);
        }
        match callee {
            Some(next) => function = next,
            None => return Ok(literal),
        }
    }
    Ok(None)
}

fn observe(
    root: ProductionCheckedContextRootV29<'_>,
    budget: &mut Budget<'_>,
    provider_definition: SemanticItemDefinitionIdentityV1,
) -> Result<SourceObservation, ProductionContextRootErrorV29> {
    let ssa = root.semantic_ssa();
    let source = ssa.source_semantic();
    assert_eq!(source.wire_version(), SemanticMirWireVersionV1::V29);
    assert!(std::ptr::eq(
        root.root(),
        &source.functions()[root.root_id().index() as usize]
    ));
    assert!(std::ptr::eq(
        root.helper(),
        &source.functions()[root.helper_id().index() as usize]
    ));
    let mut observation = SourceObservation {
        source: *ssa.source_semantic_sha256(),
        root: root.root_id().index(),
        helper: root.helper_id().index(),
        derives: 0,
        rust_call_functions: 0,
        borrows: 0,
        helper_stores: 0,
        helper_stored_u32: None,
        provider: None,
        provider_calls: 0,
        provider_returns: 0,
        provider_returned_u32: None,
        structure: String::new(),
    };
    for (index, function) in source.functions().iter().enumerate() {
        budget.charge_work(1)?;
        observation.rust_call_functions +=
            usize::from(function.abi().extern_abi() == SemanticExternAbiV1::RustCall);
        for block in function.blocks() {
            budget.charge_work(2)?;
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && matches!(
                    source.callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Execution(
                            SemanticExecutionOperationV29::WorkgroupDerive { .. }
                        ),
                        ..
                    })
                )
            {
                observation.derives += 1;
                assert!(observation.provider.replace(index as u32).is_none());
            }
            for statement in block.statements() {
                budget.charge_work(2)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                    observation.borrows += usize::from(matches!(
                        assignment.value().kind(),
                        SemanticRvalueKindV1::Borrow { .. }
                    ));
                    budget.charge_work(assignment.destination().projections().len())?;
                    if index == observation.helper as usize
                        && assignment
                            .destination()
                            .projections()
                            .iter()
                            .any(|projection| {
                                projection.kind() == SemanticProjectionKindV1::Dereference
                            })
                    {
                        observation.helper_stores += 1;
                        if let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value)) =
                            assignment.value().kind()
                            && let SemanticConstantValueV1::Scalar(scalar) = value.value()
                            && source.types()[value.ty().index() as usize].shape()
                                == &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                                    signed: false,
                                    bits: 32,
                                })
                            && scalar.size_bytes() == 4
                        {
                            observation.helper_stored_u32 =
                                Some(u32::try_from(scalar.bits()).unwrap());
                        }
                    }
                }
            }
        }
    }
    // Diagnostic source observations only; this does not construct scope custody.
    if let Some(provider) = observation.provider {
        assert_ne!(provider, observation.root);
        assert_ne!(provider, observation.helper);
        for function in source.functions() {
            budget.charge_work(1)?;
            for block in function.blocks() {
                budget.charge_work(1)?;
                if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                    && matches!(source.callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::Defined { function })
                            if function.index() == provider)
                {
                    observation.provider_calls += 1;
                }
            }
        }
        let body = &source.functions()[provider as usize];
        assert_eq!(body.item_definition_identity(), provider_definition);
        observation.provider_returned_u32 =
            literal_return_through_defined_calls(source, provider as usize, budget)?;
        for block in body.blocks() {
            budget.charge_work(1)?;
            observation.provider_returns += usize::from(matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Return
            ));
        }
    }
    budget.reserve_storage(REPORT_BYTES)?;
    let mut text = String::new();
    text.try_reserve_exact(REPORT_BYTES)
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(text.capacity() - REPORT_BYTES)?;
    let mut writer = MeteredText {
        text,
        budget,
        error: None,
    };
    // Owned diagnostic text cannot be decoded into a source or execution owner.
    if write!(&mut writer,
        "authority=none\nssa_identity={:?}\nroot={:?}\nhelper={:?}\nissuance={:?}\nhelper_call={:?}\nlaunch={:?}\ntypes={:#?}\ncallables={:#?}\nfunctions={:#?}\nssa_plans={:#?}\n",
        ssa.identity(), root.root_id(), root.helper_id(), root.issuance_boundary(),
        root.helper_call_boundary(), root.launch(), source.types(), source.callables(),
        source.functions(), ssa.plans()).is_err()
    {
        return Err(writer.error.unwrap_or(ResourceError::Arithmetic).into());
    }
    observation.structure = writer.text;
    Ok(observation)
}

#[derive(Default)]
struct ContextCallbacks {
    result: Option<Result<SourceObservation, String>>,
}

impl Callbacks for ContextCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_pipeline::ProductionPipelineError;
        use fe2o3_lower_mir_kernel::{ProductionPreRankedKirErrorV1, ProductionSemanticKirErrorV1};
        self.result = Some((|| {
            let provider = tcx
                .get_diagnostic_item(rustc_span::Symbol::intern("fe2o3_device_with_workgroup_v1"))
                .expect("reviewed provider identity");
            assert_eq!(
                crate::trusted_device_items::classify(tcx, provider),
                Some(crate::trusted_device_items::TrustedDeviceItem::ExecutionWithWorkgroup)
            );
            assert!(crate::production_semantic_terminal_v1::classify(tcx, provider).is_none());
            let provider_definition =
                crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                    tcx,
                    rustc_middle::ty::Instance::new_raw(
                        provider,
                        rustc_middle::ty::GenericArgs::identity_for_item(tcx, provider),
                    ),
                )
                .item_definition();
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut observation = None;
            let result = transaction.observe_context_handoff_v29(|root, budget| {
                assert!(observation.is_none(), "one genuine source root");
                observation = Some(observe(root, budget, provider_definition)?);
                Ok(())
            });
            match result {
                Err(error)
                    if matches!(
                        *error,
                        ProductionPipelineError::PreRankedMaterialization(
                            ProductionPreRankedKirErrorV1::Lowering(
                                ProductionSemanticKirErrorV1::Unsupported {
                                    detail: "execution capabilities require checked canonical KIR materialization",
                                    ..
                                }
                            )
                        )
                    ) => {}
                Err(error) => return Err(format!("unexpected source boundary: {error}")),
                Ok(()) => return Err("staged callback unexpectedly materialized".into()),
            }
            observation.ok_or_else(|| "refusal occurred without a checked-root observation".into())
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn context_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ContextCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual rustc callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "context source observation: {result:?}");
}

fn source(body: &str) -> String {
    format!(
        r#"use fe2o3_device::{{kernel, thread, DisjointSlice, KernelContext}};
struct Capture(u32, u32);
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn callback_probe(mut ctx: KernelContext<'_>, mut output: DisjointSlice<u32>, seed: u32) {{
    {body}
    if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
}}
"#
    )
}

const CALLBACK_CASES: &[(&str, &str)] = &[
    ("constant7", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
    (
        "constant11",
        "let value = ctx.with_workgroup(|_wg| 11_u32);",
    ),
    (
        "captured",
        "let captured = Capture(seed, thread::index_1d().get() as u32); let value = ctx.with_workgroup(move |_wg| { let owned = captured; owned.0.wrapping_sub(owned.1) });",
    ),
    ("constant7", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
    (
        "discarded",
        "let _ = ctx.with_workgroup(|_wg| 7_u32); let value = seed;",
    ),
];

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_workgroup_callbacks_reach_the_checked_root_consumer() {
    check_actual_sources(CALLBACK_CASES, 1, &[(0, 0), (0, 2), (3, 0), (3, 2)]);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn optimized_workgroup_callbacks_reach_the_checked_root_consumer() {
    check_actual_sources(CALLBACK_CASES, 1, &[(3, 2)]);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_context_store_reaches_the_checked_root_consumer() {
    check_actual_sources(
        &[
            ("plain", "let value = seed;"),
            ("plain", "let value = seed;"),
        ],
        0,
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
    );
}

fn check_actual_sources(cases: &[(&str, &str)], expected_derives: usize, profiles: &[(u8, u8)]) {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let package_dir =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device");
    let metadata: serde_json::Value = serde_json::from_slice(
        &output(clean_command(env!("CARGO")).current_dir(&workspace).args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ]))
        .stdout,
    )
    .unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| {
            Path::new(package["manifest_path"].as_str().unwrap()) == package_dir.join("Cargo.toml")
        })
        .unwrap();
    let package_name = package["name"].as_str().unwrap();
    let version = package["version"].as_str().unwrap();
    let crate_name = package_name.replace('-', "_");
    let identity = PortablePackageIdentityV1::new(
        package_name,
        version,
        Sha256::digest(std::fs::read(package_dir.join("Cargo.toml")).unwrap()).into(),
    )
    .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-context-source-v29");
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot =
        String::from_utf8(output(clean_command(&rustc).args(["--print", "sysroot"])).stdout)
            .unwrap();
    let mut failures = Vec::new();
    for target in ["gfx942", "gfx950"] {
        let dependency_target = scratch.path().join(target);
        let flags = format!(
            "-Zalways-encode-mir -Ctarget-cpu={target} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"
        );
        let built = output(
            clean_command(env!("CARGO"))
                .current_dir(&workspace)
                .args([
                    "check",
                    "--offline",
                    "--locked",
                    "--release",
                    "-Zbuild-std=core",
                    "-p",
                    "fe2o3-device",
                    "--target",
                    "amdgcn-amd-amdhsa",
                    "--message-format=json-render-diagnostics",
                    "--target-dir",
                ])
                .arg(&dependency_target)
                .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", flags),
        );
        let messages: Vec<serde_json::Value> = built
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        let device = artifact(&messages, "fe2o3_device");
        let core = artifact(&messages, "core");
        let builtins = artifact(&messages, "compiler_builtins");
        for &(opt, mir) in profiles {
            let mut observations = std::collections::BTreeMap::<&str, SourceObservation>::new();
            for (ordinal, (label, body)) in cases.iter().enumerate() {
                let source_path = scratch.path().join(format!("{label}.rs"));
                std::fs::write(&source_path, source(body)).unwrap();
                let compiler_output = scratch
                    .path()
                    .join(format!("output-{target}-{opt}-{mir}-{ordinal}"));
                std::fs::create_dir(&compiler_output).unwrap();
                let mut args = vec![
                    rustc.to_str().unwrap().into(),
                    "--crate-name".into(),
                    crate_name.clone(),
                    "--crate-type=lib".into(),
                    format!("--edition={}", package["edition"].as_str().unwrap()),
                    "--target=amdgcn-amd-amdhsa".into(),
                    format!("-Ctarget-cpu={target}"),
                    "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".into(),
                    format!("-Copt-level={opt}"),
                    "-Cdebug-assertions=off".into(),
                    "-Coverflow-checks=on".into(),
                    format!("-Zmir-opt-level={mir}"),
                    "-Zalways-encode-mir".into(),
                    "-Zunstable-options".into(),
                    "--cfg=feature=\"provider_context_protocol\"".into(),
                    "--emit=metadata".into(),
                    "--sysroot".into(),
                    sysroot.trim().into(),
                    format!("--extern=fe2o3_device={}", device.display()),
                    format!("--extern=noprelude:core={}", core.display()),
                    format!(
                        "--extern=noprelude:compiler_builtins={}",
                        builtins.display()
                    ),
                    format!("-Ldependency={}", device.parent().unwrap().display()),
                    format!(
                        "-Ldependency={}",
                        dependency_target.join("release/deps").display()
                    ),
                    "--out-dir".into(),
                    compiler_output.display().to_string(),
                    package_dir.join("src/lib.rs").display().to_string(),
                ];
                let original: Vec<OsString> = args.iter().map(OsString::from).collect();
                let RustcInvocationV2::Compile(compile) =
                    classify_rustc_invocation_v2(&original).unwrap()
                else {
                    panic!("source compile")
                };
                let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
                args.push(format!("-Cmetadata={portable}"));
                let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
                let RustcInvocationV2::Compile(compile) =
                    classify_rustc_invocation_v2(&actual).unwrap()
                else {
                    panic!("bound source compile")
                };
                let build = derive_cargo_metadata_build_observation_v2(
                    &ordered_rustc_codegen_metadata_v1(compile).unwrap(),
                );
                let binding = derive_crate_binding_id_v1(&crate_name, [portable.as_str()]);
                let request = scratch
                    .path()
                    .join(format!("request-{target}-{opt}-{mir}-{ordinal}.json"));
                let response = scratch
                    .path()
                    .join(format!("response-{target}-{opt}-{mir}-{ordinal}.json"));
                assert!(!request.exists() && !response.exists());
                std::fs::write(&request, serde_json::to_vec(&args).unwrap()).unwrap();
                let child = clean_command(env::current_exe().unwrap())
                    .current_dir(&workspace)
                    .args(["--exact", CHILD, "--ignored", "--nocapture"])
                    .env(ARGS, &request)
                    .env(RESULT, &response)
                    .env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path)
                    .env("CARGO_MANIFEST_DIR", &package_dir)
                    .env("CARGO_PKG_NAME", package_name)
                    .env("CARGO_PKG_VERSION", version)
                    .env("CARGO_PRIMARY_PACKAGE", "1")
                    .env(CRATE_BINDING_ID_ENV_V1, binding.to_hex())
                    .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, build.to_hex())
                    .output()
                    .unwrap();
                assert_eq!(
                    std::fs::read_dir(&compiler_output).unwrap().count(),
                    0,
                    "no compiled kernel artifacts"
                );
                if !child.status.success() {
                    failures.push(format!(
                        "{target}/opt{opt}/mir{mir}/{label}: {}\n{}",
                        child.status,
                        String::from_utf8_lossy(&child.stderr)
                    ));
                    continue;
                }
                let result: Result<SourceObservation, String> =
                    serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
                let observation = result.unwrap();
                eprintln!(
                    "CONTEXT_SOURCE_OBSERVATION {target}/opt{opt}/mir{mir}/{label}: {}\n{}",
                    serde_json::to_string(&observation).unwrap(),
                    String::from_utf8_lossy(&child.stdout)
                );
                assert_eq!(observation.derives, expected_derives);
                assert_eq!(observation.helper_stores, 1);
                if opt == 3 && mir == 2 {
                    let expected = match *label {
                        "constant7" | "discarded" => Some(7),
                        "constant11" => Some(11),
                        _ => None,
                    };
                    assert_eq!(observation.provider_returned_u32, expected, "{label}");
                }
                if expected_derives != 0 {
                    assert!(observation.borrows > 0);
                    assert!(observation.provider.is_some());
                    assert_eq!(observation.provider_calls, 1);
                    assert_eq!(observation.provider_returns, 1);
                    assert_eq!(observation.helper_stored_u32, None);
                } else {
                    assert_eq!(observation.provider, None);
                    assert_eq!(observation.provider_calls, 0);
                    assert_eq!(observation.provider_returns, 0);
                }
                if let Some(previous) = observations.get(label) {
                    assert_eq!(&observation, previous, "fresh-process source observation");
                } else {
                    for previous in observations.values() {
                        assert_ne!(
                            observation.source, previous.source,
                            "changed source cannot reuse the observation"
                        );
                    }
                    observations.insert(label, observation);
                }
            }
        }
        std::fs::remove_dir_all(&dependency_target).unwrap();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
