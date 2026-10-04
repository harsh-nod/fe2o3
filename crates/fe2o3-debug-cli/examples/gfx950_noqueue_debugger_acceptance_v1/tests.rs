//! CPU-only fake transport controls. No subprocess, pidfd, debugger, GPU FD,
//! native activation, or process-control authority is created by these tests.
use super::{config, custody, parser, protocol, wire};
use parser::Refusal;
use protocol::Peer;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

fn arguments() -> [String; 12] {
    [
        "--allow-vm-mapping",
        "--retain-until-process-exit",
        "--acknowledge-isolated-noqueue-activation",
        "--acknowledge-reviewed-host-debugger-control",
        "/fixture/kernel.hsaco",
        "4",
        &config::hex(&Sha256::digest(b"\x7fELF")),
        "kernel",
        "3",
        "42",
        "7",
        &"a".repeat(64),
    ]
    .map(str::to_owned)
}
fn producer(pid: u32) -> Vec<u8> {
    let a = arguments();
    let value = serde_json::json!({
        "schema":"diagnostic-gfx950-debug-acceptance-noqueue-producer-v1",
        "status":"registered_no_queue_host_rendezvous",
        "observation":{
            "process_id":pid,"process_entry_snapshot_checked":true,"preactivation_snapshot_checked":true,
            "proc_snapshot_proves_general_foreign_exclusion":false,
            "caller_contract":"audited_standalone_program_and_supervisor_process_lifetime_exclusion",
            "target":"gfx950:xnack-","wave_width":64,"node":3,"unique_id":"42","gpu_id":7,
            "device_profile_sha256":a[11],"artifact_sha256":a[6],"artifact_bytes":"4","selected_kernel":"kernel",
            "trap_sha256":"b".repeat(64),"trap_bytes":256,"mapped_backing_bytes":"4096","metadata_retained_bytes":"4096",
            "file_snapshot_rechecked":true,"native_vm_and_mappings_prepared":true,
            "retention":"native_resources_retained_until_process_exit","metadata_version":11,
            "metadata_published":true,"trap_registered":true,"debug_runtime_enabled":true,
            "queue_created":false,"kernel_dispatched":false,"gpu_trap_execution_qualified":false,
            "cleanup_acknowledged":false,"host_process_entry_rendezvous_reached":true,
            "host_pre_activation_rendezvous_reached":true,"host_post_publication_rendezvous_reached":true,
            "debugger_acceptance_observed":false,"physical_register_capture":false,
            "authority":"registration_facts_only_no_queue_stop_or_launch_authority"
        }
    });
    let mut out = serde_json::to_vec(&value).unwrap();
    out.push(b'\n');
    out
}
fn stop(number: usize, symbol: &str) -> String {
    format!(
        "*stopped,reason=\"breakpoint-hit\",disp=\"keep\",bkptno=\"{number}\",frame={{addr=\"0x1234\",func=\"{symbol}\",args=[],arch=\"i386:x86-64\"}},thread-id=\"7\",stopped-threads=\"all\"\n"
    )
}
fn bp(number: usize, symbol: &str) -> String {
    format!(
        "bkpt={{number=\"{number}\",type=\"breakpoint\",disp=\"keep\",enabled=\"y\",addr=\"0x1234\",func=\"{symbol}\",times=\"0\",original-location=\"{symbol}\",thread-groups=[\"i1\"]}}"
    )
}
fn library_record(token: u64, pid: u32, range_fields: &str) -> String {
    let uri = format!("memory://{pid}#offset=0x1000&size=4");
    format!(
        "{token}^done,shared-libraries=[{{id=\"gpu1\",target-name=\"{uri}\",host-name=\"{uri}\",symbols-loaded=\"0\",thread-group=\"i1\",ranges=[{{{range_fields}}}]}}]\n"
    )
}
const RANGE_FIELDS: &str = r#"from="0x1000",to="0x1004""#;
#[derive(Clone, Copy, Default)]
enum Fault {
    #[default]
    None,
    Entry,
    Current,
    ExtraThread,
    DuplicateGroup,
    WrongToken,
    WrongColdStop,
    PresentCold,
    MissingPublished,
    WrongElf,
    ForeignUri,
    OverdeepRange,
    UnknownRangeField,
    DuplicateRangeField,
    NonzeroExit,
    NoNormalExit,
    WrongProducer,
    NoPidfdExit,
    NoReap,
    NoStreams,
    NoJoin,
    FalseReapClaim,
}
struct Fake {
    records: VecDeque<Vec<u8>>,
    commands: Vec<String>,
    fault: Fault,
    bp: usize,
    resumes: usize,
    lists: usize,
    observed: bool,
    entry: bool,
    closed: bool,
    current_checks: usize,
}
impl Fake {
    fn new(fault: Fault) -> Self {
        Self {
            records: VecDeque::from([b"=thread-group-added,id=\"i1\"\n".to_vec()]),
            commands: Vec::new(),
            fault,
            bp: 0,
            resumes: 0,
            lists: 0,
            observed: false,
            entry: false,
            closed: false,
            current_checks: 0,
        }
    }
    fn line(&mut self, line: impl AsRef<[u8]>) {
        self.records.push_back(line.as_ref().to_vec());
    }
}
impl Peer for Fake {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
        self.commands.push(command.to_owned());
        let token = if matches!(self.fault, Fault::WrongToken) {
            token + 1
        } else {
            token
        };
        if let Some(symbol) = command.strip_prefix("-break-insert ") {
            self.bp += 1;
            self.line(format!("{token}^done,{}\n", bp(40 + self.bp, symbol)));
        } else if command == "-exec-run" {
            self.line("=thread-group-started,id=\"i1\",pid=\"77\"\n");
            if matches!(self.fault, Fault::DuplicateGroup) {
                self.line("=thread-group-started,id=\"i2\",pid=\"88\"\n");
            }
            self.line("=thread-created,id=\"7\",group-id=\"i1\"\n");
            if matches!(self.fault, Fault::ExtraThread) {
                self.line("=thread-created,id=\"8\",group-id=\"i1\"\n");
            }
            self.line(format!("{token}^running\n"));
            self.line("*running,thread-id=\"all\"\n");
            self.line(stop(41, parser::ENTRY_HOST_SYMBOL));
        } else if command == "-exec-continue" {
            self.resumes += 1;
            if !self.entry {
                return Err(Refusal::Process);
            }
            self.line(format!("{token}^running\n"));
            self.line("*running,thread-id=\"all\"\n");
            match self.resumes {
                1 => self.line(stop(
                    if matches!(self.fault, Fault::WrongColdStop) {
                        41
                    } else {
                        42
                    },
                    parser::PRE_HOST_SYMBOL,
                )),
                2 => self.line(stop(43, parser::POST_HOST_SYMBOL)),
                3 => {
                    self.line(producer(if matches!(self.fault, Fault::WrongProducer) {
                        78
                    } else {
                        77
                    }));
                    self.line("=thread-exited,id=\"7\",group-id=\"i1\"\n");
                    let exit = if matches!(self.fault, Fault::NonzeroExit) {
                        "1"
                    } else {
                        "0"
                    };
                    self.line(format!(
                        "=thread-group-exited,id=\"i1\",exit-code=\"{exit}\"\n"
                    ));
                    if !matches!(self.fault, Fault::NoNormalExit) {
                        self.line("*stopped,reason=\"exited-normally\"\n");
                    }
                }
                _ => return Err(Refusal::State),
            }
        } else if command == "-file-list-shared-libraries" {
            self.lists += 1;
            let present = if self.lists == 1 {
                matches!(self.fault, Fault::PresentCold)
            } else {
                !matches!(self.fault, Fault::MissingPublished)
            };
            if present {
                let pid = if matches!(self.fault, Fault::ForeignUri) {
                    88
                } else {
                    77
                };
                let range_fields = match self.fault {
                    Fault::OverdeepRange => r#"from=["0x1000"],to="0x1004""#,
                    Fault::UnknownRangeField => r#"from="0x1000",to="0x1004",extra="x""#,
                    Fault::DuplicateRangeField => r#"from="0x1000",from="0x1000",to="0x1004""#,
                    _ => RANGE_FIELDS,
                };
                self.line(library_record(token, pid, range_fields));
            } else {
                self.line(format!("{token}^done,shared-libraries=[]\n"));
            }
        } else if command == "-data-read-memory-bytes 0x1000 4" {
            let content = if matches!(self.fault, Fault::WrongElf) {
                "00454c46"
            } else {
                "7f454c46"
            };
            self.line(format!("{token}^done,memory=[{{begin=\"0x1000\",offset=\"0x0\",end=\"0x1004\",contents=\"{content}\"}}]\n"));
        } else if command == "-gdb-exit" {
            self.line(format!("{token}^exit\n"));
        } else {
            self.line(format!("{token}^done\n"));
        }
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        Ok(self.records.pop_front())
    }
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
        if pid != 77 || self.observed {
            return Err(Refusal::Process);
        }
        self.observed = true;
        Ok(())
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        if !self.observed || matches!(self.fault, Fault::Entry) {
            return Err(Refusal::Process);
        }
        self.entry = true;
        Ok(())
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.current_checks += 1;
        if !self.entry || matches!(self.fault, Fault::Current) {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<wire::Cleanup, Refusal> {
        self.closed = true;
        Ok(wire::Cleanup {
            owned_inferior_pidfd_exit_observed: !matches!(self.fault, Fault::NoPidfdExit),
            debugger_direct_child_reaped: !matches!(self.fault, Fault::NoReap),
            streams_complete: !matches!(self.fault, Fault::NoStreams),
            reader_threads_joined: !matches!(self.fault, Fault::NoJoin),
            inferior_reaped_by_controller: matches!(self.fault, Fault::FalseReapClaim),
            unadmitted_inferior_cleanup_not_proven: false,
        })
    }
}
fn run(fake: &mut Fake) -> Result<protocol::Observation, Refusal> {
    protocol::run(fake, "/fixture/observer", &arguments())
}
#[test]
fn dynamic_ids_and_three_real_protocol_stops_have_only_inert_match_result() {
    let mut fake = Fake::new(Fault::None);
    let observed = run(&mut fake).unwrap();
    assert_eq!(observed.process_id, 77);
    assert!(observed.same_host_stop_original_elf_content_matched);
    assert!(!observed.debugger_acceptance_observed);
    assert!(!observed.runtime_loaded_success_observed);
    assert!(!observed.physical_register_capture);
    assert!(!observed.process_control_authority);
    assert!(!observed.source_or_launch_authority);
    assert_eq!(fake.resumes, 3);
    assert!(fake.current_checks >= 8);
    assert!(fake.closed);
}
#[test]
fn first_entry_identity_failure_never_permits_kfd_setup_continuation() {
    let mut fake = Fake::new(Fault::Entry);
    assert_eq!(run(&mut fake).err(), Some(Refusal::Process));
    assert_eq!(fake.resumes, 0);
}
#[test]
fn changed_entry_identity_never_permits_kfd_setup_continuation() {
    let mut fake = Fake::new(Fault::Current);
    assert_eq!(run(&mut fake).err(), Some(Refusal::Changed));
    assert_eq!(fake.resumes, 0);
}
#[test]
fn unexpected_second_thread_or_inferior_refused_before_setup() {
    for fault in [Fault::ExtraThread, Fault::DuplicateGroup] {
        let mut fake = Fake::new(fault);
        assert!(run(&mut fake).is_err());
        assert_eq!(fake.resumes, 0);
    }
}
#[test]
fn unmatched_token_never_launches_inferior() {
    let mut fake = Fake::new(Fault::WrongToken);
    assert_eq!(run(&mut fake).err(), Some(Refusal::Token));
    assert!(!fake.observed);
}
#[test]
fn wrong_cold_breakpoint_refuses_activation_without_retry() {
    let mut fake = Fake::new(Fault::WrongColdStop);
    assert!(run(&mut fake).is_err());
    assert_eq!(fake.resumes, 1);
}
#[test]
fn preexisting_gpu_object_refuses_activation_without_retry() {
    let mut fake = Fake::new(Fault::PresentCold);
    assert!(run(&mut fake).is_err());
    assert_eq!(fake.resumes, 1);
}
#[test]
fn absent_published_object_or_foreign_memory_uri_never_reads_memory() {
    for (fault, expected) in [
        (Fault::MissingPublished, Refusal::Artifact),
        (Fault::ForeignUri, Refusal::Process),
    ] {
        let mut fake = Fake::new(fault);
        assert_eq!(run(&mut fake).err(), Some(expected));
        assert!(
            !fake
                .commands
                .iter()
                .any(|s| s.starts_with("-data-read-memory"))
        );
    }
}
#[test]
fn required_library_range_shape_uses_exact_shared_syntax_depth() {
    use crate::rocgdb_mi_parser_v3::{
        MiParseErrorV3, MiParserLimitsV3, MiRecordV3, parse_mi_record_v3,
    };

    let line = library_record(19, 77, RANGE_FIELDS);
    // The existing closed schema reaches depth 12, not eight: result(1),
    // value/list(2/3), value/tuple(4/5), ranges result/value/list(6/7/8),
    // range value/tuple(9/10), and from/to result/value(11/12).
    assert_eq!(
        parse_mi_record_v3(
            line.as_bytes(),
            MiParserLimitsV3 {
                max_line_bytes: 192 * 1024,
                max_string_bytes: 128 * 1024,
                max_fields: 512,
                max_depth: 11,
            },
        ),
        Err(MiParseErrorV3::DepthLimit)
    );
    assert!(matches!(
        parser::record(line.as_bytes()),
        Ok(MiRecordV3::Result {
            token: Some(19),
            ..
        })
    ));
    assert_eq!(
        parser::parse_libraries(line.as_bytes(), 19, 77, 4),
        Ok(Some(parser::MemoryObject {
            pid: 77,
            address: 0x1000,
            bytes: 4,
        }))
    );
}

