use super::*;

pub(super) struct Request {
    pub(super) args: Vec<String>,
    pub(super) response: std::path::PathBuf,
    request_path: std::path::PathBuf,
    source_path: std::path::PathBuf,
    pub(super) request_hash: [u8; 32],
    pub(super) source_hash: [u8; 32],
    pub(super) target: u16,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
impl Request {
    pub(super) fn read() -> Self {
        let request_path = std::path::PathBuf::from(
            env::var_os(ARGS).expect("strict tile child requires actual-source request"),
        );
        let response = std::path::PathBuf::from(
            env::var_os(RESULT).expect("strict tile child requires fresh report path"),
        );
        let source_path = std::path::PathBuf::from(
            env::var_os("FE2O3_CONTEXT_PROTOCOL_SOURCE")
                .expect("strict tile child requires original Rust source"),
        );
        assert!(request_path.is_absolute() && response.is_absolute() && source_path.is_absolute());
        assert!(request_path.is_file() && source_path.is_file() && !response.exists());
        assert_ne!(request_path, response);
        assert_ne!(source_path, response);
        let bytes = std::fs::read(&request_path).unwrap();
        assert!(bytes.len() <= REPORT_BYTES);
        let args: Vec<String> = serde_json::from_slice(&bytes).unwrap();
        let target = validate_arguments(&args).expect("exact parent argument contract");
        let source = std::fs::read(&source_path).unwrap();
        assert!(
            CASES
                .iter()
                .any(|(_, case)| source == program(case).as_bytes()),
            "only the requested real-source fixture bodies"
        );
        Self {
            args,
            response,
            request_path,
            source_path,
            request_hash: hash(&bytes),
            source_hash: hash(&source),
            target,
        }
    }
    pub(super) fn assert_unchanged(&self) {
        assert_eq!(
            hash(&std::fs::read(&self.request_path).unwrap()),
            self.request_hash
        );
        assert_eq!(
            hash(&std::fs::read(&self.source_path).unwrap()),
            self.source_hash
        );
        assert!(
            !self.response.exists(),
            "response must be fresh after actual compilation"
        );
    }
}
fn validate_arguments(args: &[String]) -> Result<u16, &'static str> {
    if args.len() != 27 {
        return Err("exact parent argc");
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
            return Err("changed parent argument");
        }
    }
    if !matches!(
        (args[8].as_str(), args[11].as_str()),
        ("-Copt-level=0", "-Zmir-opt-level=0") | ("-Copt-level=3", "-Zmir-opt-level=2")
    ) {
        return Err("unrequested optimization profile");
    }
    let target = match args[6].as_str() {
        "-Ctarget-cpu=gfx942" => 942,
        "-Ctarget-cpu=gfx950" => 950,
        _ => return Err("target"),
    };
    for (index, prefix) in [
        (18, "--extern=fe2o3_device="),
        (19, "--extern=noprelude:core="),
        (20, "--extern=noprelude:compiler_builtins="),
    ] {
        let file = args[index].strip_prefix(prefix).ok_or("extern ordering")?;
        if !Path::new(file).is_absolute() || !Path::new(file).is_file() {
            return Err("extern file");
        }
    }
    for index in [21, 22] {
        let directory = args[index]
            .strip_prefix("-Ldependency=")
            .ok_or("dependency ordering")?;
        if !Path::new(directory).is_absolute() || !Path::new(directory).is_dir() {
            return Err("dependency directory");
        }
    }
    let manifest = env::var_os("CARGO_MANIFEST_DIR").ok_or("manifest environment")?;
    if Path::new(&args[25]) != Path::new(&manifest).join("src/lib.rs") {
        return Err("fixture source path");
    }
    if !Path::new(&args[17]).is_absolute()
        || !Path::new(&args[17]).is_dir()
        || !Path::new(&args[24]).is_absolute()
        || !Path::new(&args[24]).is_dir()
    {
        return Err("compiler paths");
    }
    if args[26]
        .strip_prefix("-Cmetadata=")
        .is_none_or(str::is_empty)
    {
        return Err("metadata binding");
    }
    let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|_| "compiler invocation")?
    else {
        return Err("not a compiler invocation");
    };
    let metadata = ordered_rustc_codegen_metadata_v1(compile).map_err(|_| "metadata")?;
    let build = derive_cargo_metadata_build_observation_v2(&metadata);
    if env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).map_err(|_| "build binding")?
        != build.to_hex()
    {
        return Err("foreign build binding");
    }
    let binding =
        derive_crate_binding_id_v1(&args[2], [args[26].strip_prefix("-Cmetadata=").unwrap()]);
    if env::var(CRATE_BINDING_ID_ENV_V1).map_err(|_| "crate binding")? != binding.to_hex() {
        return Err("foreign crate binding");
    }
    Ok(target)
}
pub(super) fn validate(row: &TileObservation) -> Result<(), &'static str> {
    if row.callbacks != 1
        || ![942, 950].contains(&row.target)
        || row.order > 1
        || row.source == [0; 32]
        || row.pending == [0; 32]
        || row.current == [0; 32]
        || row.request == [0; 32]
        || row.source_file == [0; 32]
    {
        return Err("custody");
    }
    if row.roots == 0
        || row.instances == 0
        || row.aliases == 0
        || row.attachments == 0
        || row.piece_aliases == 0
        || row.multiple_aliases == 0
        || row.source_loads == 0
        || row.source_parts == 0
    {
        return Err("empty source or inverse");
    }
    if row.inventory != row.mapped || row.inventory.iter().any(|n| *n == 0) {
        return Err("incomplete actual inventory");
    }
    if row.source_loads.checked_mul(2) != Some(row.generated_reads)
        || row.source_parts.checked_mul(4) != Some(row.part_results)
    {
        return Err("incomplete expansion");
    }
    if row.pending_kinds.iter().any(|n| *n == 0)
        || row.pending_kinds[0] != row.aliases
        || row.pending_kinds[2] != row.roots
        || row.pending_kinds[3] != row.generated_reads
        || row.pending_kinds[4] != row.attachments
    {
        return Err("pending subject census");
    }
    let total = row
        .pending_kinds
        .iter()
        .try_fold(0_usize, |sum, n| sum.checked_add(*n))
        .ok_or("pending census overflow")?;
    if total != row.obligations {
        return Err("omitted pending obligation");
    }
    Ok(())
}
pub(super) fn audit_real_report(baseline: &TileObservation) {
    validate(baseline).unwrap();
    let bytes = serde_json::to_vec(baseline).unwrap();
    let roundtrip: TileObservation = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(&roundtrip, baseline);
    for mutation in 0..10 {
        let mut row = baseline.clone();
        match mutation {
            0 => row.callbacks = 0,
            1 => row.callbacks = 2,
            2 => row.mapped[0] = row.mapped[0].checked_add(1).unwrap(),
            3 => row.pending_kinds[1] = 0,
            4 => row.source_file = [0; 32],
            5 => row.generated_reads = row.generated_reads.checked_add(1).unwrap(),
            6 => row.obligations = row.obligations.checked_add(1).unwrap(),
            7 => {
                row.source_loads = (usize::MAX / 2 + 1)
                    .checked_add(baseline.source_loads)
                    .unwrap();
            }
            8 => {
                row.source_parts = (usize::MAX / 4 + 1)
                    .checked_add(baseline.source_parts)
                    .unwrap();
            }
            9 => {
                // Keep individual equalities valid; only the checked complete sum overflows.
                row.aliases = usize::MAX;
                row.pending_kinds[0] = usize::MAX;
                row.obligations = usize::MAX;
            }
            _ => unreachable!(),
        }
        let wire = serde_json::to_vec(&row).unwrap();
        let decoded: TileObservation = serde_json::from_slice(&wire).unwrap();
        assert!(validate(&decoded).is_err(), "wire mutation {mutation}");
    }
    let mut object: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    object.as_object_mut().unwrap().remove("source");
    assert!(serde_json::from_value::<TileObservation>(object).is_err());
    let mut duplicate = String::from_utf8(bytes).unwrap();
    duplicate.insert_str(1, "\"callbacks\":1,");
    assert!(serde_json::from_str::<TileObservation>(&duplicate).is_err());
}
#[test]
fn empty_and_incomplete_wire_reports_are_not_accepted_baselines() {
    assert!(serde_json::from_str::<TileObservation>("{}").is_err());
    assert!(serde_json::from_str::<TileObservation>("null").is_err());
    assert!(serde_json::from_str::<TileObservation>("{\"callbacks\":1,\"callbacks\":1}").is_err());
}
#[test]
fn strict_child_rejects_missing_or_extra_argv_without_running_compiler() {
    assert!(validate_arguments(&[]).is_err());
    assert!(validate_arguments(&vec!["forged".to_owned(); 28]).is_err());
}
