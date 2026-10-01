//! Ignored storage-only genuine frontend adapter. No recipe replay or warm timing.
//! Root supplies a FRESH existing recipe-series preparation and owns process,
//! compiler/dependency/source custody, deadlines and stream/file caps.
use super::{
    Callbacks, Compilation, Compiler, TyCtxt, require_canonical_overflow_checks_v1,
    transaction_in_active_session_v1,
};
use crate::production_pipeline::{
    RankedVerifiedProductionCompilation,
    bindings_retained_storage_v1::{
        BindingsRetainedStorageV1, BindingsStorageErrorV1,
        checkpoint::BindingsStorageCheckpointErrorV1 as CheckpointError,
    },
};
use fe2o3_kernel_ir::{LogicalStorageErrorV1, LogicalStorageLimitsV1};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[path = "source_bindings_checkpoint_io_v1_tests.rs"]
mod io;
use io::{Pin, Retained, diagnostic, digest, frame, hash, json_bytes};

const CONFIG_ENV: &str = "FE2O3_BINDINGS_CHECKPOINT_CONFIG";
const HASH_ENV: &str = "FE2O3_BINDINGS_CHECKPOINT_CONFIG_SHA256";
const SCHEMA: &str = "fe2o3-bindings-checkpoint-input-v1";
const PREFIX: &str = "FE2O3_BINDINGS_CHECKPOINT_V1 ";
const NORMAL_PREFIX: &str = "FE2O3_BINDINGS_NORMAL_V1 ";
const CONFIG_CAP: usize = 16 * 1024;
const PREPARED_CAP: usize = 256 * 1024;
const SOURCE_CAP: usize = 64 * 1024;
const NORMAL_CAP: usize = 2 * 1024 * 1024;
const RECORD_CAP: usize = 16 * 1024;
const OBSERVATION_BYTES: usize = 128 * 1024 * 1024;
const OBSERVATION_ITEMS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ordinary,
    Observed,
    DenyBytes,
    DenyItems,
}
impl Mode {
    fn limits(self) -> LogicalStorageLimitsV1 {
        LogicalStorageLimitsV1 {
            max_bytes: Some(if self == Self::DenyBytes {
                0
            } else {
                OBSERVATION_BYTES
            }),
            max_items: if self == Self::DenyItems {
                0
            } else {
                OBSERVATION_ITEMS
            },
        }
    }
    fn denied(self) -> bool {
        matches!(self, Self::DenyBytes | Self::DenyItems)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    mode: Mode,
    preparation: Pin,
    ordinary_oracle: Option<Pin>,
}
// This selects fields of the existing preparation record. Its complete original
// bytes/hash remain retained, not reconstructed as compiler or recipe authority.
#[derive(Deserialize)]
struct Preparation {
    schema: String,
    repository_cwd: PathBuf,
    source_relative: PathBuf,
    source_absolute: PathBuf,
    source_bytes: usize,
    source_sha256: String,
    rustc_args: Vec<String>,
    required_child_environment: BTreeMap<String, String>,
    compiler_frontends_entered: usize,
    recipe_created: bool,
    child_processes_spawned: usize,
    grants_artifact_or_launch_authority: bool,
}
fn validate_config(config: &Config) -> Result<(), String> {
    if config.schema != SCHEMA {
        return Err("checkpoint config schema differs".into());
    }
    hash(&config.preparation.sha256)?;
    if (config.mode == Mode::Observed) != config.ordinary_oracle.is_some() {
        return Err("only observed mode requires exactly one independent ordinary oracle".into());
    }
    Ok(())
}
fn validate_preparation_shape(p: &Preparation) -> Result<(), String> {
    if p.schema != "fe2o3-recipe-series-prepared-v1"
        || p.compiler_frontends_entered != 0
        || p.recipe_created
        || p.child_processes_spawned != 0
        || p.grants_artifact_or_launch_authority
        || p.source_bytes == 0
        || p.source_bytes > SOURCE_CAP
        || p.rustc_args.is_empty()
        || p.rustc_args.len() > 512
        || p.required_child_environment.len() != 6
    {
        return Err("fresh original preparation shape differs".into());
    }
    hash(&p.source_sha256)?;
    if !p.repository_cwd.is_absolute()
        || !p.source_absolute.is_absolute()
        || p.source_relative.is_absolute()
        || p.source_relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || p.repository_cwd.join(&p.source_relative) != p.source_absolute
    {
        return Err("prepared source/cwd path join differs".into());
    }
    let source = p.source_absolute.to_str().ok_or("source path not UTF-8")?;
    if source.len() > 4096
        || p.rustc_args
            .iter()
            .filter(|arg| arg.as_str() == source)
            .count()
            != 1
    {
        return Err("exact prepared source argv member differs".into());
    }
    let mut bytes = 0usize;
    for arg in &p.rustc_args {
        if arg.len() > 4096 || arg.contains('\0') {
            return Err("bounded rustc argument required".into());
        }
        bytes = bytes
            .checked_add(arg.len())
            .ok_or("argument byte overflow")?;
    }
    if bytes > PREPARED_CAP {
        return Err("argument byte envelope exceeded".into());
    }
    for (key, value) in &p.required_child_environment {
        if key.is_empty()
            || key.len() > 256
            || value.len() > PREPARED_CAP
            || key.contains('\0')
            || key.contains('=')
            || value.contains('\0')
        {
            return Err("bounded prepared environment required".into());
        }
    }
    Ok(())
}
fn current_preparation(p: &Preparation) -> Result<(), String> {
    io::canonical_directory(&p.repository_cwd)?;
    if std::env::current_dir().map_err(|e| e.to_string())? != p.repository_cwd {
        return Err("current cwd differs from original preparation".into());
    }
    for (key, value) in &p.required_child_environment {
        if std::env::var(key).map_err(|_| "required prepared environment missing")? != *value {
            return Err(format!("current prepared environment differs for {key}"));
        }
    }
    crate::source_local_order_recipe_api_v1::validate_arguments(&p.rustc_args)
        .map_err(|e| e.to_string())?;
    require_canonical_overflow_checks_v1(&p.rustc_args)
}
fn join_actual_source(tcx: TyCtxt<'_>, p: &Preparation, source: &Retained) -> Result<(), String> {
    source.metadata_current()?;
    let local = tcx
        .sess
        .local_crate_source_file()
        .and_then(|file| file.local_path().map(PathBuf::from))
        .ok_or("actual local crate source unavailable")?;
    if local != p.source_absolute
        || source.path != p.source_absolute
        || source.bytes.len() != p.source_bytes
    {
        return Err("actual original crate/source identity differs".into());
    }
    let text = std::str::from_utf8(&source.bytes).map_err(|_| "original source is not UTF-8")?;
    let files = tcx.sess.source_map().files();
    if files.len() > 4096 {
        return Err("source map roster envelope".into());
    }
    let mut selected = 0usize;
    for file in files.iter() {
        let rustc_span::FileName::Real(name) = &file.name else {
            continue;
        };
        if name.local_path() != Some(p.source_absolute.as_path()) {
            continue;
        }
        selected += 1;
        if file.src.as_deref().is_none_or(|actual| actual != text)
            || !file.src_hash.matches(text)
            || file.unnormalized_source_len as usize != source.bytes.len()
            || file.normalized_source_len.0 as usize != source.bytes.len()
        {
            return Err("actual compiler source bytes/normalization differ".into());
        }
    }
    if selected != 1 {
        return Err("actual source map file absent or ambiguous".into());
    }
    source.metadata_current()
}

#[derive(Serialize)]
struct NormalRoot<'a> {
    function_name: &'a str,
    export_symbol: &'a [u8],
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    ranked_ir: &'a str,
    bounds_are_clean: bool,
    all_kernel_checks_are_clean: bool,
    // Exact ordered site identities, not a new recursive effect serializer.
    observed_gpu_write_sites: Vec<[u64; 3]>,
}
#[derive(Serialize)]
struct Normal<'a> {
    schema: &'static str,
    source_sha256: &'a str,
    ranked_root_count: usize,
    semantic_function_count: usize,
    semantic_callable_count: usize,
    retained_identity_and_transaction_binding_count: usize,
    bounds_are_clean: bool,
    all_kernel_checks_are_clean: bool,
    grants_artifact_or_launch_authority: bool,
    root: NormalRoot<'a>,
}
fn normal(
    owner: &RankedVerifiedProductionCompilation,
    source_hash: &str,
) -> Result<Vec<u8>, String> {
    let [root] = owner.ranked_roots() else {
        return Err("fixed genuine single-root fixture required".into());
    };
    if owner.ranked_root_count() != 1
        || !owner.bounds_are_clean()
        || !owner.all_kernel_checks_are_clean()
        || owner.grants_artifact_or_launch_authority()
        || !root.bounds_are_clean()
        || !root.all_kernel_checks_are_clean()
        || root.function_name().len() > 4096
        || root.export_symbol().len() > 4096
        || root.ranked_ir().len() > 512 * 1024
        || root.observed_reference_writes().len() > 256
    {
        return Err("ranked content/verification envelope differs".into());
    }
    let mut sites = Vec::with_capacity(root.observed_reference_writes().len());
    for site in root.observed_reference_writes() {
        sites.push([
            u64::try_from(site.block).map_err(|_| "effect block overflow")?,
            u64::try_from(site.operation).map_err(|_| "effect operation overflow")?,
            site.allocation_origin,
        ]);
    }
    json_bytes(
        &Normal {
            schema: "fe2o3-bindings-checkpoint-ranked-content-v1",
            source_sha256: source_hash,
            ranked_root_count: owner.ranked_root_count(),
            semantic_function_count: owner.semantic_function_count(),
            semantic_callable_count: owner.semantic_callable_count(),
            retained_identity_and_transaction_binding_count: owner
                .retained_identity_and_transaction_binding_count(),
            bounds_are_clean: owner.bounds_are_clean(),
            all_kernel_checks_are_clean: owner.all_kernel_checks_are_clean(),
            grants_artifact_or_launch_authority: owner.grants_artifact_or_launch_authority(),
            root: NormalRoot {
                function_name: root.function_name(),
                export_symbol: root.export_symbol(),
                semantic_root: root.semantic_root().index(),
                semantic_root_identity: *root.semantic_root_identity().as_bytes(),
                kernel_binding: *root.kernel_binding(),
                source_rank: root.source_rank(),
                ranked_ir: root.ranked_ir(),
                bounds_are_clean: root.bounds_are_clean(),
                all_kernel_checks_are_clean: root.all_kernel_checks_are_clean(),
                observed_gpu_write_sites: sites,
            },
        },
        NORMAL_CAP,
    )
}
enum Outcome {
    Verified {
        normal: Vec<u8>,
        storage: Option<BindingsRetainedStorageV1>,
    },
    Denied(&'static str),
}
fn expected_denial(mode: Mode, error: CheckpointError) -> Result<&'static str, String> {
    match (mode, error) {
        (
            Mode::DenyBytes,
            CheckpointError::Storage(BindingsStorageErrorV1::Counter(
                LogicalStorageErrorV1::ByteLimit,
            )),
        ) => Ok("ByteLimit"),
        (
            Mode::DenyItems,
            CheckpointError::Storage(BindingsStorageErrorV1::Counter(
                LogicalStorageErrorV1::ItemLimit,
            )),
        ) => Ok("ItemLimit"),
        (_, error) => Err(format!("unexpected original checkpoint error: {error}")),
    }
}
struct Observe<'a> {
    preparation: &'a Preparation,
    source: &'a Retained,
    mode: Mode,
    entries: usize,
    result: Option<Result<Outcome, String>>,
}
impl Observe<'_> {
    fn once(&self, tcx: TyCtxt<'_>) -> Result<Outcome, String> {
        join_actual_source(tcx, self.preparation, self.source)?;
        let transaction = transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )?;
        if self.mode == Mode::Ordinary {
            let owner = transaction
                .verify_general_kernel_checks()
                .map_err(|e| e.to_string())?;
            return Ok(Outcome::Verified {
                normal: normal(&owner, &self.preparation.source_sha256)?,
                storage: None,
            });
        }
        match transaction.verify_general_kernel_checks_with_bindings_storage_v1(self.mode.limits())
        {
            Ok((owner, report)) => {
                if self.mode.denied() {
                    return Err("denied mode unexpectedly returned an owner/report".into());
                }
                if report.header_bytes == 0
                    || report.visited_items == 0
                    || report.header_bytes.checked_add(report.heap_bytes)
                        != Some(report.total_bytes)
                    || report.total_bytes > OBSERVATION_BYTES
                    || report.visited_items > OBSERVATION_ITEMS
                {
                    return Err("returned named bindings report is inconsistent".into());
                }
                Ok(Outcome::Verified {
                    normal: normal(&owner, &self.preparation.source_sha256)?,
                    storage: Some(report),
                })
            }
            Err(error) if self.mode.denied() => {
                Ok(Outcome::Denied(expected_denial(self.mode, error)?))
            }
            Err(error) => Err(format!("original observed continuation: {error}")),
        }
    }
}
impl Callbacks for Observe<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.entries = self.entries.saturating_add(1);
        self.result = Some(if self.entries == 1 {
            self.once(tcx)
        } else {
            Err("repeated genuine callback; no successful observation".into())
        });
        Compilation::Stop
    }
}
fn finish(
    entries: usize,
    fatal: bool,
    result: Option<Result<Outcome, String>>,
) -> Result<Outcome, String> {
    if fatal || entries != 1 {
        return Err("fatal/absent/repeated compiler callback; no successful report".into());
    }
    result.ok_or("genuine callback did not finish")?
}
fn compare(bytes: &[u8], oracle: &[u8]) -> Result<(), String> {
    if bytes == oracle {
        Ok(())
    } else {
        Err("full ordinary ranked-content bytes differ".into())
    }
}
fn emit(value: &impl Serialize) -> Result<(), String> {
    frame(PREFIX, &json_bytes(value, RECORD_CAP)?, RECORD_CAP)
}
fn child() -> Result<(), String> {
    let pin = Pin {
        path: std::env::var(CONFIG_ENV).map_err(|_| "missing checkpoint config")?,
        sha256: std::env::var(HASH_ENV).map_err(|_| "missing checkpoint config SHA256")?,
    };
    let mut config_guard = Retained::open(&pin, CONFIG_CAP)?;
    let config: Config = serde_json::from_slice(&config_guard.bytes).map_err(|e| e.to_string())?;
    validate_config(&config)?;
    let mut prepared_guard = Retained::open(&config.preparation, PREPARED_CAP)?;
    let preparation: Preparation =
        serde_json::from_slice(&prepared_guard.bytes).map_err(|e| e.to_string())?;
    validate_preparation_shape(&preparation)?;
    current_preparation(&preparation)?;
    let mut source = Retained::open(
        &Pin {
            path: preparation
                .source_absolute
                .to_str()
                .ok_or("UTF-8 source path")?
                .to_owned(),
            sha256: preparation.source_sha256.clone(),
        },
        SOURCE_CAP,
    )?;
    if source.bytes.len() != preparation.source_bytes {
        return Err("prepared source length differs".into());
    }
    let mut oracle = config
        .ordinary_oracle
        .as_ref()
        .map(|pin| Retained::open(pin, NORMAL_CAP))
        .transpose()?;
    emit(&serde_json::json!({
        "kind":"started","schema":SCHEMA,"mode":config.mode,
        "config_sha256":pin.sha256,"preparation_sha256":config.preparation.sha256,
        "source_sha256":preparation.source_sha256,"source_bytes":preparation.source_bytes,
        "argv_sha256":digest(&json_bytes(&preparation.rustc_args, PREPARED_CAP)?),
        "environment_sha256":digest(&json_bytes(&preparation.required_child_environment, PREPARED_CAP)?),
        "maximum_frontends":1,"maximum_original_imports":1,
        "maximum_selected_observer_input_bytes":3*(CONFIG_CAP+PREPARED_CAP+SOURCE_CAP+NORMAL_CAP+4),
        "scope":"one_original_post_import_ExtractionOnly_bindings_root",
        "whole_action_bytes":null,"peak_bytes":null,"rss_bytes":null,"elapsed_ns":null
    }))?;
    let mut callbacks = Observe {
        preparation: &preparation,
        source: &source,
        mode: config.mode,
        entries: 0,
        result: None,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&preparation.rustc_args, &mut callbacks)
    })
    .is_err();
    let entries = callbacks.entries;
    let result = callbacks.result.take();
    drop(callbacks);
    emit(&serde_json::json!({"kind":"attempt","mode":config.mode,
        "callback_count":entries,"compiler_fatal":fatal,
        "returned_kind":match result.as_ref() {
            Some(Ok(Outcome::Verified { .. })) => "verified",
            Some(Ok(Outcome::Denied(_))) => "denied",
            Some(Err(_)) => "error", None => "missing",
        },
        "original_counter_denial":match result.as_ref() {
            Some(Ok(Outcome::Denied(reason))) => Some(*reason), _ => None,
        },
        "diagnostic":result.as_ref().and_then(|r|r.as_ref().err()).map(|e|diagnostic(e.clone())),
        "normal":null,"storage":null,"grants_authority":false}))?;
    // Recheck ORIGINAL immutable inputs before any success/denial completion.
    // Each retained file is read at most three times; no replacement/retry.
    source.recheck()?;
    prepared_guard.recheck()?;
    config_guard.recheck()?;
    if let Some(oracle) = oracle.as_mut() {
        oracle.recheck()?;
    }
    current_preparation(&preparation)?;
    let outcome = finish(entries, fatal, result)?;
    match outcome {
        Outcome::Denied(reason) => {
            if !config.mode.denied() {
                return Err("unexpected denial mode".into());
            }
            emit(
                &serde_json::json!({"kind":"denied","mode":config.mode,"callback_count":entries,
                "original_error_layer":"Storage.Counter","original_error":reason,
                "normal":null,"storage":null,"continuation_completed":false,
                "whole_action_bytes":null,"grants_authority":false}),
            )
        }
        Outcome::Verified { normal, storage } => {
            if config.mode.denied() {
                return Err("unexpected successful denied run".into());
            }
            if let Some(oracle) = oracle.as_ref() {
                compare(&normal, &oracle.bytes)?;
            }
            if (config.mode == Mode::Observed) != storage.is_some() {
                return Err("mode/storage observation join differs".into());
            }
            frame(NORMAL_PREFIX, &normal, NORMAL_CAP)?;
            emit(
                &serde_json::json!({"kind":"complete","mode":config.mode,"callback_count":entries,
                "normal_bytes":normal.len(),"normal_sha256":digest(&normal),
                "exact_ordinary_oracle_equal":oracle.is_some(),
                "storage":storage.map(|r|serde_json::json!({"header_bytes":r.header_bytes,
                    "heap_bytes":r.heap_bytes,"total_bytes":r.total_bytes,"visited_items":r.visited_items})),
                "observation_limit_bytes":OBSERVATION_BYTES,"observation_limit_items":OBSERVATION_ITEMS,
                "scope":"one_original_post_import_ExtractionOnly_bindings_root",
                "logical_btree_payload_not_physical_nodes":true,
                "continuation_completed":true,"recipe_replayed":false,"llvm_emitted":false,
                "whole_action_bytes":null,"peak_bytes":null,"rss_bytes":null,"elapsed_ns":null,
                "grants_authority":false}),
            )
        }
    }
}
#[test]
#[ignore = "requires root-owned fresh original preparation, independent baseline and bounded process interval"]
fn actual_source_bindings_checkpoint_v1() {
    if let Err(error) = child() {
        let _ = emit(
            &serde_json::json!({"kind":"failed","diagnostic":diagnostic(error),
            "normal":null,"storage":null,"whole_action_bytes":null,"grants_authority":false}),
        );
        panic!("genuine bindings checkpoint refused; retain original bounded process and streams");
    }
}
#[path = "source_bindings_checkpoint_controls_v1_tests.rs"]
mod controls;