#[test]
fn malformed_range_fields_never_reach_memory_read_or_final_continuation() {
    for (fault, expected) in [
        (Fault::OverdeepRange, Refusal::Syntax),
        (Fault::UnknownRangeField, Refusal::Shape),
        (Fault::DuplicateRangeField, Refusal::Syntax),
    ] {
        let mut fake = Fake::new(fault);
        assert_eq!(run(&mut fake).err(), Some(expected));
        assert_eq!(fake.resumes, 2);
        assert!(!fake.closed);
        assert!(
            !fake
                .commands
                .iter()
                .any(|s| s.starts_with("-data-read-memory"))
        );
    }
}

#[test]
fn changed_original_elf_cannot_be_accepted_from_symbols_loaded_or_exit() {
    let mut fake = Fake::new(Fault::WrongElf);
    assert_eq!(run(&mut fake).err(), Some(Refusal::Artifact));
    assert_eq!(fake.resumes, 2);
}
#[test]
fn nonzero_or_missing_exit_observation_refused() {
    for fault in [
        Fault::NonzeroExit,
        Fault::NoNormalExit,
        Fault::WrongProducer,
    ] {
        assert!(run(&mut Fake::new(fault)).is_err());
    }
}
#[test]
fn each_exit_reap_stream_join_observation_is_required_separately() {
    for fault in [
        Fault::NoPidfdExit,
        Fault::NoReap,
        Fault::NoStreams,
        Fault::NoJoin,
        Fault::FalseReapClaim,
    ] {
        assert_eq!(run(&mut Fake::new(fault)).err(), Some(Refusal::Incomplete));
    }
}
#[test]
fn no_attach_eval_register_or_caller_memory_command_exists_in_trace() {
    let mut fake = Fake::new(Fault::None);
    run(&mut fake).unwrap();
    assert!(!fake.commands.iter().any(|s| s.contains("attach")
        || s.contains("evaluate")
        || s.contains("register")
        || s.contains("-exec-jump")));
    assert_eq!(
        fake.commands
            .iter()
            .filter(|s| s.starts_with("-data-read-memory"))
            .count(),
        1
    );
}
fn stat(pid: u32, parent: u32, start: u64) -> Vec<u8> {
    let mut fields = vec!["0".to_owned(); 20];
    fields[0] = "t".into();
    fields[1] = parent.to_string();
    fields[19] = start.to_string();
    format!("{pid} (name ) with spaces) {}\n", fields.join(" ")).into_bytes()
}
#[test]
fn proc_stat_handles_parentheses_and_retains_nonzero_starttime() {
    let a = custody::parse_stat(&stat(77, 99, 123), 77, 99).unwrap();
    let b = custody::parse_stat(&stat(77, 99, 124), 77, 99).unwrap();
    assert_ne!(a, b);
    assert!(custody::parse_stat(&stat(78, 99, 123), 77, 99).is_err());
    assert!(custody::parse_stat(&stat(77, 98, 123), 77, 99).is_err());
    assert!(custody::parse_stat(&stat(77, 99, 0), 77, 99).is_err());
}
#[test]
fn exact_environment_rejects_injection_and_duplicates() {
    assert_eq!(custody::clean_environment(b"LC_ALL=C\0LANG=C\0"), Ok(()));
    for bad in [
        b"LANG=C\0LC_ALL=C\0LD_PRELOAD=x\0".as_slice(),
        b"LANG=C\0LANG=C\0LC_ALL=C\0",
        b"LANG=C\0",
        b"LANG=C\0LC_ALL=C",
    ] {
        assert!(custody::clean_environment(bad).is_err());
    }
}
#[test]
fn partial_and_oversize_lines_are_not_truncated_into_valid_records() {
    assert!(wire::bounded_line(&mut std::io::Cursor::new(b"1^done")).is_err());
    let long = vec![b'x'; 192 * 1024 + 1];
    assert!(wire::bounded_line(&mut std::io::Cursor::new(long)).is_err());
    assert_eq!(
        wire::bounded_line(&mut std::io::Cursor::new(b"1^done\n")).unwrap(),
        Some(b"1^done\n".to_vec())
    );
}
#[test]
fn argument_grammar_refuses_missing_acks_and_oversized_artifact() {
    let mut a = arguments();
    config::validate_arguments(&a).unwrap();
    a[3] = "--attach=77".into();
    assert!(config::validate_arguments(&a).is_err());
    let mut a = arguments();
    a[5] = "65537".into();
    assert!(config::validate_arguments(&a).is_err());
    let mut a = arguments();
    a[7] = "x\n-exec-run".into();
    assert!(config::validate_arguments(&a).is_err());
}
#[test]
fn producer_cannot_mint_capture_or_cleanup_claims() {
    for key in [
        "debugger_acceptance_observed",
        "physical_register_capture",
        "cleanup_acknowledged",
        "queue_created",
        "kernel_dispatched",
    ] {
        let mut p: serde_json::Value = serde_json::from_slice(&producer(77)).unwrap();
        p["observation"][key] = true.into();
        let mut raw = serde_json::to_vec(&p).unwrap();
        raw.push(b'\n');
        assert!(protocol::check_producer(&raw, 77, &arguments()).is_err());
    }
}
#[test]
fn producer_duplicate_keys_and_extra_records_are_refused() {
    let valid = String::from_utf8(producer(77)).unwrap();
    let duplicate = valid.replacen(
        "\"process_id\":77",
        "\"process_id\":88,\"process_id\":77",
        1,
    );
    assert!(protocol::check_producer(duplicate.as_bytes(), 77, &arguments()).is_err());
    let twice = format!("{valid}{valid}");
    assert!(protocol::check_producer(twice.as_bytes(), 77, &arguments()).is_err());
}

