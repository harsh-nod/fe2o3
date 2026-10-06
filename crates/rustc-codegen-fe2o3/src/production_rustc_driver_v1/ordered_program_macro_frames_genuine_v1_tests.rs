//! Actual one-step macro source through the existing prepared Cargo route.
//! Test-only, one callback, no subprocess, no provider replacement or native work.
use super::super::{
    Callbacks, Compilation, Compiler, Profile, TyCtxt, observe_live_owner,
    transaction_in_active_session_v1,
};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
const PREFIX: &str = "FE2O3_ORDERED_MACRO_FRAMES_V1 ";
const FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/production-extraction-device/src/ordered_program_v32.rs");
const OUTPUT_CAP: usize = 128 * 1024;
static CLAIMED: AtomicBool = AtomicBool::new(false);

fn expected_callsite(source: &[u8]) -> Result<(usize, usize), &'static str> {
    if source != FIXTURE {
        return Err("macro fixture is not current embedded source");
    }
    let source = std::str::from_utf8(source).map_err(|_| "macro source UTF8")?;
    let marker = "#[cfg(feature = \"ordered-program-one-v32\")]\n    let region_value = ";
    if source.matches(marker).count() != 1 {
        return Err("macro selected source ambiguous");
    }
    let start = source.find(marker).ok_or("macro selected source absent")? + marker.len();
    let tail = source.get(start..).ok_or("macro source start")?;
    if !tail.starts_with("amdgpu_ordered_program! {") {
        return Err("macro source spelling");
    }
    let end = start + tail.find("\n    };").ok_or("macro source close")? + "\n    }".len();
    Ok((start, end))
}
fn coordinate(source: &[u8], offset: usize) -> Result<(u64, u64), &'static str> {
    let prefix = std::str::from_utf8(source.get(..offset).ok_or("macro byte range")?)
        .map_err(|_| "macro UTF8 boundary")?;
    Ok((
        prefix.bytes().filter(|byte| *byte == b'\n').count() as u64 + 1,
        prefix
            .rsplit('\n')
            .next()
            .ok_or("macro line")?
            .chars()
            .count() as u64
            + 1,
    ))
}
fn digest_field(value: &Value) -> Result<String, &'static str> {
    let values = value.as_array().ok_or("macro digest array")?;
    if values.len() != 32 {
        return Err("macro digest extent");
    }
    let mut bytes = [0_u8; 32];
    for (to, from) in bytes.iter_mut().zip(values) {
        *to = u8::try_from(from.as_u64().ok_or("macro digest byte")?)
            .map_err(|_| "macro digest byte")?;
    }
    Ok(super::super::super::lower_hex_v1(&bytes))
}
fn validate_binding(origin: &Value, frames: &Value) -> Result<(), &'static str> {
    if origin["schema"] != "fe2o3-diagnostic-ordered-program-origin-v1"
        || frames["schema"] != "fe2o3-diagnostic-ordered-region-macro-frames-v1"
        || origin["canonical_sha256"] != digest_field(&frames["canonical_sha256"])?
        || origin["declared_source_ids"]["frontend_unit"]
            != digest_field(&frames["source_frontend_unit"])?
        || origin["declared_source_ids"]["function"] != digest_field(&frames["source_function"])?
        || origin["declared_source_ids"]["contract"] != digest_field(&frames["source_contract"])?
        || origin["declared_source_ids"]["statement"] != digest_field(&frames["source_statement"])?
        || origin["expansion_chain_sha256"] != digest_field(&frames["expansion_chain_sha256"])?
        || origin["expansion_depth"] != frames["expansion_depth"]
        || frames["frame_order"] != "innermost_to_outermost"
        || frames["origin_scope"] != "whole_ordered_region"
    {
        return Err("macro report identity or profile mismatch");
    }
    for field in [
        "is_llvm_inline_stack",
        "instruction_specific_origins_available",
        "allocator_lifetime_trace_available",
        "authenticates_source",
        "grants_artifact_or_launch_authority",
    ] {
        if frames[field] != false {
            return Err("macro report authority or granularity");
        }
    }
    Ok(())
}
fn validate(origin: &Value, frames: &Value, source: &[u8]) -> Result<(), &'static str> {
    validate_binding(origin, frames)?;
    let rows = frames["frames"].as_array().ok_or("macro frame array")?;
    // Independent fixed fixture expectation, not a repeat-count-derived depth.
    if rows.len() != 1
        || frames["expansion_depth"] != 1
        || rows[0]["ordinal"] != 0
        || rows[0]["kind"] != "macro"
        || rows[0]["macro_name"] != "amdgpu_ordered_program"
    {
        return Err("macro actual fixture frame mismatch");
    }
    let (start, end) = expected_callsite(source)?;
    let (line_start, column_start) = coordinate(source, start)?;
    let (line_end, column_end) = coordinate(source, end)?;
    for call in [&origin["call_site"], &rows[0]["call_site"]] {
        if call["byte_start"] != start as u64
            || call["byte_end"] != end as u64
            || call["line_start"] != line_start
            || call["column_start"] != column_start
            || call["line_end"] != line_end
            || call["column_end"] != column_end
        {
            return Err("macro actual source interval differs");
        }
    }
    if rows[0]["call_site"]["availability"] != "available"
        || origin["call_site"]["file_identity"]
            != digest_field(&rows[0]["call_site"]["file_identity"])?
        || rows[0]["expansion"]["availability"] != "available"
        || origin["expansion"]["file_identity"]
            != digest_field(&rows[0]["expansion"]["file_identity"])?
    {
        return Err("macro compiler file identity mismatch");
    }
    for field in [
        "byte_start",
        "byte_end",
        "line_start",
        "column_start",
        "line_end",
        "column_end",
    ] {
        if origin["expansion"][field] != rows[0]["expansion"][field] {
            return Err("macro expansion interval mismatch");
        }
    }
    let definition = rows[0]["definition_site"]["availability"]
        .as_str()
        .ok_or("macro definition availability")?;
    if !matches!(definition, "available" | "unavailable_dummy_definition") {
        return Err("macro definition availability");
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum FixtureCase {
    One,
    Nested,
}
impl FixtureCase {
    fn depth(self) -> usize {
        match self {
            Self::One => 1,
            Self::Nested => 2,
        }
    }
    fn validate(self, origin: &Value, frames: &Value, source: &[u8]) -> Result<(), &'static str> {
        match self {
            Self::One => validate(origin, frames, source),
            Self::Nested => validate_nested(origin, frames, source),
        }
    }
}
const DEVICE_MACRO_SOURCE: &[u8] = include_bytes!("../../../fe2o3-device/src/ordered_program.rs");
const WRAPPER_NAME: &str = "ordered_program_wrapper";
const WRAPPER_HEAD: &str = "macro_rules! ordered_program_wrapper";
fn unique_range(source: &[u8], needle: &str) -> Result<(usize, usize), &'static str> {
    let text = std::str::from_utf8(source).map_err(|_| "nested source UTF8")?;
    if text.matches(needle).count() != 1 {
        return Err("nested source anchor not unique");
    }
    let start = text.find(needle).ok_or("nested source anchor absent")?;
    Ok((start, start + needle.len()))
}
fn nested_ranges(source: &[u8]) -> Result<[(usize, usize); 4], &'static str> {
    if source != FIXTURE {
        return Err("nested fixture is not current embedded source");
    }
    let (wrapper_start, _) = unique_range(source, WRAPPER_HEAD)?;
    // rustc reports the complete local macro definition, unlike the external
    // diagnostic macro's definition-head span. Derive its end independently
    // from the unique exact fixture closing sequence and following attribute.
    let (wrapper_close, _) = unique_range(
        source,
        "\n    };\n}\n\n#[cfg_attr(feature = \"ordered-program-wrong-launch-v32\",",
    )?;
    let wrapper_end = wrapper_close + "\n    };\n}".len();
    if wrapper_start >= wrapper_end {
        return Err("nested wrapper definition range");
    }
    let wrapper = (wrapper_start, wrapper_end);
    let text = std::str::from_utf8(source).map_err(|_| "nested source UTF8")?;
    let prefix = "#[cfg(feature = \"ordered-program-nested-v32\")]\nmacro_rules! ordered_program_wrapper {\n    ($a:expr, $b:expr, $c:expr) => {\n        ";
    let (at, _) = unique_range(source, prefix)?;
    let inner_start = at + prefix.len();
    if !text[inner_start..].starts_with("amdgpu_ordered_program! {") {
        return Err("nested inner spelling");
    }
    let inner_end = inner_start
        + text[inner_start..]
            .find("\n        }\n    };")
            .ok_or("nested inner close")?
        + "\n        }".len();
    let outer = unique_range(source, "ordered_program_wrapper!(a, b, c)")?;
    let inner_definition =
        unique_range(DEVICE_MACRO_SOURCE, "macro_rules! amdgpu_ordered_program")?;
    Ok([(inner_start, inner_end), outer, wrapper, inner_definition])
}
fn exact_interval(value: &Value, source: &[u8], range: (usize, usize)) -> Result<(), &'static str> {
    let (line_start, column_start) = coordinate(source, range.0)?;
    let (line_end, column_end) = coordinate(source, range.1)?;
    if value["availability"] != "available"
        || value["byte_start"] != range.0 as u64
        || value["byte_end"] != range.1 as u64
        || value["line_start"] != line_start
        || value["column_start"] != column_start
        || value["line_end"] != line_end
        || value["column_end"] != column_end
    {
        return Err("nested exact source interval differs");
    }
    digest_field(&value["file_identity"])?;
    Ok(())
}
fn same_interval(left: &Value, right: &Value) -> Result<(), &'static str> {
    for key in [
        "byte_start",
        "byte_end",
        "line_start",
        "column_start",
        "line_end",
        "column_end",
    ] {
        if left[key] != right[key] {
            return Err("nested compiler interval mismatch");
        }
    }
    Ok(())
}
fn validate_nested(origin: &Value, frames: &Value, source: &[u8]) -> Result<(), &'static str> {
    validate_binding(origin, frames)?;
    let rows = frames["frames"].as_array().ok_or("macro frame array")?;
    if rows.len() != 2
        || frames["expansion_depth"] != 2
        || rows[0]["ordinal"] != 0
        || rows[1]["ordinal"] != 1
        || rows[0]["kind"] != "macro"
        || rows[1]["kind"] != "macro"
        || rows[0]["macro_name"] != "amdgpu_ordered_program"
        || rows[1]["macro_name"] != WRAPPER_NAME
    {
        return Err("nested actual fixture frame mismatch");
    }
    let [inner, outer, wrapper_definition, inner_definition] = nested_ranges(source)?;
    exact_interval(&rows[0]["call_site"], source, inner)?;
    exact_interval(&rows[1]["call_site"], source, outer)?;
    exact_interval(&rows[1]["definition_site"], source, wrapper_definition)?;
    exact_interval(
        &rows[0]["definition_site"],
        DEVICE_MACRO_SOURCE,
        inner_definition,
    )?;
    exact_interval(&rows[1]["expansion"], source, inner)?;
    // The original origin report records source_callsite() (outermost) but
    // expansion() remains the innermost actual MIR call span.
    same_interval(&origin["call_site"], &rows[1]["call_site"])?;
    same_interval(&origin["expansion"], &rows[0]["expansion"])?;
    if rows[0]["expansion"]["availability"] != "available"
        || origin["call_site"]["file_identity"]
            != digest_field(&rows[1]["call_site"]["file_identity"])?
        || origin["expansion"]["file_identity"]
            != digest_field(&rows[0]["expansion"]["file_identity"])?
    {
        return Err("nested compiler file identity mismatch");
    }
    let fixture_file = digest_field(&rows[1]["call_site"]["file_identity"])?;
    for span in [
        &rows[0]["call_site"],
        &rows[1]["definition_site"],
        &rows[1]["expansion"],
    ] {
        if digest_field(&span["file_identity"])? != fixture_file {
            return Err("nested fixture file identity mismatch");
        }
    }
    if digest_field(&rows[0]["definition_site"]["file_identity"])?
        != digest_field(&rows[0]["expansion"]["file_identity"])?
        || digest_field(&rows[0]["expansion_identity"])?
            == digest_field(&rows[1]["expansion_identity"])?
    {
        return Err("nested frame identity mismatch");
    }
    Ok(())
}
fn nested_refusal_controls(
    origin: &Value,
    frames: &Value,
    source: &[u8],
) -> Result<(), &'static str> {
    validate_nested(origin, frames, source)?;
    for fault in 0..6 {
        let mut changed = frames.clone();
        match fault {
            0 => {
                changed["frames"]
                    .as_array_mut()
                    .ok_or("macro frame array")?
                    .reverse();
                changed["frames"][0]["ordinal"] = json!(0);
                changed["frames"][1]["ordinal"] = json!(1);
            }
            1 => {
                changed["frames"]
                    .as_array_mut()
                    .ok_or("macro frame array")?
                    .pop();
            }
            2 => {
                changed["frames"][1]["call_site"]["byte_start"] = json!(0);
            }
            3 => {
                let first = changed["frames"][0]["call_site"].clone();
                changed["frames"][1]["call_site"] = first;
            }
            4 => {
                changed["frames"][1]["definition_site"]["byte_end"] = json!(0);
            }
            5 => {
                let first = changed["frames"][0]["expansion_identity"].clone();
                changed["frames"][1]["expansion_identity"] = first;
            }
            _ => unreachable!(),
        }
        if validate_nested(origin, &changed, source).is_ok() {
            return Err("nested mutated frame accepted");
        }
    }
    Ok(())
}

