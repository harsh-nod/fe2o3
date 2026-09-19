//! Private test-only source descriptor / live-HIR association. No report input.

use std::path::Path;

use fe2o3_source_isa_observation::source_candidate_io_v1::{RetainedSource, publish};
use fe2o3_source_isa_observation::source_edit_v1::{
    MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1, MAX_SOURCE_EDIT_OUTPUT_BYTES_V1,
};

use super::*;

pub(crate) const CANDIDATE_CAP: usize = SOURCE_CAP + 64 * 1024;
const IO_WORK_CAP: usize = 32 * 1024 * 1024;
const IO_STORAGE_CAP: usize = 16 * 1024 * 1024;

/// Conservative cumulative payload envelopes for bounded library calls, not RSS
/// or rustc work. Charges include rejected maximum-length reads, not just bytes
/// in the successful fixture. Every call is prepaid; counters never reset.
#[derive(Debug, Default, Serialize)]
pub(crate) struct IoMeter {
    calls: usize,
    work: usize,
    storage: usize,
}

impl IoMeter {
    fn charge(&mut self, output: usize) -> Result<(), String> {
        let read = MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + 1;
        let work = read
            .checked_mul(4)
            .and_then(|n| output.checked_mul(4).and_then(|o| n.checked_add(o)))
            .ok_or("source-candidate I/O accounting overflow")?;
        let storage = read
            .checked_mul(2)
            .and_then(|n| output.checked_add(1).and_then(|o| n.checked_add(o)))
            .ok_or("source-candidate I/O accounting overflow")?;
        let calls = self
            .calls
            .checked_add(1)
            .ok_or("source-candidate I/O accounting overflow")?;
        let work = self
            .work
            .checked_add(work)
            .ok_or("source-candidate I/O accounting overflow")?;
        let storage = self
            .storage
            .checked_add(storage)
            .ok_or("source-candidate I/O accounting overflow")?;
        if calls > 4 || work > IO_WORK_CAP || storage > IO_STORAGE_CAP {
            return Err("source-candidate I/O envelope exceeded".into());
        }
        self.calls = calls;
        self.work = work;
        self.storage = storage;
        Ok(())
    }
}

/// Move-only descriptor owner. The path is still an input until live-HIR binding.
pub(crate) struct RetainedInput {
    source: RetainedSource,
    path: String,
    pub(crate) io: IoMeter,
}

impl RetainedInput {
    pub(crate) fn open(path: &str, candidate: bool) -> Result<Self, String> {
        let mut io = IoMeter::default();
        io.charge(0)?;
        let source = RetainedSource::open(path)?;
        let cap = if candidate { CANDIDATE_CAP } else { SOURCE_CAP };
        if source.original().is_empty() || source.original().len() > cap {
            return Err("source-candidate fixture byte limit".into());
        }
        Ok(Self {
            source,
            path: path.to_owned(),
            io,
        })
    }

    pub(crate) fn original(&self) -> &[u8] {
        self.source.original()
    }

    pub(crate) fn recheck(&mut self) -> Result<(), String> {
        self.io.charge(0)?;
        self.source.recheck()
    }

    pub(crate) fn publish(&mut self, candidate: &str, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > CANDIDATE_CAP {
            return Err("source-candidate replacement byte limit".into());
        }
        // Stage write/readback plus retained-source recheck, prepaid even when
        // publication refuses. The library independently enforces its own cap.
        self.io.charge(MAX_SOURCE_EDIT_OUTPUT_BYTES_V1)?;
        publish(&mut self.source, candidate, bytes)?;
        Ok(())
    }
}

fn require_file(
    file: &SourceFile,
    input: &RetainedInput,
    meter: &mut ScanMeter,
) -> Result<(), String> {
    let FileName::Real(name) = &file.name else {
        return Err("source-candidate requires a real compiler source file".into());
    };
    let actual = name
        .local_path()
        .ok_or("source-candidate compiler path absent")?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let expected = cwd.join(&input.path);
    let actual = if actual.is_absolute() {
        actual.to_owned()
    } else {
        cwd.join(actual)
    };
    if actual != expected || Path::new(&input.path).is_absolute() {
        return Err("source-candidate selected compiler path differs from retained path".into());
    }
    let bytes = input.original();
    meter.scan(
        bytes
            .len()
            .checked_mul(3)
            .ok_or("source-candidate comparison overflow")?,
    )?;
    let text = std::str::from_utf8(bytes).map_err(|_| "source-candidate original is not UTF-8")?;
    if file.unnormalized_source_len as usize != bytes.len() || !file.src_hash.matches(text) {
        return Err("source-candidate retained bytes differ from parsed compiler input".into());
    }
    require_unchanged_normalization(
        text,
        file.src
            .as_deref()
            .ok_or("source-candidate compiler text absent")?,
    )?;
    Ok(())
}