#[test]
fn exact_fixed_startup_notifications_are_inert_and_optional() {
    let mut fake = Fake::new(Fault::None);
    for parameter in [
        "auto-load gdb-scripts",
        "auto-load libthread-db",
        "auto-load local-gdbinit",
        "auto-load python-scripts",
        "startup-with-shell",
    ] {
        fake.line(format!(
            "=cmd-param-changed,param=\"{parameter}\",value=\"off\"\n"
        ));
    }
    let observed = run(&mut fake).unwrap();
    assert!(!observed.debugger_acceptance_observed);
    assert!(!observed.runtime_loaded_success_observed);
    assert_eq!(fake.resumes, 3);
    // The existing all-absent fixture remains admitted, too.
}

#[test]
fn startup_parameter_values_unknown_keys_and_extra_fields_are_closed() {
    for line in [
        "=cmd-param-changed,param=\"auto-load python-scripts\",value=\"on\"\n",
        "=cmd-param-changed,param=\"auto-load python-scripts\",value=\"0\"\n",
        "=cmd-param-changed,param=\"auto-load\",value=\"off\"\n",
        "=cmd-param-changed,param=\"debuginfod enabled\",value=\"off\"\n",
        "=cmd-param-changed,param=\"startup-with-shell\",value=\"off\",extra=\"x\"\n",
        "=cmd-param-changed,param=\"startup-with-shell\"\n",
        "=cmd-param-changed,value=\"off\"\n",
        "=cmd-param-changed,param=\"startup-with-shell\",value=[\"off\"]\n",
        "=cmd-param-changed,param=\"startup-with-shell\",param=\"startup-with-shell\",value=\"off\"\n",
    ] {
        let mut fake = Fake::new(Fault::None);
        fake.line(line);
        assert!(run(&mut fake).is_err(), "{line:?}");
        assert!(!fake.observed);
        assert_eq!(fake.resumes, 0);
    }
}