// Refusal-only diagnostics. This does not participate in acceptance or source
// authority and never writes an observation artifact.
const NESTED_REFUSAL_DIAGNOSTIC_CAP: usize = 4096;
#[derive(serde::Serialize)]
struct NestedExpectedInterval {
    availability: &'static str,
    byte_start: usize,
    byte_end: usize,
    line_start: u64,
    column_start: u64,
    line_end: u64,
    column_end: u64,
}
#[derive(serde::Serialize)]
struct NestedIntervalDiagnostic<'a> {
    label: &'static str,
    source: &'static str,
    expected: NestedExpectedInterval,
    observed: &'a Value,
}
#[derive(serde::Serialize)]
struct NestedRefusalDiagnostic<'a> {
    schema: &'static str,
    refused: bool,
    reason: &'a str,
    file_identity_constraint: &'static str,
    intervals: [NestedIntervalDiagnostic<'a>; 5],
}
struct NestedDiagnosticWriter {
    bytes: [u8; NESTED_REFUSAL_DIAGNOSTIC_CAP],
    len: usize,
}
impl std::io::Write for NestedDiagnosticWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.bytes.len().saturating_sub(self.len) {
            return Err(std::io::Error::other("nested refusal diagnostic bound"));
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn nested_interval_diagnostic<'a>(
    label: &'static str,
    source_name: &'static str,
    source: &[u8],
    range: (usize, usize),
    observed: &'a Value,
) -> Result<NestedIntervalDiagnostic<'a>, &'static str> {
    let (line_start, column_start) = coordinate(source, range.0)?;
    let (line_end, column_end) = coordinate(source, range.1)?;
    Ok(NestedIntervalDiagnostic {
        label,
        source: source_name,
        expected: NestedExpectedInterval {
            availability: "available",
            byte_start: range.0,
            byte_end: range.1,
            line_start,
            column_start,
            line_end,
            column_end,
        },
        observed,
    })
}
fn nested_refusal_diagnostic(
    reason: &str,
    frames: &Value,
    source: &[u8],
) -> Result<String, &'static str> {
    let rows = frames["frames"].as_array().ok_or("macro frame array")?;
    if rows.len() != 2 {
        return Err("nested diagnostic frame count");
    }
    let [inner, outer, wrapper, device] = nested_ranges(source)?;
    let diagnostic = NestedRefusalDiagnostic {
        schema: "fe2o3-refused-nested-macro-intervals-v1",
        refused: true,
        reason,
        // Expected file identities are not invented from source text. Existing
        // digest shape, actual origin and cross-frame identity joins stay exact.
        file_identity_constraint: "32-byte digest; unchanged actual-origin and cross-frame joins",
        intervals: [
            nested_interval_diagnostic(
                "frame0.call_site",
                "fixture",
                source,
                inner,
                &rows[0]["call_site"],
            )?,
            nested_interval_diagnostic(
                "frame1.call_site",
                "fixture",
                source,
                outer,
                &rows[1]["call_site"],
            )?,
            nested_interval_diagnostic(
                "frame1.definition_site",
                "fixture",
                source,
                wrapper,
                &rows[1]["definition_site"],
            )?,
            nested_interval_diagnostic(
                "frame0.definition_site",
                "device_macro",
                DEVICE_MACRO_SOURCE,
                device,
                &rows[0]["definition_site"],
            )?,
            nested_interval_diagnostic(
                "frame1.expansion",
                "fixture",
                source,
                inner,
                &rows[1]["expansion"],
            )?,
        ],
    };
    let mut out = NestedDiagnosticWriter {
        bytes: [0; NESTED_REFUSAL_DIAGNOSTIC_CAP],
        len: 0,
    };
    // Serialize borrowed observed objects directly into fixed storage: even
    // malformed inert inputs cannot allocate a copied unbounded report.
    serde_json::to_writer(&mut out, &diagnostic).map_err(|_| "nested refusal diagnostic bound")?;
    let text =
        std::str::from_utf8(&out.bytes[..out.len]).map_err(|_| "nested refusal diagnostic UTF8")?;
    Ok(text.to_owned())
}
fn macro_refusal_detail(
    case: FixtureCase,
    reason: &'static str,
    frames: &Value,
    source: &[u8],
) -> String {
    if matches!(case, FixtureCase::Nested) {
        nested_refusal_diagnostic(reason, frames, source).unwrap_or_else(|_| reason.to_owned())
    } else {
        reason.to_owned()
    }
}

