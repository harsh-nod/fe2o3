//! Diagnostic projection of the existing collected closure, never compiler authority.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::Path;

use rustc_middle::ty::TyCtxt;
use rustc_span::{FileName, Span};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use super::{AuthenticatedCollectedKernelClosureV1, CollectedFunctionRole};
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_source_provenance_v1,
};

#[path = "production_source_census_coordinates_v1.rs"]
mod coordinates;
#[path = "production_source_census_io_v1.rs"]
mod output;

pub(crate) const OUTPUT_ENV: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1";
pub(crate) const RUN_ID_ENV: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1";
const MAX_FUNCTIONS: usize = 512;
const MAX_FILES: usize = 128;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_EXPANSION_DEPTH: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_ARGUMENTS: usize = 4096;

#[derive(Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum ExtractionMode {
    SemanticMir,
    RankedMemory,
    FixedCheckedOutput {
        policy: u16,
    },
    Llvm {
        expected_target: Option<&'static str>,
    },
    CompilerHandoff {
        version: u16,
        expected_target: Option<&'static str>,
    },
    SimulationBundle {
        version: u16,
    },
}

#[derive(Serialize)]
#[serde(tag = "status", content = "value", rename_all = "kebab-case")]
enum Observation<T> {
    Available(T),
    Unavailable(&'static str),
}

impl<T> From<Result<T, &'static str>> for Observation<T> {
    fn from(value: Result<T, &'static str>) -> Self {
        match value {
            Ok(value) => Self::Available(value),
            Err(reason) => Self::Unavailable(reason),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Selection {
    target: String,
    functions: Vec<Function>,
    files: Vec<SourceFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Function {
    function_identity: String,
    definition_identity: String,
    monomorphization_identity: String,
    role: &'static str,
    export_name: String,
    logical_name: Option<String>,
    definition: Observation<SourceSpan>,
    identifier: Observation<SourceSpan>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceFile {
    identity: String,
    display_path: String,
    compiled_source_hash: String,
    original_sha256: String,
    original_bytes: usize,
    normalized_bytes: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSpan {
    expansion: SourceOrigin,
    call_site: SourceOrigin,
    expansion_chain_sha256: String,
    expansion_depth: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceOrigin {
    file: usize,
    coordinates: coordinates::SourceCoordinatesV1,
}

/// Owned by one existing rustc driver invocation. It cannot issue a transaction.
pub(crate) struct Recorder {
    sink: output::CensusSinkV1,
    arguments: Vec<String>,
    working_directory: String,
    extraction_mode: ExtractionMode,
    run_id: String,
    selection: RefCell<Observation<Selection>>,
    observed: std::cell::Cell<bool>,
}

impl Recorder {
    pub(crate) fn from_environment(
        args: &[String],
        outputs: &[&Path],
        mode: ExtractionMode,
    ) -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os(OUTPUT_ENV) else {
            return Ok(None);
        };
        let path = Path::new(&path);
        reject_output_alias(path, outputs)?;
        // The wrapper publishes this sidecar after the driver returns.
        if let Some(binding) = std::env::var_os("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1") {
            reject_output_alias(path, &[Path::new(&binding)])?;
        }
        let run_id =
            std::env::var(RUN_ID_ENV).map_err(|_| "diagnostic census run ID unavailable")?;
        Self::create(path, args, mode, &run_id).map(Some)
    }

    fn create(
        path: &Path,
        args: &[String],
        mode: ExtractionMode,
        run_id: &str,
    ) -> Result<Self, String> {
        if run_id.len() != 64
            || !run_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("diagnostic census run ID must be 64 lowercase hexadecimal digits".into());
        }
        if path.as_os_str().is_empty() || args.len() > MAX_ARGUMENTS {
            return Err("invalid diagnostic census request".into());
        }
        let bytes = args
            .iter()
            .try_fold(0_usize, |size, arg| size.checked_add(arg.len()));
        if bytes.is_none_or(|bytes| bytes > MAX_ARGUMENT_BYTES) {
            return Err("diagnostic census argument bound exceeded".into());
        }
        let working_directory = std::env::current_dir()
            .map_err(|_| "diagnostic census working directory unavailable")?
            .into_os_string()
            .into_string()
            .map_err(|_| "diagnostic census working directory is not UTF-8")?;
        bounded_text(&working_directory).map_err(str::to_owned)?;
        let sink = output::CensusSinkV1::create(path).map_err(|error| error.to_string())?;
        Ok(Self {
            sink,
            arguments: args.to_vec(),
            working_directory,
            extraction_mode: mode,
            run_id: run_id.to_owned(),
            selection: RefCell::new(Observation::Unavailable("collection not reached")),
            observed: std::cell::Cell::new(false),
        })
    }

    pub(crate) fn observe<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
    ) {
        *self.selection.borrow_mut() = if self.observed.replace(true) {
            Observation::Unavailable("multiple collections in one diagnostic request")
        } else {
            capture(tcx, closure).into()
        };
    }

    pub(crate) fn finish(self, extraction_succeeded: bool) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report {
            schema: &'static str,
            diagnostic_only: bool,
            qualified: bool,
            authenticates_compiler_execution: bool,
            extraction_succeeded: bool,
            arguments: Vec<String>,
            working_directory: String,
            extraction_mode: ExtractionMode,
            run_id: String,
            selection: Observation<Selection>,
        }
        self.sink
            .finish(&Report {
                schema: "fe2o3-diagnostic-source-census-v1",
                diagnostic_only: true,
                qualified: false,
                authenticates_compiler_execution: false,
                extraction_succeeded,
                arguments: self.arguments,
                working_directory: self.working_directory,
                extraction_mode: self.extraction_mode,
                run_id: self.run_id,
                selection: self.selection.into_inner(),
            })
            .map_err(|error| error.to_string())
    }
}

fn capture<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
) -> Result<Selection, &'static str> {
    if closure.collection.functions.len() > MAX_FUNCTIONS {
        return Err("function bound exceeded");
    }
    let mut sources = Sources::default();
    let mut functions = Vec::with_capacity(closure.collection.functions.len());
    for function in &closure.collection.functions {
        let identities = canonical_function_identities_v1(tcx, function.instance);
        let definition = function.instance.def_id();
        let identifier = tcx
            .def_ident_span(definition)
            .ok_or("identifier span unavailable")
            .and_then(|span| {
                let name = tcx
                    .opt_item_name(definition)
                    .ok_or("identifier name unavailable")?;
                let observed = sources.span(tcx, span)?;
                let origin = &observed.expansion;
                let source = &sources.bytes[origin.file];
                let coordinates = origin.coordinates;
                let text = source
                    .get(coordinates.original_start as usize..coordinates.original_end as usize)
                    .ok_or("identifier byte range unavailable")?;
                if text.strip_prefix("r#").unwrap_or(text) != name.as_str() {
                    return Err("identifier token does not match compiled definition");
                }
                Ok(observed)
            });
        functions.push(Function {
            function_identity: hex(identities.function().as_bytes()),
            definition_identity: hex(identities.item_definition().as_bytes()),
            monomorphization_identity: hex(identities.monomorphization().as_bytes()),
            role: match function.role {
                CollectedFunctionRole::KernelEntry => "kernel-entry",
                CollectedFunctionRole::InternalHelper => "internal-helper",
                CollectedFunctionRole::DeviceFfiExport => "device-ffi-export",
            },
            export_name: bounded_text(&function.export_name)?.to_owned(),
            logical_name: function
                .logical_name
                .as_deref()
                .map(bounded_text)
                .transpose()?
                .map(str::to_owned),
            definition: sources.span(tcx, tcx.def_span(definition)).into(),
            identifier: identifier.into(),
        });
    }
    functions.sort_by(|left, right| left.function_identity.cmp(&right.function_identity));
    Ok(Selection {
        target: bounded_text(closure.target.canonical_name())?.to_owned(),
        functions,
        files: sources.files,
    })
}

#[derive(Default)]
struct Sources {
    indices: BTreeMap<[u8; 32], Result<usize, &'static str>>,
    files: Vec<SourceFile>,
    bytes: Vec<String>,
    read_bytes: usize,
}

impl Sources {
    fn span(&mut self, tcx: TyCtxt<'_>, span: Span) -> Result<SourceSpan, &'static str> {
        if span.is_dummy() {
            return Err("source span unavailable");
        }
        // Bound ancestry before calling the canonical adapter's source_callsite.
        let mut call_site = span;
        for depth in 0..=MAX_EXPANSION_DEPTH {
            let Some(parent) = call_site.parent_callsite() else {
                break;
            };
            if depth == MAX_EXPANSION_DEPTH {
                return Err("macro expansion bound exceeded");
            }
            call_site = parent;
        }
        let expansion_coordinates = checked_coordinates(tcx, span)?;
        let call_coordinates = checked_coordinates(tcx, call_site)?;
        let canonical = canonical_source_provenance_v1(tcx, span, MAX_EXPANSION_DEPTH)
            .map_err(|_| "canonical source provenance unavailable")?;
        let provenance = canonical.provenance();
        let expansion = provenance
            .expansion()
            .ok_or("expansion origin unavailable")?;
        let call = provenance
            .call_site()
            .ok_or("callsite origin unavailable")?;
        Ok(SourceSpan {
            expansion: SourceOrigin {
                file: self.file(tcx, span, *expansion.file().as_bytes())?,
                coordinates: expansion_coordinates,
            },
            call_site: SourceOrigin {
                file: self.file(tcx, call_site, *call.file().as_bytes())?,
                coordinates: call_coordinates,
            },
            expansion_chain_sha256: hex(&canonical.expansion_chain_sha256()),
            expansion_depth: canonical.expansion_depth(),
        })
    }

    fn file(
        &mut self,
        tcx: TyCtxt<'_>,
        span: Span,
        identity: [u8; 32],
    ) -> Result<usize, &'static str> {
        if let Some(result) = self.indices.get(&identity) {
            if let Ok(index) = result {
                let current = tcx.sess.source_map().lookup_source_file(span.lo());
                let retained = &self.files[*index];
                if current.src_hash.to_string() != retained.compiled_source_hash
                    || current.unnormalized_source_len as usize != retained.original_bytes
                    || current.normalized_source_len.0 != retained.normalized_bytes
                {
                    return Err("source file identity has conflicting compiler metadata");
                }
            }
            return *result;
        }
        if self.indices.len() >= MAX_FILES {
            return Err("source file bound exceeded");
        }
        let result = self.read_file(tcx, span, identity);
        self.indices.insert(identity, result);
        result
    }

