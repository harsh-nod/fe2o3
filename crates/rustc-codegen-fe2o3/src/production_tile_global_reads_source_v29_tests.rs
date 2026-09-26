//! Managed ordinary Rust, not source-model fallback or complete final admission.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, ExecutionOperationV15,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedTileGlobalReadsV29 as Checked, ProductionTileGlobalReadErrorV29 as ReadError,
    ProductionTilePendingKindV29 as PendingKind, ProductionTileScalarOrderV29 as Order,
};
#[path = "production_tile_global_reads_protocol_v29_tests.rs"]
mod protocol;
const REQUEST: &str = "FE2O3_TEST_TILE_GLOBAL_READ_REQUEST_V29";
const RESPONSE: &str = "FE2O3_TEST_TILE_GLOBAL_READ_RESPONSE_V29";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::tile_global_reads::tile_global_read_source_child";
const WIRE_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadObservation {
    request: [u8; 32],
    source_file: [u8; 32],
    target: u16,
    order: u8,
    source: [u8; 32],
    pending: [u8; 32],
    current: [u8; 32],
    roots: Vec<String>,
    callbacks: usize,
    stage: String,
    source_loads: usize,
    expected_reads: usize,
    reads: usize,
    checked_operations: usize,
    source_aliases: usize,
    read_aliases: usize,
    attachments: usize,
    instances: usize,
    pending_kinds: [usize; 5],
    obligations: usize,
    helper_reads: usize,
    work: usize,
    retained_units: usize,
    peak_mixed_units: usize,
    artifact_or_launch_authority: bool,
}
fn inspect(
    view: &Checked<'_>,
    budget: &mut Budget<'_>,
    order: Order,
) -> Result<ReadObservation, ReadError> {
    let transport = view.transport(budget)?;
    let inventory = transport.current_inventory(budget)?;
    assert!(std::ptr::eq(
        view.graph(budget)?.owner(budget)?,
        inventory.owner()
    ));
    let mut row = ReadObservation {
        request: [0; 32],
        source_file: [0; 32],
        target: 0,
        order: match order {
            Order::Blocked => 0,
            Order::Striped => 1,
        },
        source: *transport.source_identity(budget)?,
        pending: *transport.pending_identity(budget)?.digest(),
        current: *transport.current_identity(budget)?.digest(),
        roots: Vec::new(),
        callbacks: 1,
        stage: "local-global-read-conditions-pending".to_owned(),
        source_loads: 0,
        expected_reads: 0,
        reads: view.read_count(budget)?,
        checked_operations: 0,
        source_aliases: transport.source_alias_count(budget)?,
        read_aliases: view.alias_count(budget)?,
        attachments: transport.attachment_count(budget)?,
        instances: transport.instance_count(budget)?,
        pending_kinds: [0; 5],
        obligations: view.pending_obligation_count(budget)?,
        helper_reads: 0,
        work: 0,
        retained_units: 0,
        peak_mixed_units: 0,
        artifact_or_launch_authority: view.grants_artifact_or_launch_authority(),
    };
    for kernel in inventory.kernels() {
        row.roots.push(kernel.kernel.id.as_str().to_owned());
    }
    let original = transport.pending_ancestor(budget)?.pending_module();
    assert_eq!(original.functions.len(), inventory.functions().len());
    for (actual, original) in inventory.functions().iter().zip(&original.functions) {
        budget.charge_work(1)?;
        assert_eq!(actual.function.id, original.id);
        assert_eq!(actual.function.signature, original.signature);
        assert_eq!(actual.function.body.is_some(), original.body.is_some());
        if let Some(body) = &original.body {
            for block in &body.blocks {
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    if let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                        elements,
                        ..
                    }) = operation.kind
                    {
                        row.source_loads = row.source_loads.checked_add(1).unwrap();
                        row.expected_reads = row
                            .expected_reads
                            .checked_add(usize::from(elements))
                            .unwrap();
                    }
                }
            }
        }
    }
    let mut physical = std::collections::BTreeSet::new();
    let mut alias_count = 0_usize;
    for ordinal in 0..row.reads {
        let read = view.read(ordinal, budget)?;
        assert!(physical.insert(read.operation()));
        assert_eq!(read.domain().element_bytes(), 4);
        assert_eq!(read.checked_operations(), 3);
        assert!(!read.aliases().is_empty());
        row.checked_operations = row
            .checked_operations
            .checked_add(read.checked_operations())
            .unwrap();
        let subject = view
            .pending_obligation(read.obligation(), budget)?
            .global_read()
            .unwrap();
        assert_eq!(subject.output(), read.operation());
        let fe2o3_lower_mir_kernel::ProductionTileSourceSpanV29::Terminator(span) = transport
            .source_alias(subject.source_alias(), budget)?
            .source()
        else {
            panic!("actual source Load terminator");
        };
        row.helper_reads += usize::from(
            span.semantic_function()
                != transport
                    .root(read.root(), budget)?
                    .launch()
                    .selected_root(),
        );
        for alias in read.aliases() {
            let alias = view.alias(alias, budget)?;
            assert_eq!(alias.read(), ordinal);
            assert_eq!(
                transport.source_alias(alias.source_alias(), budget)?.root(),
                read.root()
            );
            alias_count = alias_count.checked_add(1).unwrap();
        }
    }
    assert_eq!(alias_count, row.read_aliases);
    for ordinal in 0..row.obligations {
        let pending = view.pending_obligation(ordinal, budget)?;
        let kind = match pending.kind() {
            PendingKind::Source { .. } => 0,
            PendingKind::CollectiveLifecycle { .. } => 1,
            PendingKind::LaunchGeometry { .. } => 2,
            PendingKind::GlobalRead => 3,
            PendingKind::RetainedAttachment { .. } => 4,
        };
        row.pending_kinds[kind] = row.pending_kinds[kind].checked_add(1).unwrap();
    }
    // Test-protocol allocations and serde time are outside compiler resource
    // qualification. These are observations, never expected quota boundaries.
    row.work = budget.work();
    row.retained_units = budget.storage();
    row.peak_mixed_units = budget.peak_storage();
    Ok(row)
}
struct GlobalCallbacks {
    order: Order,
    visits: usize,
    result: Option<Result<ReadObservation, String>>,
}
impl Callbacks for GlobalCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.visits += 1;
        assert_eq!(self.visits, 1);
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut callbacks = 0;
            let row = transaction
                .consume_checked_tile_global_reads_v29(self.order, |view, budget| {
                    callbacks += 1;
                    assert_eq!(callbacks, 1);
                    inspect(view, budget, self.order)
                })
                .map_err(|error| format!("actual Global read consumer refused: {error:?}"))?;
            assert_eq!(callbacks, 1);
            Ok(row)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict child: only managed parent supplies authenticated source/request/environment; missing input is failure"]