pub(crate) fn bind_capture(
    tcx: TyCtxt<'_>,
    captured: &mut CapturedBitselect<'_>,
    input: &RetainedInput,
) -> Result<(), String> {
    let file = checked_file(tcx, captured.span, &mut captured.meter)?;
    require_file(&file, input, &mut captured.meter)?;
    captured.meter.scan(input.original().len())?;
    if captured.original.as_bytes() != input.original() {
        return Err("source-candidate capture bytes differ from retained source".into());
    }
    Ok(())
}

pub(crate) fn require_baseline_profile(
    closure: &AuthenticatedCollectedKernelClosureV1<'_>,
    meter: &mut ScanMeter,
) -> Result<(), String> {
    if closure.target.canonical_name() != "gfx942:xnack-" {
        return Err("source-candidate requires authenticated gfx942 xnack-off wave64".into());
    }
    let [root] = closure.roots.as_ref() else {
        return Err("source-candidate baseline root roster".into());
    };
    meter.rows(closure.collection.functions.len())?;
    let mut selected = None;
    for function in &closure.collection.functions {
        if function.instance == root.instance && function.role == CollectedFunctionRole::KernelEntry
        {
            if selected.replace(function).is_some() {
                return Err("source-candidate ambiguous collected source contract".into());
            }
        }
    }
    let bounds = selected
        .and_then(|f| f.frontend_contract.as_ref())
        .and_then(|f| f.contract().launch());
    if bounds.and_then(|b| b.required()).map(|v| v.as_array()) != Some([64, 1, 1])
        || bounds.and_then(|b| b.maximum()).map(|v| v.as_array()) != Some([64, 1, 1])
    {
        return Err("source-candidate requires required and maximum 64x1x1 bounds".into());
    }
    Ok(())
}

pub(crate) struct FreshHeader {
    pub(crate) identities: CanonicalFunctionIdentitiesV1,
    pub(crate) ordinals: [u32; 3],
    pub(crate) names: [String; 3],
}

/// Independently binds fresh candidate source and parameters to its sealed root.
/// No baseline report, expected source name, SSA label or old identity is read.
pub(crate) fn fresh_header<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
    input: &RetainedInput,
    meter: &mut ScanMeter,
) -> Result<FreshHeader, String> {
    let [root] = closure.roots.as_ref() else {
        return Err("source-candidate fresh root roster".into());
    };
    if root.role != CollectedFunctionRole::KernelEntry || !root.instance.args.is_empty() {
        return Err("source-candidate fresh root profile".into());
    }
    let local = root
        .instance
        .def_id()
        .as_local()
        .ok_or("source-candidate fresh root is external")?;
    let body = tcx
        .hir_maybe_body_owned_by(local)
        .ok_or("source-candidate fresh HIR body absent")?;
    let file = checked_file(tcx, body.value.span, meter)?;
    require_file(&file, input, meter)?;
    let text = std::str::from_utf8(input.original()).map_err(|_| "source-candidate UTF-8")?;
    let typeck = tcx.typeck(local);
    meter.rows(body.params.len())?;
    if body.params.len() != 4 {
        return Err("source-candidate fresh parameter profile".into());
    }
    let mut names: [String; 3] = std::array::from_fn(|_| String::new());
    for (index, parameter) in body.params[1..].iter().enumerate() {
        let PatKind::Binding(BindingMode::NONE, _, ident, None) = parameter.pat.kind else {
            return Err("source-candidate fresh parameter is not a plain immutable binding".into());
        };
        if typeck.pat_ty(parameter.pat) != tcx.types.u32 {
            return Err("source-candidate fresh parameter is not u32".into());
        }
        names[index] = name(&file, text, ident, meter)?.0;
    }
    Ok(FreshHeader {
        identities: canonical_function_identities_v1(tcx, root.instance),
        ordinals: [1, 2, 3],
        names,
    })
}

#[test]
fn source_candidate_io_envelopes_are_cumulative_and_refusal_preserves_counters() {
    let mut meter = IoMeter::default();
    for _ in 0..4 {
        meter.charge(0).unwrap();
    }
    let before = (meter.calls, meter.work, meter.storage);
    assert_eq!(
        meter.charge(0).unwrap_err(),
        "source-candidate I/O envelope exceeded"
    );
    assert_eq!((meter.calls, meter.work, meter.storage), before);
    assert!(meter.work >= 4 * (MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + 1));
}