    fn read_file(
        &mut self,
        tcx: TyCtxt<'_>,
        span: Span,
        identity: [u8; 32],
    ) -> Result<usize, &'static str> {
        let file = tcx.sess.source_map().lookup_source_file(span.lo());
        let FileName::Real(name) = &file.name else {
            return Err("source has no real file");
        };
        let path = name.local_path().ok_or("local source path unavailable")?;
        let display_path = path.to_str().ok_or("source path is not UTF-8")?;
        bounded_text(display_path)?;
        let length = file.unnormalized_source_len as usize;
        let read_limit = length.checked_add(1).ok_or("source byte bound exceeded")?;
        if length > MAX_FILE_BYTES || read_limit > MAX_SOURCE_BYTES.saturating_sub(self.read_bytes)
        {
            return Err("source byte bound exceeded");
        }
        self.read_bytes += read_limit;
        let bytes = read_original_source(&file, path)?;
        let index = self.files.len();
        self.files.push(SourceFile {
            identity: hex(&identity),
            display_path: display_path.to_owned(),
            compiled_source_hash: file.src_hash.to_string(),
            original_sha256: hex(&Sha256::digest(bytes.as_bytes())),
            original_bytes: bytes.len(),
            normalized_bytes: file.normalized_source_len.0,
        });
        self.bytes.push(bytes);
        Ok(index)
    }
}