fn tile_global_read_source_child() {
    let (request, request_bytes, response) = protocol::read_request();
    let order = if request.order == 0 {
        Order::Blocked
    } else {
        Order::Striped
    };
    let mut callbacks = GlobalCallbacks {
        order,
        visits: 0,
        result: None,
    };
    rustc_driver::run_compiler(&request.args, &mut callbacks);
    assert_eq!(
        std::fs::read(PathBuf::from(env::var_os(REQUEST).unwrap())).unwrap(),
        request_bytes
    );
    protocol::validate_request(&request).unwrap();
    assert_eq!(callbacks.visits, 1);
    let result = callbacks
        .result
        .expect("actual rustc callback required")
        .map(|mut row| {
            row.request = Sha256::digest(&request_bytes).into();
            row.source_file = request.source_hash;
            row.target = request.target;
            protocol::validate_report(&request, &request_bytes, &row).unwrap();
            protocol::audit_accepted(&request, &request_bytes, &row);
            row
        });
    assert!(!response.exists());
    std::fs::write(response, serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(
        result.is_ok(),
        "producer/consumer refusal is not qualification: {result:?}"
    );
}

const CASES: &[&str] = &["single", "repeated", "branch"];
fn program(case: &str) -> String {
    let (extract, value) = match case {
        "single" => (
            "let ([a,b],[ma,mb]) = fragment.into_parts();",
            "read(&workgroup,input,base as usize)",
        ),
        "repeated" => (
            "let ([a,b],[ma,mb]) = fragment.into_parts();",
            "read(&workgroup,input,base as usize).wrapping_add(read(&workgroup,input,base.wrapping_add(7) as usize))",
        ),
        "branch" => (
            "let ([a,b],[ma,mb]) = if base == 0 { fragment.into_parts() } else { let ([a,b],mask) = fragment.into_parts(); ([a.wrapping_add(1),b],mask) };",
            "read(&workgroup,input,base as usize)",
        ),
        _ => panic!("closed test fixture roster"),
    };
    format!(
        r#"use fe2o3_device::{{kernel, thread, DisjointSlice, KernelContext, WorkgroupCapability, MaskedTile1D}};
#[inline(never)]
fn read<Brand>(workgroup: &WorkgroupCapability<'_, Brand>, input: &[u32], base: usize) -> u32 {{
    let tile = MaskedTile1D::<u32,64,2,_>::load_masked(workgroup,input,base);
    let fragment = tile.into_fragment();
    {extract}
    let a = if ma {{ a }} else {{ 0 }};
    let b = if mb {{ b }} else {{ 0 }};
    a.wrapping_add(b)
}}
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1]))]
pub fn tile_probe(mut ctx: KernelContext<'_>, input: &[u32], base: u64, mut output: DisjointSlice<u32>) {{
    ctx.with_workgroup(|workgroup| {{
        let value = {value};
        if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
    }});
}}
"#
    )
}

