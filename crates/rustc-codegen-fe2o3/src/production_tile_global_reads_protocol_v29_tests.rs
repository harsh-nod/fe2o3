use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) schema: u8,
    pub(super) args: Vec<String>,
    pub(super) cwd: PathBuf,
    pub(super) source_path: PathBuf,
    pub(super) source_hash: [u8; 32],
    pub(super) manifest: PathBuf,
    pub(super) manifest_hash: [u8; 32],
    pub(super) case: String,
    pub(super) target: u16,
    pub(super) order: u8,
    pub(super) build: String,
    pub(super) binding: String,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(super) fn read_request() -> (Request, Vec<u8>, PathBuf) {
    let path = PathBuf::from(env::var_os(REQUEST).expect("strict child requires request"));
    let response = PathBuf::from(env::var_os(RESPONSE).expect("strict child requires response"));
    assert!(path.is_absolute() && response.is_absolute() && path.is_file() && !response.exists());
    assert_ne!(path, response);
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() <= WIRE_LIMIT);
    let request: Request = serde_json::from_slice(&bytes).unwrap();
    assert_ne!(request.source_path, response);
    assert_ne!(request.manifest, response);
    validate_request(&request).unwrap();
    (request, bytes, response)
}

pub(super) fn validate_request(request: &Request) -> Result<(), &'static str> {
    if request.schema != 1 || request.order > 1 || !CASES.contains(&request.case.as_str()) {
        return Err("schema/order/source case");
    }
    if env::current_dir().map_err(|_| "cwd")? != request.cwd || !request.cwd.is_absolute() {
        return Err("exact execution cwd");
    }
    if request.source_hash != hash(program(&request.case).as_bytes())
        || !request.source_path.is_absolute()
        || hash(&std::fs::read(&request.source_path).map_err(|_| "source file")?)
            != request.source_hash
        || PathBuf::from(env::var_os("FE2O3_CONTEXT_PROTOCOL_SOURCE").ok_or("source environment")?)
            != request.source_path
    {
        return Err("actual included source");
    }
    let package = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("manifest environment")?);
    if request.manifest != package.join("Cargo.toml")
        || hash(&std::fs::read(&request.manifest).map_err(|_| "manifest file")?)
            != request.manifest_hash
    {
        return Err("actual package manifest");
    }
    let args = &request.args;
    if args.len() != 27 {
        return Err("exact actual argc");
    }
    for (index, expected) in [
        (1, "--crate-name"),
        (2, "fe2o3_production_ranked_bounds_fixture"),
        (3, "--crate-type=lib"),
        (4, "--edition=2024"),
        (5, "--target=amdgcn-amd-amdhsa"),
        (
            7,
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        ),
        (9, "-Cdebug-assertions=off"),
        (10, "-Coverflow-checks=on"),
        (12, "-Zalways-encode-mir"),
        (13, "-Zunstable-options"),
        (14, "--cfg=feature=\"provider_context_protocol\""),
        (15, "--emit=metadata"),
        (16, "--sysroot"),
        (23, "--out-dir"),
    ] {
        if args[index] != expected {
            return Err("actual ordered argv");
        }
    }
    if !matches!(
        (args[8].as_str(), args[11].as_str()),
        ("-Copt-level=0", "-Zmir-opt-level=0") | ("-Copt-level=3", "-Zmir-opt-level=2")
    ) {
        return Err("optimization profile");
    }
    let target = match args[6].as_str() {
        "-Ctarget-cpu=gfx942" => 942,
        "-Ctarget-cpu=gfx950" => 950,
        _ => return Err("target"),
    };
    if request.target != target || Path::new(&args[25]) != package.join("src/lib.rs") {
        return Err("actual target/entry source");
    }
    for (index, prefix) in [
        (18, "--extern=fe2o3_device="),
        (19, "--extern=noprelude:core="),
        (20, "--extern=noprelude:compiler_builtins="),
    ] {
        let path = Path::new(args[index].strip_prefix(prefix).ok_or("extern order")?);
        if !path.is_absolute() || !path.is_file() {
            return Err("extern path");
        }
    }
    for index in [21, 22] {
        let path = Path::new(
            args[index]
                .strip_prefix("-Ldependency=")
                .ok_or("dependency order")?,
        );
        if !path.is_absolute() || !path.is_dir() {
            return Err("dependency path");
        }
    }
    for index in [17, 24] {
        if !Path::new(&args[index]).is_absolute() || !Path::new(&args[index]).is_dir() {
            return Err("compiler directory");
        }
    }
    let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|_| "invocation")?
    else {
        return Err("compile");
    };
    let metadata = ordered_rustc_codegen_metadata_v1(compile).map_err(|_| "metadata")?;
    let build = derive_cargo_metadata_build_observation_v2(&metadata).to_hex();
    let portable = args[26]
        .strip_prefix("-Cmetadata=")
        .filter(|v| !v.is_empty())
        .ok_or("portable metadata")?;
    let binding = derive_crate_binding_id_v1(&args[2], [portable]).to_hex();
    if request.build != build
        || request.binding != binding
        || env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).map_err(|_| "build environment")?
            != build
        || env::var(CRATE_BINDING_ID_ENV_V1).map_err(|_| "binding environment")? != binding
    {
        return Err("actual compile environment covariance");
    }
    let package_name = env::var("CARGO_PKG_NAME").map_err(|_| "package name")?;
    let version = env::var("CARGO_PKG_VERSION").map_err(|_| "package version")?;
    if package_name.replace('-', "_") != args[2] {
        return Err("crate name");
    }
    let identity = PortablePackageIdentityV1::new(&package_name, &version, request.manifest_hash)
        .map_err(|_| "package identity")?;
    let original: Vec<OsString> = args[..26].iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&original).map_err(|_| "original invocation")?
    else {
        return Err("original compile");
    };
    if portable_rustc_metadata_v1(compile, &identity).map_err(|_| "portable derivation")?
        != portable
    {
        return Err("derived ordered metadata");
    }
    Ok(())
}