fn read_original_source(
    file: &rustc_span::SourceFile,
    path: &Path,
) -> Result<String, &'static str> {
    let length = file.unnormalized_source_len as usize;
    if length > MAX_FILE_BYTES {
        return Err("source byte bound exceeded");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let input = options.open(path).map_err(|_| "source bytes unavailable")?;
    if !input
        .metadata()
        .map_err(|_| "source metadata unavailable")?
        .is_file()
    {
        return Err("source is not a regular file");
    }
    let mut bytes = String::new();
    input
        .take(length as u64 + 1)
        .read_to_string(&mut bytes)
        .map_err(|_| "source bytes unavailable or not UTF-8")?;
    if bytes.len() != length || !file.src_hash.matches(&bytes) {
        return Err("source bytes differ from rustc input");
    }
    Ok(bytes)
}

fn checked_coordinates(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<coordinates::SourceCoordinatesV1, &'static str> {
    let files = tcx.sess.source_map().files();
    let index = files
        .partition_point(|file| file.start_pos <= span.lo())
        .checked_sub(1)
        .ok_or("source file unavailable")?;
    coordinates::source_coordinates_v1(&files[index], span)
}

fn bounded_text(text: &str) -> Result<&str, &'static str> {
    if text.len() > MAX_TEXT_BYTES {
        Err("text bound exceeded")
    } else {
        Ok(text)
    }
}

