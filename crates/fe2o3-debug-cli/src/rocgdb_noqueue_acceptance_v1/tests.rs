//! CPU-only fake-MI controls. Never creates a process, opens KFD, or attaches.
//! The parser and model are NOT wired to a native acceptance API.
#[path = "controller.rs"]
mod controller;
#[path = "parser.rs"]
mod parser;
use controller::{ProcessStamp, TranscriptModel};
use parser::{MemoryObject, POST_HOST_SYMBOL, PRE_HOST_SYMBOL, Refusal};
use sha2::{Digest, Sha256};

fn stamp() -> ProcessStamp {
    ProcessStamp {
        pid: 42,
        start_ticks: 12345,
        executable_sha256: [7; 32],
    }
}
fn digest() -> [u8; 32] {
    Sha256::digest(b"\x7fELF").into()
}
fn model() -> TranscriptModel {
    TranscriptModel::new(stamp(), 4, digest()).unwrap()
}
fn stop(symbol: &str, bp: u32) -> Vec<u8> {
    format!("*stopped,reason=\"breakpoint-hit\",bkptno=\"{bp}\",frame={{addr=\"0x1000\",func=\"{symbol}\",args=[],arch=\"i386:x86-64\"}},thread-id=\"1\",stopped-threads=\"all\"\n").into_bytes()
}
fn library(uri: &str, compat: &str) -> String {
    format!(
        "{{id=\"gpu-object\",target-name=\"{uri}\",host-name=\"{uri}\",symbols-loaded=\"{compat}\",thread-group=\"i1\",ranges=[{{from=\"0x9000\",to=\"0xa000\"}}]}}"
    )
}
fn libraries(rows: &str, token: u64) -> Vec<u8> {
    format!("{token}^done,shared-libraries=[{rows}]\n").into_bytes()
}
fn published(compat: &str) -> Vec<u8> {
    libraries(&library("memory://42#offset=0x4000&size=4", compat), 17)
}
fn memory(contents: &str) -> Vec<u8> {
    format!("18^done,memory=[{{begin=\"0x4000\",offset=\"0x0\",end=\"0x4004\",contents=\"{contents}\"}}]\n").into_bytes()
}
fn selected() -> MemoryObject {
    MemoryObject {
        pid: 42,
        address: 0x4000,
        bytes: 4,
    }
}
fn at_publication() -> TranscriptModel {
    let mut x = model();
    x.cold_stop(&stop(PRE_HOST_SYMBOL, 1), stamp()).unwrap();
    x.cold_list(&libraries("", 16), 16, stamp()).unwrap();
    x.begin_activation(stamp()).unwrap();
    x.published_stop(&stop(POST_HOST_SYMBOL, 2), stamp())
        .unwrap();
    x
}
fn at_content() -> TranscriptModel {
    let mut x = at_publication();
    x.published_list(&published("1"), 17, stamp()).unwrap();
    x.original_elf(&memory("7f454c46"), 18, stamp()).unwrap();
    x
}