fn managed(order: u8) {
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
        .find(|row| {
            Path::new(row["manifest_path"].as_str().unwrap()) == package_dir.join("Cargo.toml")
        })
        .unwrap();
    let package_name = package["name"].as_str().unwrap();
    let version = package["version"].as_str().unwrap();
    let crate_name = package_name.replace('-', "_");
    let manifest_hash: [u8; 32] =
        Sha256::digest(std::fs::read(package_dir.join("Cargo.toml")).unwrap()).into();
    let identity = PortablePackageIdentityV1::new(package_name, version, manifest_hash).unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-tile-global-read-v29");
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot =
        String::from_utf8(output(clean_command(&rustc).args(["--print", "sysroot"])).stdout)
            .unwrap();
    for (target_name, target) in [("gfx942", 942), ("gfx950", 950)] {
        let dependency_target = scratch.path().join(target_name);
        let built = output(clean_command(env!("CARGO")).current_dir(&workspace)
            .args(["check", "--offline", "--locked", "--release", "-Zbuild-std=core", "-p", "fe2o3-device",
                "--target", "amdgcn-amd-amdhsa", "--message-format=json-render-diagnostics", "--target-dir"])
            .arg(&dependency_target).env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!("-Zalways-encode-mir -Ctarget-cpu={target_name} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")));
        let messages: Vec<serde_json::Value> = built
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        let device = artifact(&messages, "fe2o3_device");
        let core = artifact(&messages, "core");
        let builtins = artifact(&messages, "compiler_builtins");
        for (opt, mir) in [(0, 0), (3, 2)] {
            for (ordinal, case) in CASES.iter().enumerate() {
                let suffix = format!("{target_name}-{opt}-{mir}-{ordinal}-{order}");
                let source_path = scratch.path().join(format!("source-{suffix}.rs"));
                let source_bytes = program(case).into_bytes();
                std::fs::write(&source_path, &source_bytes).unwrap();
                let compiler_output = scratch.path().join(format!("output-{suffix}"));
                std::fs::create_dir(&compiler_output).unwrap();
                let mut args: Vec<String> = vec![
                    rustc.to_str().unwrap().into(),
                    "--crate-name".into(),
                    crate_name.clone(),
                    "--crate-type=lib".into(),
                    format!("--edition={}", package["edition"].as_str().unwrap()),
                    "--target=amdgcn-amd-amdhsa".into(),
                    format!("-Ctarget-cpu={target_name}"),
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
                    panic!("source compile");
                };
                let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
                args.push(format!("-Cmetadata={portable}"));
                let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
                let RustcInvocationV2::Compile(compile) =
                    classify_rustc_invocation_v2(&actual).unwrap()
                else {
                    panic!("actual compile");
                };
                let build = derive_cargo_metadata_build_observation_v2(
                    &ordered_rustc_codegen_metadata_v1(compile).unwrap(),
                )
                .to_hex();
                let binding = derive_crate_binding_id_v1(&crate_name, [portable.as_str()]).to_hex();
                let request = protocol::Request {
                    schema: 1,
                    args,
                    cwd: workspace.clone(),
                    source_path: source_path.clone(),
                    source_hash: Sha256::digest(&source_bytes).into(),
                    manifest: package_dir.join("Cargo.toml"),
                    manifest_hash,
                    case: (*case).into(),
                    target,
                    order,
                    build: build.clone(),
                    binding: binding.clone(),
                };
                let request_path = scratch.path().join(format!("request-{suffix}.json"));
                let response_path = scratch.path().join(format!("response-{suffix}.json"));
                let bytes = serde_json::to_vec(&request).unwrap();
                std::fs::write(&request_path, &bytes).unwrap();
                let child = clean_command(env::current_exe().unwrap())
                    .current_dir(&workspace)
                    .args(["--exact", CHILD, "--ignored", "--nocapture"])
                    .env(REQUEST, &request_path)
                    .env(RESPONSE, &response_path)
                    .env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path)
                    .env("CARGO_MANIFEST_DIR", &package_dir)
                    .env("CARGO_PKG_NAME", package_name)
                    .env("CARGO_PKG_VERSION", version)
                    .env("CARGO_PRIMARY_PACKAGE", "1")
                    .env(CRATE_BINDING_ID_ENV_V1, binding)
                    .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, build)
                    .output()
                    .unwrap();
                assert_eq!(std::fs::read(&request_path).unwrap(), bytes);
                assert_eq!(std::fs::read(&source_path).unwrap(), source_bytes);
                assert_eq!(
                    std::fs::read_dir(&compiler_output).unwrap().count(),
                    0,
                    "no artifact activation"
                );
                assert!(
                    child.status.success(),
                    "{suffix}: {}\n{}",
                    String::from_utf8_lossy(&child.stdout),
                    String::from_utf8_lossy(&child.stderr)
                );
                let report_bytes = std::fs::read(&response_path).unwrap();
                assert!(report_bytes.len() <= WIRE_LIMIT);
                let report: Result<ReadObservation, String> =
                    serde_json::from_slice(&report_bytes).unwrap();
                let report = report.unwrap();
                protocol::validate_report(&request, &bytes, &report).unwrap();
                if *case == "repeated" {
                    assert!(report.source_loads >= 2);
                }
                eprintln!(
                    "TILE_GLOBAL_LOCAL_PENDING {suffix}: {}",
                    serde_json::to_string(&report).unwrap()
                );
            }
        }
        std::fs::remove_dir_all(&dependency_target).unwrap();
    }
}
#[test]
#[ignore = "managed actual-source gate, gfx942/gfx950 and opt0/mir0 + opt3/mir2; pinned nightly rust-src and device dependencies required"]
fn actual_blocked_tile_global_reads_have_same_graph_local_conditions() {
    managed(0);
}
#[test]
#[ignore = "managed actual-source gate, gfx942/gfx950 and opt0/mir0 + opt3/mir2; pinned nightly rust-src and device dependencies required"]
fn actual_striped_tile_global_reads_have_same_graph_local_conditions() {
    managed(1);
}