fn reject_output_alias(path: &Path, outputs: &[&Path]) -> Result<(), String> {
    let destination = |path: &Path| {
        let name = path
            .file_name()
            .ok_or("diagnostic output has no filename")?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = parent
            .canonicalize()
            .map_err(|_| "diagnostic output parent unavailable")?;
        Ok::<_, String>(parent.join(name))
    };
    let report = destination(path)?;
    for output in outputs {
        match std::fs::symlink_metadata(output) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(
                    "diagnostic census cannot accompany a symlink extraction output".into(),
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("extraction output metadata unavailable".into()),
        }
        if report == destination(output)? {
            return Err("diagnostic census path aliases extraction output".into());
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("String writes are infallible");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    const RUN_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn unavailable_collection_cannot_report_selection_or_qualification() {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-census-unavailable");
        let path = scratch.path().join("report.json");
        let args = vec!["rustc".to_owned(), "--cfg=feature=\"one\"".to_owned()];
        Recorder::create(&path, &args, ExtractionMode::RankedMemory, RUN_ID)
            .unwrap()
            .finish(false)
            .unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(report["schema"], "fe2o3-diagnostic-source-census-v1");
        assert_eq!(report["arguments"], serde_json::json!(args));
        assert_eq!(report["runId"], RUN_ID);
        assert_eq!(report["extractionMode"]["kind"], "ranked-memory");
        assert_eq!(report["diagnosticOnly"], true);
        for field in [
            "qualified",
            "authenticatesCompilerExecution",
            "extractionSucceeded",
        ] {
            assert_eq!(report[field], false);
        }
        assert_eq!(report["selection"]["status"], "unavailable");
        assert_eq!(report["selection"]["value"], "collection not reached");
    }

    #[test]
    fn request_bounds_precede_output_creation() {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-census-request-cap");
        let path = scratch.path().join("report.json");
        for args in [
            vec![String::new(); MAX_ARGUMENTS + 1],
            vec!["x".repeat(MAX_ARGUMENT_BYTES + 1)],
        ] {
            assert!(Recorder::create(&path, &args, ExtractionMode::SemanticMir, RUN_ID).is_err());
            assert!(!path.exists());
        }
        assert!(Recorder::create(Path::new(""), &[], ExtractionMode::SemanticMir, RUN_ID).is_err());
        for run_id in ["", "short", &"A".repeat(64), &"g".repeat(64)] {
            assert!(Recorder::create(&path, &[], ExtractionMode::SemanticMir, run_id).is_err());
            assert!(!path.exists());
        }
        assert!(bounded_text(&"x".repeat(MAX_TEXT_BYTES)).is_ok());
        assert!(bounded_text(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
    }

    #[test]
    fn census_cannot_reserve_the_extraction_output_even_through_parent_aliases() {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-census-output-alias");
        let output = scratch.path().join("bundle");
        let report = scratch.path().join("./bundle");
        assert!(reject_output_alias(&report, &[&output]).is_err());
        assert!(reject_output_alias(&scratch.path().join("report"), &[&output]).is_ok());
        #[cfg(unix)]
        {
            let link = scratch.path().join("alias");
            std::os::unix::fs::symlink(scratch.path(), &link).unwrap();
            assert!(reject_output_alias(&link.join("bundle"), &[&output]).is_err());
            let dangling = scratch.path().join("dangling-output");
            std::os::unix::fs::symlink(&output, &dangling).unwrap();
            assert!(reject_output_alias(&output, &[&dangling]).is_err());
            assert!(!output.exists());
            std::fs::write(&output, "existing artifact").unwrap();
            assert!(reject_output_alias(&scratch.path().join("report"), &[&dangling]).is_err());
            assert_eq!(
                std::fs::read_to_string(&output).unwrap(),
                "existing artifact"
            );
            std::fs::remove_file(&output).unwrap();
        }
        assert!(!output.exists());
    }

    #[test]
    fn original_source_hash_refuses_stale_normalized_missing_and_nonregular_inputs() {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-census-source-hash");
        let path = scratch.path().join("source.rs");
        let original = "\u{feff}// utf8: \u{00e9}\r\nfn r#source() {}\r\n";
        let file = rustc_span::SourceFile::new(
            FileName::Custom("fixture.rs".into()),
            original.into(),
            rustc_span::SourceFileHashAlgorithm::Sha256,
            None,
        )
        .unwrap();
        assert!(read_original_source(&file, &path).is_err());
        assert!(read_original_source(&file, scratch.path()).is_err());
        std::fs::write(&path, original).unwrap();
        assert_eq!(read_original_source(&file, &path).unwrap(), original);
        for changed in [
            original.replace("source", "change"),
            file.src.as_deref().unwrap().to_string(),
            format!("{original} "),
            original[..original.len() - 1].to_owned(),
        ] {
            std::fs::write(&path, changed).unwrap();
            assert_eq!(
                read_original_source(&file, &path).unwrap_err(),
                "source bytes differ from rustc input"
            );
        }
        std::fs::write(&path, [0xff]).unwrap();
        assert!(read_original_source(&file, &path).is_err());
    }
}