struct MacroCallbacks<'a> {
    baseline: &'a [u8],
    source: &'a [u8],
    case: FixtureCase,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for MacroCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls = self.calls.saturating_add(1);
        if self.calls != 1 {
            self.result = Some(Err("macro callback repeated".into()));
            return Compilation::Stop;
        }
        self.result = Some((|| {
            let (owner, origin, frames) = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .observe_ordered_program_macro_frames_v1()?;
            let actual = observe_live_owner(&owner, Profile::One, self.baseline)
                .map_err(|_| "macro unchanged original baseline or actual profile differs")?;
            let origin_bytes = origin.bytes()?;
            let frame_bytes = frames.bytes()?;
            if frames.frame_count() != self.case.depth() {
                return Err("macro fixture actual frame count differs from fixed case".into());
            }
            let origin: Value =
                serde_json::from_slice(&origin_bytes).map_err(|_| "macro origin JSON")?;
            let frames: Value =
                serde_json::from_slice(&frame_bytes).map_err(|_| "macro frames JSON")?;
            self.case
                .validate(&origin, &frames, self.source)
                .map_err(|reason| macro_refusal_detail(self.case, reason, &frames, self.source))?;
            // Mutate only inert copies; none can reconstruct the actual owner.
            let mut stale = frames.clone();
            stale["canonical_sha256"][0] = json!(256);
            if self.case.validate(&origin, &stale, self.source).is_ok() {
                return Err("stale macro canonical accepted".into());
            }
            let mut stale = frames.clone();
            stale["frames"][0]["call_site"]["byte_start"] = json!(0);
            if self.case.validate(&origin, &stale, self.source).is_ok() {
                return Err("stale macro callsite accepted".into());
            }
            let mut stale = frames.clone();
            stale["is_llvm_inline_stack"] = json!(true);
            if self.case.validate(&origin, &stale, self.source).is_ok() {
                return Err("macro inline relabel accepted".into());
            }
            // The actual original owner remains live through baseline equality,
            // both bounded reports and all inert refusal controls.
            if matches!(self.case, FixtureCase::Nested) {
                nested_refusal_controls(&origin, &frames, self.source)?;
            }
            let mut result = json!({"origin":origin,"macro_frames":frames,"actual":actual,
                "full_original_baseline_equal_while_owner_live":true,
                "inert_stale_canonical_callsite_inline_refusals":true});
            if matches!(self.case, FixtureCase::Nested) {
                result["inert_nested_order_omission_outer_definition_identity_refusals"] =
                    json!(true);
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "root-supervised current one-step fixture; use existing operational preparation and original baseline"]
fn actual_ordered_macro_frames() {
    let directory = root();
    let before = checked_record(&directory, "one");
    verify_environment(&before);
    let mut source =
        RetainedBytes::open(&fixture().join("src/ordered_program_v32.rs"), 64 * 1024).unwrap();
    expected_callsite(&source.bytes).unwrap();
    let mut baseline =
        RetainedBytes::open(&directory.join("one.baseline-v17.bin"), BASELINE_CAP).unwrap();
    assert!(
        !CLAIMED.swap(true, Ordering::SeqCst),
        "one compiler session per process"
    );
    let mut callbacks = MacroCallbacks {
        baseline: &baseline.bytes,
        source: &source.bytes,
        case: FixtureCase::One,
        calls: 0,
        result: None,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&before.args, &mut callbacks)
    })
    .is_err();
    assert!(!fatal, "original compiler failure is not macro evidence");
    assert_eq!(callbacks.calls, 1);
    let observation = callbacks
        .result
        .take()
        .expect("actual callback absent")
        .expect("actual macro observation refused");
    drop(callbacks);
    baseline.recheck().unwrap();
    source.recheck().unwrap();
    assert_eq!(checked_record(&directory, "one"), before);
    verify_environment(&before);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let value = json!({
        "schema":"fe2o3-actual-ordered-macro-frames-fixture-v1",
        "invocation":before,"observation":observation,
        "source_sha256":digest(&source.bytes),"baseline_sha256":digest(&baseline.bytes),
        "source_rechecked_after_callback":true,"compiler_sessions":1,
        "native_emitted":false,"hardware_observed":false,"artifact_or_launch_authority":false
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() <= OUTPUT_CAP);
    write_new(&directory, "one.macro-frames.json", &value);
    println!("\n{PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}
#[test]
#[ignore = "root-supervised separate nested fixture; use nested preparation and unchanged original baseline"]
fn actual_ordered_nested_macro_frames() {
    let directory = root();
    let before = checked_record(&directory, "nested");
    verify_environment(&before);
    let mut source =
        RetainedBytes::open(&fixture().join("src/ordered_program_v32.rs"), 64 * 1024).unwrap();
    nested_ranges(&source.bytes).unwrap();
    let mut baseline =
        RetainedBytes::open(&directory.join("nested.baseline-v17.bin"), BASELINE_CAP).unwrap();
    assert!(
        !CLAIMED.swap(true, Ordering::SeqCst),
        "one compiler session per process"
    );
    let mut callbacks = MacroCallbacks {
        baseline: &baseline.bytes,
        source: &source.bytes,
        case: FixtureCase::Nested,
        calls: 0,
        result: None,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&before.args, &mut callbacks)
    })
    .is_err();
    assert!(!fatal, "original compiler failure is not macro evidence");
    assert_eq!(callbacks.calls, 1);
    let observation = callbacks
        .result
        .take()
        .expect("actual callback absent")
        .expect("actual macro observation refused");
    drop(callbacks);
    baseline.recheck().unwrap();
    source.recheck().unwrap();
    assert_eq!(checked_record(&directory, "nested"), before);
    verify_environment(&before);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let value = json!({
        "schema":"fe2o3-actual-ordered-macro-frames-fixture-v1",
        "invocation":before,"observation":observation,
        "source_sha256":digest(&source.bytes),"baseline_sha256":digest(&baseline.bytes),
        "source_rechecked_after_callback":true,"compiler_sessions":1,
        "native_emitted":false,"hardware_observed":false,"artifact_or_launch_authority":false
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() <= OUTPUT_CAP);
    write_new(&directory, "nested.macro-frames.json", &value);
    println!("\n{PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}
#[test]
fn current_real_fixture_range_is_unique_and_normalized() {
    let (start, end) = expected_callsite(FIXTURE).unwrap();
    assert!(FIXTURE[start..].starts_with(b"amdgpu_ordered_program! {"));
    assert_eq!(&FIXTURE[end - 1..end], b"}");
    assert!(!FIXTURE.starts_with(&[0xef, 0xbb, 0xbf]));
    assert!(!FIXTURE.contains(&b'\r'));
    assert!(coordinate(FIXTURE, start).unwrap().0 < coordinate(FIXTURE, end).unwrap().0);
}
#[test]
fn changed_or_empty_source_cannot_reuse_expected_span() {
    assert!(expected_callsite(b"").is_err());
    let mut changed = FIXTURE.to_vec();
    changed[0] ^= 1;
    assert!(expected_callsite(&changed).is_err());
}
#[test]
fn macro_digest_decoder_is_exact_32_byte_not_number_coercion() {
    assert!(digest_field(&json!(vec![0; 32])).is_ok());
    assert!(digest_field(&json!(vec![0; 31])).is_err());
    let mut bad = json!(vec![0; 32]);
    bad[0] = json!(256);
    assert!(digest_field(&bad).is_err());
}

#[test]
fn nested_real_fixture_has_distinct_inner_outer_and_definition_ranges() {
    let [inner, outer, definition, device] = nested_ranges(FIXTURE).unwrap();
    assert!(inner.0 < inner.1 && inner.1 < outer.0 && outer.0 < outer.1);
    assert_eq!(
        &FIXTURE[outer.0..outer.1],
        b"ordered_program_wrapper!(a, b, c)"
    );
    assert_eq!(definition, (290, 558));
    assert_eq!(
        &FIXTURE[definition.0..definition.1],
        concat!(
            "macro_rules! ordered_program_wrapper {\n",
            "    ($a:expr, $b:expr, $c:expr) => {\n",
            "        amdgpu_ordered_program! {\n",
            "            gfx942_xnack_off_wave64;\n",
            "            scratch(32); out(33); in(34) = $a; in(35) = $b; in(36) = $c;\n",
            "            mov(out, input0);\n",
            "        }\n",
            "    };\n",
            "}"
        )
        .as_bytes()
    );
    assert_eq!(coordinate(FIXTURE, definition.0), Ok((6, 1)));
    assert_eq!(coordinate(FIXTURE, definition.1), Ok((14, 2)));
    assert_eq!(device, (7894, 7929));
    assert_eq!(
        &DEVICE_MACRO_SOURCE[device.0..device.1],
        b"macro_rules! amdgpu_ordered_program"
    );
    assert!(
        expected_callsite(FIXTURE).is_ok(),
        "old one-step fixture remains selected independently"
    );
}
#[test]
fn nested_source_guard_rejects_mutation_and_unlisted_anchor() {
    let mut changed = FIXTURE.to_vec();
    changed[0] ^= 1;
    assert_eq!(
        nested_ranges(&changed),
        Err("nested fixture is not current embedded source")
    );
    assert!(nested_ranges(b"").is_err());
    assert!(unique_range(b"x x", "x").is_err());
    assert!(unique_range(b"x", "absent").is_err());
}
// These records exercise only the inert validator. They are never offered as
// compiler/source-owner evidence, unlike the ignored actual nested callback.
fn inert_nested_reports() -> (Value, Value) {
    let id = |n: u8| json!(vec![n; 32]);
    let [inner, outer, wrapper, device] = nested_ranges(FIXTURE).unwrap();
    let span = |source: &[u8], range: (usize, usize), file: u8| {
        let start = coordinate(source, range.0).unwrap();
        let end = coordinate(source, range.1).unwrap();
        json!({"availability":"available","byte_start":range.0,"byte_end":range.1,
            "line_start":start.0,"column_start":start.1,"line_end":end.0,"column_end":end.1,
            "file_identity":id(file)})
    };
    let inner_call = span(FIXTURE, inner, 1);
    let outer_call = span(FIXTURE, outer, 1);
    let inner_expansion = span(DEVICE_MACRO_SOURCE, device, 2);
    let mut origin_call = outer_call.clone();
    origin_call["file_identity"] = json!(digest_field(&id(1)).unwrap());
    let mut origin_expansion = inner_expansion.clone();
    origin_expansion["file_identity"] = json!(digest_field(&id(2)).unwrap());
    let origin = json!({"schema":"fe2o3-diagnostic-ordered-program-origin-v1",
        "canonical_sha256":digest_field(&id(3)).unwrap(),"declared_source_ids":{
            "frontend_unit":digest_field(&id(4)).unwrap(),"function":digest_field(&id(5)).unwrap(),
            "contract":digest_field(&id(6)).unwrap(),"statement":digest_field(&id(7)).unwrap()},
        "expansion_chain_sha256":digest_field(&id(8)).unwrap(),"expansion_depth":2,
        "call_site":origin_call,"expansion":origin_expansion});
    let frames = json!({"schema":"fe2o3-diagnostic-ordered-region-macro-frames-v1",
        "canonical_sha256":id(3),"source_frontend_unit":id(4),"source_function":id(5),
        "source_contract":id(6),"source_statement":id(7),"expansion_chain_sha256":id(8),
        "expansion_depth":2,"frame_order":"innermost_to_outermost","origin_scope":"whole_ordered_region",
        "is_llvm_inline_stack":false,"instruction_specific_origins_available":false,
        "allocator_lifetime_trace_available":false,"authenticates_source":false,
        "grants_artifact_or_launch_authority":false,"frames":[
            {"ordinal":0,"kind":"macro","macro_name":"amdgpu_ordered_program","expansion_identity":id(9),
             "call_site":inner_call,"expansion":inner_expansion,
             "definition_site":span(DEVICE_MACRO_SOURCE,device,2)},
            {"ordinal":1,"kind":"macro","macro_name":WRAPPER_NAME,"expansion_identity":id(10),
             "call_site":outer_call,"expansion":span(FIXTURE,inner,1),
             "definition_site":span(FIXTURE,wrapper,1)}]});
    (origin, frames)
}
#[test]
fn nested_inert_positive_and_all_six_mutations_are_closed() {
    let (origin, frames) = inert_nested_reports();
    assert!(validate_nested(&origin, &frames, FIXTURE).is_ok());
    assert!(nested_refusal_controls(&origin, &frames, FIXTURE).is_ok());
}
#[test]
fn nested_identity_and_inline_relabel_refuse() {
    let (origin, frames) = inert_nested_reports();
    for field in [
        "canonical_sha256",
        "source_frontend_unit",
        "source_function",
        "source_contract",
        "source_statement",
        "expansion_chain_sha256",
    ] {
        let mut stale = frames.clone();
        stale[field][0] = json!(255);
        assert_eq!(
            validate_nested(&origin, &stale, FIXTURE),
            Err("macro report identity or profile mismatch")
        );
    }
    let mut stale = frames;
    stale["is_llvm_inline_stack"] = json!(true);
    assert_eq!(
        validate_nested(&origin, &stale, FIXTURE),
        Err("macro report authority or granularity")
    );
}
#[test]
fn nested_outermost_origin_and_expansion_chain_are_not_interchangeable() {
    let (mut origin, frames) = inert_nested_reports();
    origin["call_site"]["byte_start"] = frames["frames"][0]["call_site"]["byte_start"].clone();
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested compiler interval mismatch")
    );
    let (origin, mut frames) = inert_nested_reports();
    frames["frames"][1]["expansion"]["file_identity"][0] = json!(255);
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested fixture file identity mismatch")
    );
}
#[test]
fn nested_case_does_not_expand_old_preparation_roster() {
    assert_eq!(CASES, ["one", "three", "sixteen", "invalid-count"]);
    assert_eq!(feature("nested"), Ok("ordered-program-nested-v32"));
    assert!(positive("nested"));
    for name in ["nested,one", "../nested", "nested2"] {
        assert!(feature(name).is_err());
        assert!(!positive(name));
    }
}

#[test]
fn nested_refusal_diagnostic_has_five_fixed_complete_intervals() {
    let (_, frames) = inert_nested_reports();
    let text = nested_refusal_diagnostic("nested exact source interval differs", &frames, FIXTURE)
        .unwrap();
    assert!(text.len() <= NESTED_REFUSAL_DIAGNOSTIC_CAP);
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["schema"], "fe2o3-refused-nested-macro-intervals-v1");
    assert_eq!(report["refused"], true);
    assert_eq!(report["reason"], "nested exact source interval differs");
    let rows = report["intervals"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    let expected = [
        ("frame0.call_site", &frames["frames"][0]["call_site"]),
        ("frame1.call_site", &frames["frames"][1]["call_site"]),
        (
            "frame1.definition_site",
            &frames["frames"][1]["definition_site"],
        ),
        (
            "frame0.definition_site",
            &frames["frames"][0]["definition_site"],
        ),
        ("frame1.expansion", &frames["frames"][1]["expansion"]),
    ];
    for (row, (label, observed)) in rows.iter().zip(expected) {
        assert_eq!(row["label"], label);
        assert_eq!(&row["observed"], observed);
        for field in [
            "availability",
            "byte_start",
            "byte_end",
            "line_start",
            "column_start",
            "line_end",
            "column_end",
        ] {
            assert_eq!(row["expected"][field], observed[field]);
        }
    }
}
#[test]
fn nested_refusal_diagnostic_preserves_all_five_numeric_mismatches() {
    let (origin, original) = inert_nested_reports();
    for (index, (frame, site)) in [
        (0, "call_site"),
        (1, "call_site"),
        (1, "definition_site"),
        (0, "definition_site"),
        (1, "expansion"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut frames = original.clone();
        frames["frames"][frame][site]["byte_start"] = json!(0);
        let result = validate_nested(&origin, &frames, FIXTURE);
        assert_eq!(result, Err("nested exact source interval differs"));
        let detail =
            macro_refusal_detail(FixtureCase::Nested, result.unwrap_err(), &frames, FIXTURE);
        let report: Value = serde_json::from_str(&detail).unwrap();
        assert_eq!(report["intervals"][index]["observed"]["byte_start"], 0);
        assert_ne!(report["intervals"][index]["expected"]["byte_start"], 0);
        assert_eq!(validate_nested(&origin, &frames, FIXTURE), result);
    }
}
#[test]
fn nested_refusal_diagnostic_bounds_payload_and_falls_back_to_original_error() {
    let (_, mut frames) = inert_nested_reports();
    frames["frames"][0]["call_site"]["extra"] =
        json!("x".repeat(NESTED_REFUSAL_DIAGNOSTIC_CAP + 1));
    assert_eq!(
        nested_refusal_diagnostic("original refusal", &frames, FIXTURE),
        Err("nested refusal diagnostic bound")
    );
    assert_eq!(
        macro_refusal_detail(FixtureCase::Nested, "original refusal", &frames, FIXTURE),
        "original refusal"
    );
    frames["frames"].as_array_mut().unwrap().pop();
    assert_eq!(
        nested_refusal_diagnostic("original refusal", &frames, FIXTURE),
        Err("nested diagnostic frame count")
    );
    assert_eq!(
        macro_refusal_detail(FixtureCase::Nested, "original refusal", &frames, FIXTURE),
        "original refusal"
    );
}
#[test]
fn nested_refusal_diagnostic_keeps_one_step_and_source_rejections_unchanged() {
    let (origin, frames) = inert_nested_reports();
    assert_eq!(
        macro_refusal_detail(
            FixtureCase::One,
            "one-step original refusal",
            &frames,
            FIXTURE
        ),
        "one-step original refusal"
    );
    assert_eq!(
        macro_refusal_detail(FixtureCase::Nested, "source original refusal", &frames, b""),
        "source original refusal"
    );
    assert!(validate_nested(&origin, &frames, FIXTURE).is_ok());
    let _ = nested_refusal_diagnostic("inert diagnostic only", &frames, FIXTURE).unwrap();
    assert!(validate_nested(&origin, &frames, FIXTURE).is_ok());
}


// Independent literal source coordinates: do not derive this observed record
// using nested_ranges() or coordinate(), whose expectations it exercises.
fn literal_local_wrapper_definition() -> Value {
    json!({"availability":"available","byte_start":290,"byte_end":558,
        "line_start":6,"column_start":1,"line_end":14,"column_end":2,
        "file_identity":vec![1_u8;32]})
}
#[test]
fn nested_wrapper_literal_body_endpoints_pass_without_generated_span_oracle() {
    let (origin, mut frames) = inert_nested_reports();
    frames["frames"][1]["definition_site"] = literal_local_wrapper_definition();
    assert_eq!(validate_nested(&origin, &frames, FIXTURE), Ok(()));
    assert_eq!(&FIXTURE[290..326], WRAPPER_HEAD.as_bytes());
    assert_eq!(&FIXTURE[549..558], b"\n    };\n}");
    assert_eq!(&FIXTURE[558..561], b"\n\n#");
}
#[test]
fn nested_wrapper_header_only_definition_is_not_the_local_body() {
    let (origin, mut frames) = inert_nested_reports();
    let mut header = literal_local_wrapper_definition();
    header["byte_end"] = json!(326);
    header["line_end"] = json!(6);
    header["column_end"] = json!(37);
    frames["frames"][1]["definition_site"] = header;
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested exact source interval differs")
    );
}
#[test]
fn nested_wrapper_wrong_end_or_line_refuses_even_with_other_coordinates_exact() {
    let (origin, original) = inert_nested_reports();
    for (field, value) in [
        ("byte_end", 557),
        ("byte_end", 559),
        ("line_end", 13),
        ("line_end", 15),
        ("column_end", 1),
        ("column_end", 3),
    ] {
        let mut frames = original.clone();
        frames["frames"][1]["definition_site"] = literal_local_wrapper_definition();
        frames["frames"][1]["definition_site"][field] = json!(value);
        assert_eq!(
            validate_nested(&origin, &frames, FIXTURE),
            Err("nested exact source interval differs")
        );
    }
}
#[test]
fn nested_wrapper_literal_span_cannot_change_file_macro_or_expansion_identity() {
    let (origin, original) = inert_nested_reports();
    let mut frames = original.clone();
    frames["frames"][1]["definition_site"] = literal_local_wrapper_definition();
    frames["frames"][1]["definition_site"]["file_identity"] = json!(vec![2_u8;32]);
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested fixture file identity mismatch")
    );
    let mut frames = original.clone();
    frames["frames"][1]["macro_name"] = json!("amdgpu_ordered_program");
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested actual fixture frame mismatch")
    );
    let mut frames = original;
    frames["frames"][1]["expansion_identity"] = frames["frames"][0]["expansion_identity"].clone();
    assert_eq!(
        validate_nested(&origin, &frames, FIXTURE),
        Err("nested frame identity mismatch")
    );
}