#[test]
fn duplicate_startup_parameter_notification_refuses_before_inferior_launch() {
    let mut fake = Fake::new(Fault::None);
    for _ in 0..2 {
        fake.line("=cmd-param-changed,param=\"startup-with-shell\",value=\"off\"\n");
    }
    assert_eq!(run(&mut fake).err(), Some(Refusal::Duplicate));
    assert!(!fake.observed);
}

#[test]
fn fixed_startup_setting_after_setup_refuses_before_entry_continuation() {
    struct Late(Fake);
    impl Peer for Late {
        fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
            self.0.send(token, command)?;
            if command == "-exec-run" {
                self.0.records.push_front(
                    b"=cmd-param-changed,param=\"startup-with-shell\",value=\"off\"\n".to_vec(),
                );
            }
            Ok(())
        }
        fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
            self.0.next()
        }
        fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
            self.0.observe_child(pid)
        }
        fn entry(&mut self) -> Result<(), Refusal> {
            self.0.entry()
        }
        fn current(&mut self) -> Result<(), Refusal> {
            self.0.current()
        }
        fn finish(&mut self) -> Result<wire::Cleanup, Refusal> {
            self.0.finish()
        }
    }
    let mut fake = Late(Fake::new(Fault::None));
    assert_eq!(
        protocol::run(&mut fake, "/fixture/observer", &arguments()).err(),
        Some(Refusal::State),
    );
    assert_eq!(fake.0.resumes, 0);
}

