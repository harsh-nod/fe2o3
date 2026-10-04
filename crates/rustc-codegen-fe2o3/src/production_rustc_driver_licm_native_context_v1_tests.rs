//! Private immutable test context, separate from unchanged captured argv.
use super::*;
use fe2o3_rustc_invocation::{
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1,
};

pub(super) const PAIR_REQUEST: &str = "FE2O3_TEST_LICM_SESSION_PAIR_REQUEST";
pub(super) const PAIR_ARGS: &str = "FE2O3_TEST_LICM_SESSION_PAIR_ARGS";
pub(super) const PAIR_RESULT: &str = "FE2O3_TEST_LICM_SESSION_PAIR_RESULT";
const DOMAIN: &str = "FE2O3/TEST/IMMUTABLE-CONFIG-CONTEXT/V1";
const KEYS: [&str; 2] = [
    CRATE_BINDING_ID_ENV_V1,
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
];
type Environment = Vec<(std::ffi::OsString, std::ffi::OsString)>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Data {
    domain: String,
    metadata_policy: String,
    case: Case,
    target: String,
    args_sha256: [u8; 32],
    cfg: Vec<String>,
    cwd: PathBuf,
    source: PathBuf,
    crate_name: String,
    original_metadata: Vec<String>,
    session_metadata: Vec<String>,
    managed: [(String, String); 2],
    captured_environment_sha256: [u8; 32],
    common_environment_sha256: [u8; 32],
    sources: Vec<(String, [u8; 32])>,
    rustc_sha256: [u8; 32],
    test_binary_sha256: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    data: Data,
    sha256: [u8; 32],
}
impl Record {
    fn new(data: Data) -> Self {
        let sha256 = digest(&serde_json::to_vec(&data).unwrap());
        Self { data, sha256 }
    }
    pub(super) fn identity(&self) -> [u8; 32] {
        self.sha256
    }
    pub(super) fn case(&self) -> Case {
        self.data.case
    }
    pub(super) fn target(&self) -> &str {
        &self.data.target
    }
    pub(super) fn binding(&self) -> &str {
        &self.data.managed[0].1
    }
    pub(super) fn observation(&self) -> [u8; 32] {
        hex(&self.data.managed[1].1).unwrap()
    }
    pub(super) fn check(&self) -> Result<(), String> {
        let d = &self.data;
        if self.sha256 != digest(&serde_json::to_vec(d).unwrap())
            || d.domain != DOMAIN
            || d.metadata_policy != "CapturedUnchanged"
            || d.original_metadata.is_empty()
            || d.original_metadata != d.session_metadata
            || d.crate_name.is_empty()
            || d.managed[0].0 != KEYS[0]
            || d.managed[1].0 != KEYS[1]
            || !d.cfg.contains(&format!("feature=\"{}\"", d.case.feature()))
        {
            return Err("exact immutable context transcript".into());
        }
        profile(&d.target)?;
        for (_, value) in &d.managed {
            hex(value)?;
        }
        let binding = derive_crate_binding_id_v1(
            &d.crate_name,
            d.session_metadata.iter().map(String::as_str),
        );
        let observation = derive_cargo_metadata_build_observation_v2(&d.original_metadata);
        if d.managed[0].1 != binding.to_hex() || d.managed[1].1 != observation.to_hex() {
            return Err("captured original metadata and session binding derivation".into());
        }
        Ok(())
    }
}
fn hex(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("managed context hex".into());
    }
    let mut out = [0; 32];
    for (out, pair) in out.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        *out = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
            .map_err(|e| e.to_string())?;
    }
    if out == [0; 32] {
        return Err("managed context zero".into());
    }
    Ok(out)
}
fn environment(values: &Environment) -> Result<[u8; 32], String> {
    let mut keys = std::collections::BTreeSet::new();
    for (key, _) in values {
        if !keys.insert(key.clone()) {
            return Err("duplicate context environment key".into());
        }
    }
    environment_digest(
        values
            .iter()
            .filter(|(key, _)| {
                ![PAIR_REQUEST, PAIR_ARGS, PAIR_RESULT]
                    .iter()
                    .any(|name| key.as_os_str() == std::ffi::OsStr::new(*name))
            })
            .cloned()
            .collect(),
    )
}
fn managed(values: &Environment) -> Result<[(String, String); 2], String> {
    environment(values)?;
    let mut result = Vec::new();
    for key in KEYS {
        let (_, value) = values
            .iter()
            .find(|(name, _)| name == key)
            .ok_or("missing captured managed input")?;
        let value = value.to_str().ok_or("managed input UTF-8")?.to_owned();
        hex(&value)?;
        result.push((key.to_owned(), value));
    }
    Ok(result.try_into().unwrap())
}
fn substitute(values: &Environment, fields: &[(String, String); 2]) -> Environment {
    values
        .iter()
        .map(|(key, value)| {
            let value = fields
                .iter()
                .find(|(name, _)| key == name.as_str())
                .map(|(_, v)| v.as_str().into())
                .unwrap_or_else(|| value.clone());
            (key.clone(), value)
        })
        .collect()
}
fn invocation(args: &[String], cwd: &Path) -> Result<(String, PathBuf, Vec<String>), String> {
    if args
        .iter()
        .skip(1)
        .any(|a| a.starts_with('@') || a == "--env-set" || a.starts_with("--env-set="))
    {
        return Err("context lane rejects response files and raw logical overrides".into());
    }
    let argv = args
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&argv).map_err(|e| e.to_string())?
    else {
        return Err("actual captured compile required".into());
    };
    let source = cwd
        .join(compile.source_path())
        .canonicalize()
        .map_err(|e| e.to_string())?;
    Ok((
        compile.crate_name().into(),
        source,
        ordered_rustc_codegen_metadata_v1(compile).map_err(|e| e.to_string())?,
    ))
}