pub(super) fn validate_report(
    request: &Request,
    request_bytes: &[u8],
    row: &ReadObservation,
) -> Result<(), &'static str> {
    if row.request != hash(request_bytes)
        || row.source_file != request.source_hash
        || row.target != request.target
        || row.order != request.order
        || row.callbacks != 1
        || row.stage != "local-global-read-conditions-pending"
        || row.artifact_or_launch_authority
    {
        return Err("actual invocation/stage/callback custody");
    }
    if row.source == [0; 32]
        || row.pending == [0; 32]
        || row.current == [0; 32]
        || row.roots != ["tile_probe"]
        || row.instances == 0
        || row.source_aliases == 0
        || row.attachments == 0
        || row.source_loads == 0
        || row.read_aliases < row.reads
        || row.helper_reads != row.reads
        || row.reads == 0
        || row.expected_reads != row.reads
    {
        return Err("actual complete source/read/alias roster");
    }
    if row.source_loads.checked_mul(2) != Some(row.reads)
        || row.reads.checked_mul(3) != Some(row.checked_operations)
    {
        return Err("complete local occurrence proofs");
    }
    if row.pending_kinds.iter().any(|count| *count == 0)
        || row.pending_kinds[0] != row.source_aliases
        || row.pending_kinds[2] != row.roots.len()
        || row.pending_kinds[3] != row.reads
        || row.pending_kinds[4] != row.attachments
        || row
            .pending_kinds
            .iter()
            .try_fold(0_usize, |a, b| a.checked_add(*b))
            != Some(row.obligations)
    {
        return Err("all unresolved obligations retained");
    }
    if row.work == 0 || row.retained_units == 0 || row.peak_mixed_units < row.retained_units {
        return Err("live descriptive resource observations");
    }
    Ok(())
}

pub(super) fn audit_accepted(request: &Request, bytes: &[u8], baseline: &ReadObservation) {
    // Called only by a genuinely successful rustc/production consumer callback.
    validate_request(request).unwrap();
    validate_report(request, bytes, baseline).unwrap();
    for mutation in 0..16 {
        let mut row = baseline.clone();
        match mutation {
            0 => row.request[0] ^= 1,
            1 => row.source_file[0] ^= 1,
            2 => row.target ^= 1,
            3 => row.order ^= 1,
            4 => row.callbacks = 0,
            5 => row.callbacks = 2,
            6 => row.stage.push('x'),
            7 => row.artifact_or_launch_authority = true,
            8 => row.reads += 1,
            9 => row.checked_operations += 1,
            10 => row.read_aliases = 0,
            11 => row.pending_kinds[1] = 0,
            12 => row.helper_reads = 0,
            13 => row.source_loads = usize::MAX,
            14 => {
                row.source_aliases = usize::MAX;
                row.pending_kinds[0] = usize::MAX;
                row.obligations = usize::MAX;
            }
            15 => row.roots.push("tile_probe".into()),
            _ => unreachable!(),
        }
        let wire = serde_json::to_vec(&row).unwrap();
        let decoded: ReadObservation = serde_json::from_slice(&wire).unwrap();
        assert!(
            validate_report(request, bytes, &decoded).is_err(),
            "real-baseline report mutation {mutation}"
        );
    }
    for mutation in 0..14 {
        let mut changed = request.clone();
        match mutation {
            0 => changed.schema = 2,
            1 => changed.args.push("--cfg=foreign".into()),
            2 => changed.args[5] = "--target=x86_64-unknown-linux-gnu".into(),
            3 => changed.args[2].push('x'),
            4 => changed.args[26].push('x'),
            5 => changed.source_hash[0] ^= 1,
            6 => changed.manifest_hash[0] ^= 1,
            7 => changed.cwd = changed.cwd.join("foreign"),
            8 => changed.target = 999,
            9 => changed.build.push('x'),
            10 => changed.binding.push('x'),
            11 => changed.order = 2,
            12 => changed.source_path = changed.manifest.clone(),
            13 => changed.args[25] = changed.source_path.display().to_string(),
            _ => unreachable!(),
        }
        assert!(
            validate_request(&changed).is_err(),
            "real-baseline request mutation {mutation}"
        );
    }
    let wire = serde_json::to_vec(baseline).unwrap();
    let mut object: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    object.as_object_mut().unwrap().remove("current");
    assert!(serde_json::from_value::<ReadObservation>(object).is_err());
    let mut duplicate = String::from_utf8(wire).unwrap();
    duplicate.insert_str(1, "\"callbacks\":1,");
    assert!(serde_json::from_str::<ReadObservation>(&duplicate).is_err());
    validate_request(request).unwrap();
    validate_report(request, bytes, baseline).unwrap();
}
#[test]
fn empty_or_duplicate_protocol_fields_do_not_supply_a_positive_baseline() {
    for text in ["{}", "null", "{\"callbacks\":1,\"callbacks\":1}"] {
        assert!(serde_json::from_str::<ReadObservation>(text).is_err());
        assert!(serde_json::from_str::<Request>(text).is_err());
    }
}