#[test]
fn exact_memory_uri_matches_only_expected_process_extent() {
    assert_eq!(
        parser::parse_memory_uri(b"memory://42#offset=0x4000&size=4", 42, 4),
        Ok(selected())
    );
    for uri in [
        "memory://43#offset=0x4000&size=4",
        "memory://042#offset=0x4000&size=4",
        "memory://42/mem#offset=0x4000&size=4",
        "memory://42?offset=0x4000&size=4",
        "memory://42#offset=0x4000&size=5",
        "memory://42#offset=0x0&size=4",
        "memory://42#offset=0xffffffffffffffff&size=4",
        "memory://42#offset=0x4000&size=04",
        "memory://42#offset=0x4000&size=4&more=1",
        "file:///artifact.hsaco",
    ] {
        assert!(
            parser::parse_memory_uri(uri.as_bytes(), 42, 4).is_err(),
            "{uri}"
        );
    }
}
#[test]
fn memory_uri_profile_and_extent_are_bounded_before_parse() {
    assert_eq!(
        parser::parse_memory_uri(b"memory://42#offset=0x1&size=4", 0, 4),
        Err(Refusal::Bound)
    );
    assert_eq!(
        parser::parse_memory_uri(b"memory://42#offset=0x1&size=4", 42, 65537),
        Err(Refusal::Bound)
    );
    assert!(parser::parse_memory_uri(&[b'x'; 128], 42, 4).is_err());
}
#[test]
fn initial_empty_list_is_not_a_registration_or_acceptance_result() {
    assert_eq!(
        parser::parse_libraries(&libraries("", 16), 16, 42, 4),
        Ok(None)
    );
    assert!(model().finish().is_err());
}
#[test]
fn compatibility_symbols_loaded_is_never_evidence() {
    for value in ["0", "1"] {
        assert_eq!(
            parser::parse_libraries(&published(value), 17, 42, 4),
            Ok(Some(selected()))
        );
    }
    // An empty list with unrelated compatibility-looking text is not accepted.
    assert!(parser::parse_libraries(b"17^done,symbols-loaded=\"1\"\n", 17, 42, 4).is_err());
}
#[test]
fn shared_library_tokens_classes_unknown_fields_and_crashes_refuse() {
    for line in [
        b"16^done,shared-libraries=[]\n".as_slice(),
        b"^done,shared-libraries=[]\n",
        b"17^error,msg=\"failed\"\n",
        b"17^done,shared-libraries=[],extra=\"true\"\n",
        b"17^done,shared-libraries=[],shared-libraries=[]\n",
        b"process exited successfully\n",
        b"17^done,shared-libraries=[]",
    ] {
        assert!(parser::parse_libraries(line, 17, 42, 4).is_err());
    }
}
#[test]
fn duplicate_foreign_group_uri_and_unbounded_rows_refuse() {
    let row = library("memory://42#offset=0x4000&size=4", "1");
    assert!(parser::parse_libraries(&libraries(&format!("{row},{row}"), 17), 17, 42, 4).is_err());
    let other = row.replace("gpu-object", "second");
    assert!(parser::parse_libraries(&libraries(&format!("{row},{other}"), 17), 17, 42, 4).is_err());
    for changed in [
        row.replace("memory://42", "memory://43"),
        row.replace("thread-group=\"i1\"", "thread-group=\"i2\""),
        row.replace("host-name=\"memory://42", "host-name=\"memory://99"),
        row.replace("ranges=[{from=\"0x9000\",to=\"0xa000\"}]", "ranges=[]"),
        row.replace("to=\"0xa000\"", "to=\"0x9000\""),
    ] {
        assert!(parser::parse_libraries(&libraries(&changed, 17), 17, 42, 4).is_err());
    }
    assert!(parser::parse_libraries(&libraries(&vec![row; 65].join(","), 17), 17, 42, 4).is_err());
}
#[test]
fn host_libraries_remain_separate_from_exact_memory_object() {
    let host = r#"{id="libc",target-name="/lib/libc.so.6",host-name="/lib/libc.so.6",ranges=[]}"#;
    let gpu = library("memory://42#offset=0x4000&size=4", "0");
    assert_eq!(
        parser::parse_libraries(&libraries(&format!("{host},{gpu}"), 17), 17, 42, 4),
        Ok(Some(selected()))
    );
    assert_eq!(
        parser::parse_libraries(&libraries(host, 17), 17, 42, 4),
        Ok(None)
    );
}
#[test]
fn exact_original_content_hash_and_range_are_required() {
    assert_eq!(
        parser::parse_original_elf(&memory("7f454c46"), 18, selected(), digest()),
        Ok(())
    );
    for changed in [
        memory("7f454c47"),
        memory("7f454c"),
        memory("7F454c46"),
        String::from_utf8(memory("7f454c46"))
            .unwrap()
            .replace("0x4004", "0x4005")
            .into_bytes(),
        String::from_utf8(memory("7f454c46"))
            .unwrap()
            .replace("offset=\"0x0\"", "offset=\"0x1\"")
            .into_bytes(),
        String::from_utf8(memory("7f454c46"))
            .unwrap()
            .replace("0x4000", "0x5000")
            .into_bytes(),
    ] {
        assert!(parser::parse_original_elf(&changed, 18, selected(), digest()).is_err());
    }
    assert!(parser::parse_original_elf(&memory("7f454c46"), 19, selected(), digest()).is_err());
    assert!(parser::parse_original_elf(&memory("7f454c46"), 18, selected(), [0; 32]).is_err());
}
#[test]
fn unknown_memory_members_multiple_fragments_and_oversize_inputs_refuse() {
    for line in [
        b"18^done,memory=[]\n".as_slice(),
        b"18^done,memory=[{begin=\"0x4000\",offset=\"0x0\",end=\"0x4004\",contents=\"7f454c46\",extra=\"x\"}]\n",
        b"18^done,memory=[{begin=\"0x4000\",offset=\"0x0\",end=\"0x4004\",contents=\"7f454c46\"},{begin=\"0x4000\",offset=\"0x0\",end=\"0x4004\",contents=\"7f454c46\"}]\n",
    ] { assert!(parser::parse_original_elf(line, 18, selected(), digest()).is_err()); }
    assert!(
        parser::parse_original_elf(&vec![b'x'; 192 * 1024 + 1], 18, selected(), digest()).is_err()
    );
}
#[test]
fn only_fixed_host_stops_with_exact_thread_breakpoint_and_empty_args_parse() {
    for (name, bp) in [(PRE_HOST_SYMBOL, 1), (POST_HOST_SYMBOL, 2)] {
        let line = stop(name, bp);
        assert!(parser::parse_host_stop(&line, bp.to_string().as_bytes(), b"1", name).is_ok());
    }
    let base = String::from_utf8(stop(POST_HOST_SYMBOL, 2)).unwrap();
    for changed in [
        base.replace("i386:x86-64", "amdgcn"),
        base.replace("thread-id=\"1\"", "thread-id=\"2\""),
        base.replace("bkptno=\"2\"", "bkptno=\"1\""),
        base.replace("stopped-threads=\"all\"", "stopped-threads=\"1\""),
        base.replace("breakpoint-hit", "signal-received"),
        base.replace("args=[]", "args=[{name=\"pointer\",value=\"0x4000\"}]"),
        base.replace(POST_HOST_SYMBOL, "some_kernel"),
    ] {
        assert!(parser::parse_host_stop(changed.as_bytes(), b"2", b"1", POST_HOST_SYMBOL).is_err());
    }
    assert!(parser::parse_host_stop(&stop(PRE_HOST_SYMBOL, 1), b"1", b"1", "some_kernel").is_err());
}
#[test]
fn complete_fake_transcript_matches_bytes_but_grants_no_native_authority() {
    let mut x = at_content();
    x.exit_observation(stamp(), Some(0), true, true).unwrap();
    let result = x.finish().unwrap();
    assert_eq!(result.artifact_bytes, 4);
    assert_eq!(result.artifact_sha256, digest());
    assert!(!result.runtime_loaded_success_observed);
    assert!(!result.physical_register_capture);
    assert!(!result.process_control_authority);
}
#[test]
fn metadata_version_or_exit_zero_cannot_skip_any_state() {
    let mut x = model();
    assert_eq!(
        x.exit_observation(stamp(), Some(0), true, true),
        Err(Refusal::State)
    );
    assert!(x.finish().is_err());
    let mut x = model();
    assert!(
        x.published_stop(&stop(POST_HOST_SYMBOL, 2), stamp())
            .is_err()
    );
    assert!(x.finish().is_err());
    let mut x = at_publication();
    assert!(x.original_elf(&memory("7f454c46"), 18, stamp()).is_err());
    assert!(x.finish().is_err());
}
#[test]
fn preactivation_presence_cannot_count_as_successful_transition() {
    let mut x = model();
    x.cold_stop(&stop(PRE_HOST_SYMBOL, 1), stamp()).unwrap();
    assert!(x.cold_list(&published("1"), 17, stamp()).is_err());
    assert!(x.begin_activation(stamp()).is_err());
    assert!(x.finish().is_err());
}
#[test]
fn process_pid_start_or_executable_drift_permanently_poisons_model() {
    for current in [
        ProcessStamp { pid: 43, ..stamp() },
        ProcessStamp {
            start_ticks: 12346,
            ..stamp()
        },
        ProcessStamp {
            executable_sha256: [8; 32],
            ..stamp()
        },
    ] {
        let mut x = at_publication();
        assert_eq!(
            x.published_list(&published("1"), 17, current),
            Err(Refusal::Changed)
        );
        assert_eq!(
            x.published_list(&published("1"), 17, stamp()),
            Err(Refusal::State)
        );
        assert!(x.finish().is_err());
    }
}
#[test]
fn cancellation_at_each_phase_cannot_retry_or_claim_completion() {
    for stage in 0..7 {
        let mut x = model();
        if stage >= 1 {
            x.cold_stop(&stop(PRE_HOST_SYMBOL, 1), stamp()).unwrap();
        }
        if stage >= 2 {
            x.cold_list(&libraries("", 16), 16, stamp()).unwrap();
        }
        if stage >= 3 {
            x.begin_activation(stamp()).unwrap();
        }
        if stage >= 4 {
            x.published_stop(&stop(POST_HOST_SYMBOL, 2), stamp())
                .unwrap();
        }
        if stage >= 5 {
            x.published_list(&published("1"), 17, stamp()).unwrap();
        }
        if stage >= 6 {
            x.original_elf(&memory("7f454c46"), 18, stamp()).unwrap();
        }
        x.cancel();
        assert!(x.exit_observation(stamp(), Some(0), true, true).is_err());
        assert!(x.finish().is_err());
    }
}
#[test]
fn incomplete_reap_streams_or_nonzero_exit_are_not_clean_success() {
    for (status, reaped, streams) in [
        (None, true, true),
        (Some(1), true, true),
        (Some(0), false, true),
        (Some(0), true, false),
    ] {
        let mut x = at_content();
        assert_eq!(
            x.exit_observation(stamp(), status, reaped, streams),
            Err(Refusal::Exit)
        );
        assert!(x.finish().is_err());
    }
}
#[test]
fn content_mismatch_and_late_process_change_have_no_retry() {
    let mut x = at_publication();
    x.published_list(&published("0"), 17, stamp()).unwrap();
    assert!(x.original_elf(&memory("7f454c47"), 18, stamp()).is_err());
    assert!(x.original_elf(&memory("7f454c46"), 18, stamp()).is_err());
    let mut x = at_content();
    assert!(
        x.exit_observation(
            ProcessStamp {
                start_ticks: 1,
                ..stamp()
            },
            Some(0),
            true,
            true
        )
        .is_err()
    );
    assert!(x.finish().is_err());
}
#[test]
fn model_constructor_has_closed_fixture_bounds() {
    assert!(TranscriptModel::new(ProcessStamp { pid: 0, ..stamp() }, 4, digest()).is_err());
    assert!(
        TranscriptModel::new(
            ProcessStamp {
                start_ticks: 0,
                ..stamp()
            },
            4,
            digest()
        )
        .is_err()
    );
    assert!(
        TranscriptModel::new(
            ProcessStamp {
                executable_sha256: [0; 32],
                ..stamp()
            },
            4,
            digest()
        )
        .is_err()
    );
    assert!(TranscriptModel::new(stamp(), 0, digest()).is_err());
    assert!(TranscriptModel::new(stamp(), 65537, digest()).is_err());
}