#[path = "native_runtime_protocol_v1_tests.rs"]
mod native_runtime_protocol_v1_tests;

fn breakpoint_result(raw: &str) -> crate::rocgdb_mi_parser_v3::MiResultsV3 {
    let line = format!("1^done,{raw}\n");
    let crate::rocgdb_mi_parser_v3::MiRecordV3::Result { results, .. } =
        parser::record(line.as_bytes()).unwrap()
    else {
        panic!("fixture shape");
    };
    results
}
#[test]
fn exact_full_or_minimal_symbol_breakpoints_preserve_dynamic_number() {
    for symbol in [
        parser::ENTRY_HOST_SYMBOL,
        parser::PRE_HOST_SYMBOL,
        parser::POST_HOST_SYMBOL,
    ] {
        let full = bp(41, symbol);
        let minimal = full.replace(&format!("func=\"{symbol}\""), &format!("at=\"<{symbol}>\""));
        for raw in [full, minimal] {
            assert_eq!(
                protocol::breakpoint(&breakpoint_result(&raw), symbol),
                Ok(b"41".to_vec())
            );
        }
    }
}
#[test]
fn minimal_symbol_breakpoints_refuse_offsets_alternate_names_and_ambiguity() {
    let symbol = parser::ENTRY_HOST_SYMBOL;
    let full = bp(41, symbol);
    for replacement in [
        format!("at=\"<{symbol}+0>\""),
        format!("at=\"<{symbol}+4>\""),
        format!("at=\"<{symbol}-4>\""),
        format!("at=\"<*{symbol}*>\""),
        format!("at=\"<{symbol} at source.rs:1>\""),
        format!("at=\"{symbol}\""),
        "at=\"<foreign>\"".into(),
        "at=\"\"".into(),
        format!("at=[\"<{symbol}>\"]"),
        format!("func=\"{symbol}\",at=\"<{symbol}>\""),
        format!("at=\"<{symbol}>\",file=\"source.rs\""),
    ] {
        let raw = full.replace(&format!("func=\"{symbol}\""), &replacement);
        assert!(
            protocol::breakpoint(&breakpoint_result(&raw), symbol).is_err(),
            "{replacement}"
        );
    }
    let missing = full.replace(&format!(",func=\"{symbol}\""), "");
    assert!(protocol::breakpoint(&breakpoint_result(&missing), symbol).is_err());
}
#[test]
fn minimal_symbol_form_does_not_relax_address_origin_or_group_checks() {
    let symbol = parser::ENTRY_HOST_SYMBOL;
    let original =
        bp(41, symbol).replace(&format!("func=\"{symbol}\""), &format!("at=\"<{symbol}>\""));
    for (from, to) in [
        ("addr=\"0x1234\"", "addr=\"0x0\""),
        ("addr=\"0x1234\"", "addr=\"<PENDING>\""),
        ("addr=\"0x1234\"", "addr=\"<MULTIPLE>\""),
        ("addr=\"0x1234\"", "addr=\"0x10000000000000000\""),
        ("thread-groups=[\"i1\"]", "thread-groups=[\"i2\"]"),
        ("thread-groups=[\"i1\"]", "thread-groups=[\"i1\",\"i2\"]"),
        ("times=\"0\"", "times=\"2\""),
        ("number=\"41\"", "number=\"0\""),
        (
            "original-location=\"fe2o3_gfx950_noqueue_process_entry_v1\"",
            "original-location=\"foreign\"",
        ),
    ] {
        assert!(
            protocol::breakpoint(&breakpoint_result(&original.replacen(from, to, 1)), symbol)
                .is_err()
        );
    }
}
#[test]
fn duplicate_minimal_symbol_fields_are_rejected_by_shared_mi_parser() {
    let symbol = parser::ENTRY_HOST_SYMBOL;
    let raw = bp(41, symbol).replace(
        &format!("func=\"{symbol}\""),
        &format!("at=\"<{symbol}>\",at=\"<{symbol}>\""),
    );
    assert!(parser::record(format!("1^done,{raw}\n").as_bytes()).is_err());
}
#[test]
fn full_closed_protocol_accepts_minimal_symbol_breakpoint_creation() {
    struct Minimal(Fake);
    impl Peer for Minimal {
        fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
            self.0.send(token, command)?;
            if let Some(symbol) = command.strip_prefix("-break-insert ") {
                let last = self.0.records.back_mut().unwrap();
                *last = String::from_utf8(last.clone())
                    .unwrap()
                    .replace(&format!("func=\"{symbol}\""), &format!("at=\"<{symbol}>\""))
                    .into_bytes();
            }
            Ok(())
        }
        fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
            self.0.next()
        }
        fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
            self.0.observe_child(pid)
        }
        fn entry(&mut self) -> Result<(), Refusal> {
            self.0.entry()
        }
        fn current(&mut self) -> Result<(), Refusal> {
            self.0.current()
        }
        fn finish(&mut self) -> Result<wire::Cleanup, Refusal> {
            self.0.finish()
        }
    }
    let mut fake = Minimal(Fake::new(Fault::None));
    let observed = protocol::run(&mut fake, "/fixture/observer", &arguments()).unwrap();
    assert!(!observed.debugger_acceptance_observed);
    assert!(!observed.runtime_loaded_success_observed);
    assert_eq!(fake.0.resumes, 3);
}
