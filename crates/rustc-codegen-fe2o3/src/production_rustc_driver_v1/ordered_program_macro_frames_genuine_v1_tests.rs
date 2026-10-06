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
fn validate(origin: &Value, frames: &Value, source: &[u8]) -> Result<(), &'static str> {
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
struct MacroCallbacks<'a> {
    baseline: &'a [u8],
    source: &'a [u8],
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
            if frames.frame_count() != 1 {
                return Err("macro fixture expected exactly one actual frame".into());
            }
            let origin: Value =
                serde_json::from_slice(&origin_bytes).map_err(|_| "macro origin JSON")?;
            let frames: Value =
                serde_json::from_slice(&frame_bytes).map_err(|_| "macro frames JSON")?;
            validate(&origin, &frames, self.source)?;
            // Mutate only inert copies; none can reconstruct the actual owner.
            let mut stale = frames.clone();
            stale["canonical_sha256"][0] = json!(256);
            if validate(&origin, &stale, self.source).is_ok() {
                return Err("stale macro canonical accepted".into());
            }
            let mut stale = frames.clone();
            stale["frames"][0]["call_site"]["byte_start"] = json!(0);
            if validate(&origin, &stale, self.source).is_ok() {
                return Err("stale macro callsite accepted".into());
            }
            let mut stale = frames.clone();
            stale["is_llvm_inline_stack"] = json!(true);
            if validate(&origin, &stale, self.source).is_ok() {
                return Err("macro inline relabel accepted".into());
            }
            // The actual original owner remains live through baseline equality,
            // both bounded reports and all inert refusal controls.
            Ok(
                json!({"origin":origin,"macro_frames":frames,"actual":actual,
                "full_original_baseline_equal_while_owner_live":true,
                "inert_stale_canonical_callsite_inline_refusals":true}),
            )
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