pub(super) struct Context {
    record: Record,
    installed: bool,
}
impl Context {
    pub(super) fn capture(
        value: &corpus_cargo::Captured,
        common: &Environment,
        case: Case,
        target: &str,
    ) -> Result<Record, String> {
        let fields = managed(&value.environment)?;
        let common_fields = managed(common)?;
        if environment(&substitute(&value.environment, &common_fields))? != environment(common)? {
            return Err("non-managed captured environment differs between sessions".into());
        }
        let (crate_name, source, metadata) = invocation(&value.args, &value.cwd)?;
        let record = Record::new(Data {
            domain: DOMAIN.into(),
            metadata_policy: "CapturedUnchanged".into(),
            case,
            target: target.into(),
            args_sha256: digest(&serde_json::to_vec(&value.args).unwrap()),
            cfg: value.cfg.clone(),
            cwd: value.cwd.clone(),
            source,
            crate_name,
            original_metadata: metadata.clone(),
            session_metadata: metadata,
            managed: fields,
            captured_environment_sha256: environment(&value.environment)?,
            common_environment_sha256: environment(common)?,
            sources: source_stamps(),
            rustc_sha256: digest(&std::fs::read(&value.args[0]).map_err(|e| e.to_string())?),
            test_binary_sha256: digest(
                &std::fs::read(env::current_exe().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            ),
        });
        record.check()?;
        Ok(record)
    }
    pub(super) fn from_record(record: Record, args: &[String]) -> Result<Self, String> {
        record.check()?;
        let d = &record.data;
        let (name, source, metadata) = invocation(args, &d.cwd)?;
        if d.args_sha256 != digest(&serde_json::to_vec(args).unwrap())
            || d.cwd != env::current_dir().map_err(|e| e.to_string())?
            || d.sources != source_stamps()
            || name != d.crate_name
            || source != d.source
            || metadata != d.original_metadata
            || d.rustc_sha256 != digest(&std::fs::read(&args[0]).map_err(|e| e.to_string())?)
            || d.test_binary_sha256
                != digest(
                    &std::fs::read(env::current_exe().map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                )
        {
            return Err("context source/argv/tool/cwd identity".into());
        }
        require_canonical_overflow_checks_v1(args)?;
        if args
            .iter()
            .filter(|a| a.starts_with("-Zmir-opt-level"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zmir-opt-level=0"]
            || args
                .iter()
                .filter(|a| a.starts_with("-Zinline-mir"))
                .map(String::as_str)
                .collect::<Vec<_>>()
                != ["-Zinline-mir=no"]
            || args.windows(2).any(|pair| {
                pair[0] == "-Z"
                    && (pair[1].starts_with("mir-opt-level") || pair[1].starts_with("inline-mir"))
            })
        {
            return Err("exact source MIR retention flags".into());
        }
        let value = Self {
            record,
            installed: false,
        };
        value.check_environment()?;
        Ok(value)
    }
    pub(super) fn record(&self) -> &Record {
        &self.record
    }
    pub(super) fn check_environment(&self) -> Result<(), String> {
        let current = env::vars_os().collect::<Environment>();
        if environment(&current)? != self.record.data.common_environment_sha256
            || environment(&substitute(&current, &self.record.data.managed))?
                != self.record.data.captured_environment_sha256
        {
            return Err("unchanged common environment and exact reconstructed capture".into());
        }
        Ok(())
    }
    fn parsed(
        &self,
        name: Option<&str>,
        cfg: &[String],
        input: &Path,
        metadata: &[String],
        logical: usize,
    ) -> Result<(), String> {
        let d = &self.record.data;
        if self.installed
            || logical != 0
            || name != Some(d.crate_name.as_str())
            || cfg != d.cfg
            || metadata != d.session_metadata
            || d.cwd
                .join(input)
                .canonicalize()
                .map_err(|e| e.to_string())?
                != d.source
        {
            return Err("actual parsed Config identity or duplicate context installation".into());
        }
        Ok(())
    }
    pub(super) fn install(
        &mut self,
        config: &mut rustc_interface::interface::Config,
    ) -> Result<(), String> {
        let rustc_session::config::Input::File(input) = &config.input else {
            return Err("actual Config Input::File required".into());
        };
        self.parsed(
            config.opts.crate_name.as_deref(),
            &config.crate_cfg,
            input,
            &config.opts.cg.metadata,
            config.opts.logical_env.len(),
        )?;
        self.check_environment()?;
        for (key, value) in &self.record.data.managed {
            assert!(
                config
                    .opts
                    .logical_env
                    .insert(key.clone(), value.clone())
                    .is_none()
            );
        }
        self.installed = true;
        Ok(())
    }
    pub(super) fn observe(&self, tcx: TyCtxt<'_>) -> Result<[u8; 32], String> {
        self.check_environment()?;
        if !self.installed
            || tcx.sess.opts.logical_env.len() != 2
            || tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE).as_str()
                != self.record.data.crate_name
            || tcx.sess.opts.cg.metadata != self.record.data.session_metadata
            || self
                .record
                .data
                .managed
                .iter()
                .any(|(key, value)| tcx.sess.opts.logical_env.get(key) != Some(value))
        {
            return Err("actual active session immutable context".into());
        }
        let target=crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).map_err(|e|e.to_string())?;
        if target.canonical_name() != profile(self.record.target())?.device_target() {
            return Err("actual context target".into());
        }
        let observation = crate::trusted_device_items::observe_managed_build_value_v1(tcx)?;
        if observation != self.record.observation() {
            return Err("actual provider observed wrong session context".into());
        }
        Ok(observation)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    fn inert() -> Record {
        inert_for_case(Case::DirectNoop, "gfx942", "fixture")
    }
    // Inert protocol input only; this never constructs an installed session or source owner.
    pub(in super::super) fn inert_for_case(case: Case, target: &str, metadata: &str) -> Record {
        let metadata = vec![metadata.to_owned()];
        Record::new(Data {
            domain: DOMAIN.into(),
            metadata_policy: "CapturedUnchanged".into(),
            case,
            target: target.into(),
            args_sha256: [1; 32],
            cfg: vec![format!("feature=\"{}\"", case.feature())],
            cwd: workspace(),
            source: workspace().join("Cargo.toml"),
            crate_name: "fixture".into(),
            original_metadata: metadata.clone(),
            session_metadata: metadata.clone(),
            managed: [
                (
                    KEYS[0].into(),
                    derive_crate_binding_id_v1("fixture", metadata.iter().map(String::as_str))
                        .to_hex(),
                ),
                (
                    KEYS[1].into(),
                    derive_cargo_metadata_build_observation_v2(&metadata).to_hex(),
                ),
            ],
            captured_environment_sha256: [2; 32],
            common_environment_sha256: [3; 32],
            sources: vec![],
            rustc_sha256: [4; 32],
            test_binary_sha256: [5; 32],
        })
    }
    #[test]
    fn session_context_transcript_requires_every_field_and_rejects_duplicates() {
        let record = inert();
        record.check().unwrap();
        let json = serde_json::to_value(&record).unwrap();
        for name in json["data"].as_object().unwrap().keys() {
            let mut changed = json.clone();
            changed["data"].as_object_mut().unwrap().remove(name);
            assert!(serde_json::from_value::<Record>(changed).is_err());
        }
        let mut changed = json.clone();
        changed["unknown"] = true.into();
        assert!(serde_json::from_value::<Record>(changed).is_err());
        let text = serde_json::to_string(&record).unwrap();
        let duplicate = text.replacen(
            "\"sha256\":",
            &format!(
                "\"sha256\":{},\"sha256\":",
                serde_json::to_string(&record.sha256).unwrap()
            ),
            1,
        );
        assert!(serde_json::from_str::<Record>(&duplicate).is_err());
        let mut changed = record;
        changed.data.args_sha256[0] ^= 1;
        assert!(changed.check().is_err());
    }
    #[test]
    fn session_context_requires_original_metadata_policy_and_exact_managed_derivations() {
        for index in 0..8 {
            let mut value = inert();
            match index {
                0 => value.data.metadata_policy = "PortableRewrite".into(),
                1 => value.data.original_metadata.push("extra".into()),
                2 => value.data.managed.swap(0, 1),
                3 => value.data.managed[1].1 = "00".repeat(32),
                4 => value.data.managed[0].1 = "g".repeat(64),
                5 => value.data.crate_name = "other".into(),
                6 => value.data.cfg.clear(),
                _ => value.data.target = "gfx000".into(),
            };
            value = Record::new(value.data);
            assert!(value.check().is_err());
        }
    }
    #[test]
    fn session_context_only_projects_exact_managed_values_not_other_environment() {
        let fields = inert().data.managed;
        let original = vec![
            (KEYS[0].into(), fields[0].1.clone().into()),
            (KEYS[1].into(), fields[1].1.clone().into()),
            ("SEMANTIC_OTHER".into(), "keep".into()),
        ];
        let mut other = original.clone();
        other[0].1 = "ab".repeat(32).into();
        assert_ne!(
            environment(&other).unwrap(),
            environment(&original).unwrap()
        );
        assert_eq!(
            environment(&substitute(&other, &fields)).unwrap(),
            environment(&original).unwrap()
        );
        other[2].1 = "changed".into();
        assert_ne!(
            environment(&substitute(&other, &fields)).unwrap(),
            environment(&original).unwrap()
        );
        other.push(other[0].clone());
        assert!(environment(&other).is_err());
        assert!(managed(&vec![]).is_err());
    }
    #[test]
    fn session_context_rejects_raw_and_response_logical_overrides() {
        for raw in [
            "@response",
            "--env-set",
            "--env-set=FE2O3_CRATE_BINDING_ID_V1=value",
        ] {
            assert!(
                invocation(&["rustc".into(), raw.into()], &workspace())
                    .unwrap_err()
                    .contains("response files")
            );
        }
    }
    #[test]
    fn session_context_parsed_identity_rejects_reinstallation_metadata_splitting_and_overrides() {
        let mut value = Context {
            record: inert(),
            installed: false,
        };
        let d = value.record.data.clone();
        value
            .parsed(
                Some(&d.crate_name),
                &d.cfg,
                &d.source,
                &d.session_metadata,
                0,
            )
            .unwrap();
        assert!(
            value
                .parsed(
                    Some(&d.crate_name),
                    &d.cfg,
                    &d.source,
                    &d.session_metadata,
                    1
                )
                .is_err()
        );
        assert!(
            value
                .parsed(Some("wrong"), &d.cfg, &d.source, &d.session_metadata, 0)
                .is_err()
        );
        assert!(
            value
                .parsed(Some(&d.crate_name), &[], &d.source, &d.session_metadata, 0)
                .is_err()
        );
        assert!(
            value
                .parsed(
                    Some(&d.crate_name),
                    &d.cfg,
                    &d.source,
                    &["a".into(), "b".into()],
                    0
                )
                .is_err()
        );
        assert!(
            value
                .parsed(
                    Some(&d.crate_name),
                    &d.cfg,
                    Path::new("Cargo.lock"),
                    &d.session_metadata,
                    0
                )
                .is_err()
        );
        value.installed = true;
        assert!(
            value
                .parsed(
                    Some(&d.crate_name),
                    &d.cfg,
                    &d.source,
                    &d.session_metadata,
                    0
                )
                .is_err()
        );
        let spaced = Context {
            record: inert_for_case(Case::DirectNoop, "gfx942", "two words"),
            installed: false,
        };
        let d = &spaced.record.data;
        spaced.record.check().unwrap();
        assert!(
            spaced
                .parsed(
                    Some(&d.crate_name),
                    &d.cfg,
                    &d.source,
                    &["two".into(), "words".into()],
                    0
                )
                .is_err()
        );
    }
}
