//! Opt-in consuming-path tests, not Cargo authorship or compiler execution.
//! Requires FE2O3_RUN_PROVISIONED_ROOT_REQUEST=isolated-disposable-root, private
//! /tmp and /run/fe2o3, actual fixed V3 provisioning records/keys/five images, and
//! independently installed immutable matching compiler approval/runtime. Never
//! provisions or rewrites those inputs. No fixed test keys, image substitutions,
//! compiler exec fixture, or synthetic Prepared producer is used by this matrix.
//! Missing/mismatched prerequisites fail, never count as negative-control passes.
//! Positive pre-exec cases require a real distinct-UID sender, writable isolated
//! cgroup parent and installed proof runtime. They execute the approved helper,
//! but NEVER release the compiler gate. Not Cargo-authorship or proof qualification.
use super::*;
use crate::compiler_invocation_backing::CompilerInvocationBacking as Backing;
use crate::compiler_output_directory::CompilerOutputDirectory as Output;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV2 as Approval, RustcInvocationCapabilityV1 as Capture,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V4 as N,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1 as SOCKET,
    CompilerExecutionRootIntakeRecordV4 as Record, CompilerExecutionRootIntakeRoleV4 as Role,
};
use fe2o3_protected_service_spawn::launch_io::{self, MessageSender};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, InvocationDigestV3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV2,
};
use rustix::{fs, net};
use std::{
    fs as disk,
    fs::File,
    os::fd::{AsFd, OwnedFd, RawFd},
    os::unix::fs::MetadataExt,
    process::Command,
    time::Instant,
};

// Separate from the unchanged synthetic native-root issuer fixtures.
#[path = "native_root_request_provisioned_fixture_tests.rs"]
mod fixtures;
#[path = "native_root_request_preexec_process_tests.rs"]
mod preexec;

const WORK: usize = 1 << 60;
const STORAGE: usize = 64 * 1024 * 1024 * 1024;
const HARNESS: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const CASE_ENV: &str = "FE2O3_NATIVE_ROOT_REQUEST_CASE";

impl Native<'_> {
    // Only the explicitly selected subprocess consumes budget, AFTER the real
    // complete request is installed. This cannot supply owners or approval.
    pub(in crate::native_entrypoint) fn drain_received_budget_for_test(
        &mut self,
        b: &mut Budget<'_>,
    ) {
        let case = std::env::var(CASE_ENV).unwrap_or_default();
        if !matches!(case.as_str(), "received-work" | "received-storage") {
            return;
        }
        assert!(self.prepared.is_none() && self.intake.is_none());
        let request = self.request.as_ref().expect("installed original request");
        let (receiver, backing) = request.received_for_test();
        let (capture, files) = receiver.received_for_test();
        assert!(capture.is_some() && backing.is_none());
        assert!(files.iter().all(Option::is_some));
        assert!(!request.failed_for_test());
        assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        if case == "received-work" {
            // Fund request entry itself; denial must occur inside prepare.
            b.charge_work(WORK - b.work() - root::LOCAL_WORK - Backing::LOCAL_WORK)
                .unwrap();
        } else {
            b.reserve_storage(STORAGE - b.storage()).unwrap();
        }
        eprintln!("ROOT_REQUEST_RECEIVED case={case}");
    }
}

fn test_name(name: &str) -> String {
    format!("native_entrypoint::tests::root_request::{name}")
}

fn postclone_case(case: &str) -> Option<(&'static str, &'static str)> {
    let case = case.strip_prefix("postclone-")?;
    for phase in [
        "helper-ready",
        "compiler-profile",
        "compiler-channel",
        "compiler-trace",
    ] {
        if let Some(action) = case.strip_prefix(phase) {
            return match action {
                "-work" => Some((phase, "work")),
                "-storage" => Some((phase, "storage")),
                "-unwind" => Some((phase, "unwind")),
                _ => None,
            };
        }
    }
    None
}

#[test]
fn postclone_channel_case_requires_exact_phase_and_action() {
    for action in ["work", "storage", "unwind"] {
        assert_eq!(
            postclone_case(&format!("postclone-compiler-channel-{action}")),
            Some(("compiler-channel", action))
        );
    }
    for case in [
        "compiler-channel-work",
        "postclone-compiler-channel",
        "postclone-compiler-channelwork",
        "postclone-compiler-channel-work-extra",
        "postclone-compiler-channel-ready",
        "postclone-compiler-channel-trace-work",
    ] {
        assert_eq!(postclone_case(case), None, "{case}");
    }
}

fn require_output(output: std::process::Output, marker: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains(marker));
}

#[test]
#[ignore = "isolated root and installed immutable matching compiler runtime; run serially"]
fn complete_root_request_consuming_matrix() {
    fixtures::require_environment();
    for case in [
        "mask0",
        "mask1",
        "mask2",
        "mask3",
        "mask4",
        "mask5",
        "mask6",
        "mask7",
        "received-work",
        "received-storage",
        "consuming-closure-refusal",
        "short-work",
        "exhausted-storage",
        "foreign-account",
        "moved-account",
        "trailing",
        "unwind",
        "helper-unwind",
        "compiler-unwind",
        "compiler-work",
        "compiler-storage",
        "two-turn-quota",
        "peer-uid-alias",
        "peer-gid-alias",
        "postclone-helper-ready-work",
        "postclone-helper-ready-storage",
        "postclone-helper-ready-unwind",
        "postclone-compiler-profile-work",
        "postclone-compiler-profile-storage",
        "postclone-compiler-profile-unwind",
        "postclone-compiler-channel-work",
        "postclone-compiler-channel-storage",
        "postclone-compiler-channel-unwind",
        "postclone-compiler-trace-work",
        "postclone-compiler-trace-storage",
        "postclone-compiler-trace-unwind",
    ] {
        let output = preexec::run_case(case);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("ROOT_REQUEST_PROVISIONED"),
            "{case}: must admit actual provisioned owners before any credited control: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Credit only the phase actually reached, never an earlier refusal.
        let marker = match case {
            "received-work" | "received-storage" => "ROOT_REQUEST_RECEIVED_REFUSED",
            "consuming-closure-refusal" => "ROOT_REQUEST_CONSUMING_REFUSED",
            _ => "ROOT_REQUEST_CONSUMED",
        };
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(marker),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if case.starts_with("mask")
            || case == "trailing"
            || case.starts_with("compiler-")
            || case == "two-turn-quota"
        {
            for marker in [
                "ROOT_REQUEST_HELPER_EXEC",
                "ROOT_REQUEST_COMPILER_CHANNEL_GATE_CLOSED",
            ] {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains(marker),
                    "{case} must reach {marker}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        if case == "helper-unwind" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("ROOT_REQUEST_HELPER_EXEC"));
        }
        if let Some((phase, action)) = postclone_case(case) {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&format!(
                    "ROOT_REQUEST_POSTCLONE_REACHED phase={phase} kind={action}"
                )),
                "{case}: actual postclone phase must be reached: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if phase != "helper-ready" {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("ROOT_REQUEST_HELPER_EXEC")
                );
            }
            if phase == "compiler-channel" {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr.contains("ROOT_REQUEST_COMPILER_CHANNEL_PRETRACE_GATE_CLOSED"),
                    "{case}: actual untraced channel must be reached behind the closed gate: {stderr}"
                );
                assert!(
                    !stderr.contains("ROOT_REQUEST_COMPILER_CHANNEL_GATE_CLOSED"),
                    "{case}: pre-trace denial must not complete the compiler attempt"
                );
            }
            if action != "unwind" {
                assert!(
                    String::from_utf8_lossy(&output.stderr)
                        .contains("ROOT_REQUEST_POSTCLONE_REFUSED")
                );
            }
        }
        if case.starts_with("peer-") {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("ROOT_REQUEST_PEER_ALIAS_REFUSED")
            );
        }
        if case == "two-turn-quota" {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("ROOT_REQUEST_TWO_TURN_QUOTA")
            );
        }
        if case.ends_with("unwind") {
            assert_eq!(
                output.status.code(),
                Some(125),
                "armed creator must fail-stop after unwind"
            );
        } else {
            require_output(output, "ROOT_REQUEST_DRAINED");
        }
    }
}

#[test]
#[ignore = "subprocess role; use complete_root_request_consuming_matrix"]
#[allow(unsafe_code)]
fn complete_root_request_case() {
    fixtures::require_environment();
    let case = std::env::var(CASE_ENV).expect("matrix case required");
    let mask = case
        .strip_prefix("mask")
        .map_or(7, |s| s.parse::<u8>().unwrap());
    assert!(mask < 8);
    let mut work = Work::new(WORK);
    let mut displaced_work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let mut displaced = Budget::new(&mut displaced_work, STORAGE);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    b.reserve_storage(HARNESS + FRAME).unwrap();
    b.charge_work(CreatorScope::CONTROL_WORK).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let pool = Cleanup::admit(Account::new(Work::new(WORK), STORAGE)).unwrap();
    // SAFETY: this isolated subprocess owns its real pool and prepays scope
    // control above; an assertion/unwind cannot return while it is still armed.
    let mut creator = unsafe { CreatorScope::enter(pool) };
    // SAFETY: preparation borrows only this creator's original live cleanup pool.
    let (f, prepared) = fixtures::prepare(unsafe { creator.cleanup_for_launch() }, &mut b);
    b.reserve_storage(Receiver::STORAGE + RootCompilerRequest::ENVELOPE)
        .unwrap();
    let mut native = Native {
        prepared: Some(prepared),
        intake: Some(Receiver::empty()),
        request: None,
        activation: None,
        signals: None,
        admission: Admission::Inherited,
        creator,
    };
    native
        .intake
        .as_ref()
        .unwrap()
        .activate(native.prepared.as_mut().unwrap(), &mut b)
        .unwrap();

    // The fixture sender has its own account. Its inert descriptor/profile data
    // never supply the root's approval; RootCompilerRequest opens both origins.
    let mut sender_work = Work::new(WORK);
    let mut sender_budget = Budget::new(&mut sender_work, STORAGE);
    sender_budget.reserve_storage(HARNESS).unwrap();
    let (approval, charge) = Approval::from_production_policy(&mut sender_budget)
        .expect("independently installed immutable matching compiler approval required");
    sender_budget
        .reserve_storage(charge.retained_storage())
        .unwrap();
    let cwd = File::open(f.dir.path()).unwrap();
    let output_path = f.dir.path().join("output");
    disk::create_dir(&output_path).unwrap();
    let output = File::open(&output_path).unwrap();
    let output_identity = identity(&output);
    let original = approval.policy().compiler_closure();
    let closure = if case == "consuming-closure-refusal" {
        // Keep executable/backend pins identical: mismatch only the full closure.
        let mut tree = original.rustc_runtime_tree_sha256();
        tree[0] ^= 1;
        if tree == [0; 32] {
            tree[1] = 1;
        }
        CompilerClosureV2::new(
            original.cargo_executable_sha256(),
            original.cargo_binding_trampoline_sha256(),
            original.cargo_fe2o3_binding_wrapper_sha256(),
            original.rustc_executable_sha256(),
            tree,
            original.codegen_backend_sha256(),
        )
        .unwrap()
    } else {
        original
    };
    let capture = Capture::create(
        RustcInvocationDescriptorV3::new(
            RustcInvocationDescriptorV2::new(
                closure.rustc_executable_sha256(),
                closure.codegen_backend_sha256(),
                RustcUnitV2::new(
                    f.dir.path().to_str().unwrap(),
                    vec![
                        "/unexecuted-fixture/rustc".into(),
                        "-Zcodegen-backend=/proc/./self/fd/198".into(),
                    ],
                )
                .unwrap(),
                CompileEnvironmentV2::from_child_environment([
                    ("FE2O3_TARGET".into(), "gfx942:xnack-".into()),
                    ("FE2O3_HSACO_DIR".into(), Output::CHILD_PATH.into()),
                ])
                .unwrap(),
            )
            .unwrap(),
            closure,
        )
        .unwrap(),
    )
    .unwrap();
    sender_budget
        .reserve_storage(capture.native_retained_storage().unwrap())
        .unwrap();
    let (invocation, charge) = capture
        .try_clone_for_transfer_native(&mut sender_budget)
        .unwrap();
    sender_budget
        .reserve_storage(charge.additional_storage())
        .unwrap();
    let streams = [
        tempfile::tempfile().unwrap(),
        tempfile::tempfile().unwrap(),
        tempfile::tempfile().unwrap(),
    ];
    for (index, file) in streams.iter().enumerate() {
        fs::seek(file, fs::SeekFrom::Start(17 + index as u64)).unwrap();
    }
    let (mut uid, mut gid) = preexec::identity_from_environment();
    assert_ne!(
        uid,
        approval.policy().proof_helper_uid(),
        "default client UID must be independently provisioned"
    );
    assert_ne!(
        gid,
        approval.policy().proof_helper_gid(),
        "default client GID must be independently provisioned"
    );
    if case == "peer-uid-alias" {
        uid = approval.policy().proof_helper_uid();
    }
    if case == "peer-gid-alias" {
        gid = approval.policy().proof_helper_gid();
    }
    let (proxy, client) = preexec::Sender::connect((uid, gid));
    let sender = MessageSender::new(rustix::process::getpid().as_raw_pid(), 0, 0);
    turn(&mut native, &mut b);
    let (hello, _) = Record::hello(
        approval.profile().profile().policy(),
        InvocationDigestV3::calculate(capture.descriptor())
            .unwrap()
            .into_bytes(),
        [19; 32],
        mask,
        invocation.metadata().unwrap().len(),
        output_identity,
        &mut sender_budget,
    )
    .unwrap();
    proxy.send(hello.canonical_bytes(), None);
    turn(&mut native, &mut b);
    turn(&mut native, &mut b);
    let bytes = launch_io::receive_authenticated_packet::<N>(client.as_fd(), sender)
        .unwrap()
        .unwrap();
    let (challenge, _) = Record::decode(&bytes, &mut sender_budget).unwrap();
    assert!(
        challenge
            .matches_predecessor(&hello, &mut sender_budget)
            .unwrap()
    );
    let count = challenge.roles().count();
    let mut last = None;
    let mut before = None;
    let preparation_denial = matches!(
        case.as_str(),
        "received-work" | "received-storage" | "consuming-closure-refusal"
    );
    for (index, role) in challenge.roles().enumerate() {
        let file = match role {
            Role::Invocation => &invocation,
            Role::WorkingDirectory => &cwd,
            Role::OutputDirectory => &output,
            Role::Stdin => &streams[0],
            Role::Stdout => &streams[1],
            Role::Stderr => &streams[2],
        };
        let (record, _) = Record::input(&challenge, role, &mut sender_budget).unwrap();
        proxy.send(record.canonical_bytes(), Some(file.as_fd()));
        if index + 1 == count {
            let (capture, files) = native.intake.as_ref().unwrap().received_for_test();
            let capture =
                capture.expect("original receiver decoded the invocation before completion");
            before = Some((
                b.storage(),
                b.work(),
                b.peak_storage(),
                capture.native_retained_storage().unwrap(),
                capture.descriptor().rustc().argv().next().unwrap().as_ptr(),
                files,
            ));
        }
        native.continuity(&mut b).unwrap();
        if preparation_denial && index + 1 == count {
            let outcome = native.intake(&mut b);
            match case.as_str() {
                "received-work" => {
                    assert!(matches!(outcome, Err(Failure::Resource(Resource::Work(_)))));
                    assert_eq!(
                        b.work(),
                        WORK,
                        "request entry succeeded before preparation denial"
                    );
                    assert!(b.failed_work().is_some() && b.failed_storage().is_none());
                }
                "received-storage" => {
                    assert!(matches!(
                        outcome,
                        Err(Failure::Resource(Resource::Storage(_)))
                    ));
                    assert!(b.failed_storage().is_some() && b.failed_work().is_none());
                }
                "consuming-closure-refusal" => {
                    assert!(
                        matches!(
                            outcome,
                            Err(Failure::Invalid {
                                role: "compiler approval",
                                reason: "compiler differs from root-approved closure",
                            })
                        ),
                        "must reach genuine Backing::prepare full-closure check: {outcome:?}"
                    );
                    assert!(b.failed_work().is_none() && b.failed_storage().is_none());
                }
                _ => unreachable!(),
            }
            assert_no_ack(&client, sender);
        } else {
            assert!(!native.intake(&mut b).expect("actual consuming preparation"));
        }
        last = Some(record);
    }
    assert!(native.prepared.is_none() && native.intake.is_none());
    let request = native
        .request
        .as_ref()
        .expect("actual Native::intake must install whole request");
    let (receiver, backing) = request.received_for_test();
    let (capture, files) = receiver.received_for_test();
    let (storage, spent, peak, capture_storage, argv0, original_files) = before.unwrap();
    for (old, current) in original_files.iter().zip(files) {
        if old.is_some() {
            assert_eq!(*old, current, "original received FD must not be replaced");
        }
    }
    for (index, role) in challenge.roles().enumerate() {
        let expected = match role {
            Role::Invocation => identity(&invocation),
            Role::WorkingDirectory => identity(&cwd),
            Role::OutputDirectory => output_identity,
            Role::Stdin => identity(&streams[0]),
            Role::Stdout => identity(&streams[1]),
            Role::Stderr => identity(&streams[2]),
        };
        assert_eq!(fd_identity(files[index].unwrap()), expected);
    }
    assert!(files[count..].iter().all(Option::is_none));
    for (index, file) in streams.iter().enumerate() {
        assert_eq!(
            fs::seek(file, fs::SeekFrom::Current(0)).unwrap(),
            17 + index as u64
        );
    }
    assert_no_ack(&client, sender);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    if preparation_denial {
        assert!(backing.is_none() && request.failed_for_test());
        if case == "consuming-closure-refusal" {
            assert!(
                capture.is_none(),
                "original capture was consumed by backing preparation"
            );
            assert!(
                b.storage() > storage,
                "real approval/runtime growth stays reserved"
            );
        } else {
            assert_eq!(
                capture
                    .unwrap()
                    .descriptor()
                    .rustc()
                    .argv()
                    .next()
                    .unwrap()
                    .as_ptr(),
                argv0
            );
            assert_eq!(
                b.storage(),
                if case == "received-storage" {
                    STORAGE
                } else {
                    storage
                }
            );
        }
        assert_failed_retry(&mut native, &mut b, &client, sender);
        eprintln!(
            "{} case={case}",
            if case == "consuming-closure-refusal" {
                "ROOT_REQUEST_CONSUMING_REFUSED"
            } else {
                "ROOT_REQUEST_RECEIVED_REFUSED"
            }
        );
        finish(native, &mut b, files, f, &case);
        return;
    }
    let backing =
        backing.expect("fixed-origin runtime and consuming preparation must have succeeded");
    assert!(
        capture.is_none(),
        "received capture moved into backing exactly once"
    );
    assert!(b.failed_work().is_none() && b.failed_storage().is_none());
    let preparation = RootCompilerRequest::preparation_quota().unwrap();
    let continuity = Prepared::maximum_revalidation_quota().unwrap();
    assert!(b.work() - spent <= continuity.work() + Receiver::TURN_WORK + preparation.work());
    assert!(
        b.peak_storage()
            <= peak.max(storage + continuity.scratch() + Receiver::SCRATCH + preparation.scratch())
    );
    assert_eq!(
        backing.descriptor().rustc().argv().next().unwrap().as_ptr(),
        argv0
    );
    assert_eq!(
        b.storage() - storage,
        backing.retained_storage() - capture_storage - Output::STORAGE,
        "only runtime and returned backing growth is newly reserved"
    );
    eprintln!("ROOT_REQUEST_CONSUMED case={case} mask={mask}");
    if case == "unwind" {
        panic!("unwind with complete original request retained");
    }
    if case.starts_with("peer-") {
        native.continuity(&mut b).unwrap();
        // SAFETY: observation only, through this Native's existing creator.
        let before = unsafe { native.creator.cleanup_for_launch() }
            .report()
            .unwrap();
        let result = native.intake(&mut b);
        assert!(
            matches!(
                result,
                Err(Failure::Invalid {
                    reason: "proof helper and original compiler peer credentials overlap",
                    ..
                })
            ),
            "must reach actual approved helper/original peer join: {result:?}"
        );
        let after = unsafe { native.creator.cleanup_for_launch() }
            .report()
            .unwrap();
        assert_eq!(after.work - before.work, Cleanup::GUARD_CLONE_WORK);
        assert_eq!(after.storage, before.storage);
        assert!(after.failed_work.is_none());
        assert!(
            native
                .request
                .as_ref()
                .unwrap()
                .received_for_test()
                .1
                .is_none(),
            "real helper backing preparation consumed the compiler owner"
        );
        assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        assert_no_ack(&client, sender);
        assert_failed_retry(&mut native, &mut b, &client, sender);
        eprintln!("ROOT_REQUEST_PEER_ALIAS_REFUSED case={case}");
        finish(native, &mut b, files, f, &case);
        return;
    }
    let fault = postclone_case(&case);
    if let Some(("helper-ready", action)) = fault {
        exercise_postclone_failure(&mut native, &mut b, &client, sender, "helper-ready", action);
        finish(native, &mut b, files, f, &case);
        return;
    }
    if case.starts_with("mask")
        || case == "trailing"
        || case.starts_with("compiler-")
        || case == "helper-unwind"
        || fault.is_some()
        || case == "two-turn-quota"
    {
        let launch = RootCompilerRequest::launch_quota().unwrap();
        let complete_work = launch.work().checked_add(2 * continuity.work()).unwrap();
        let complete_scratch = launch.scratch().checked_add(continuity.scratch()).unwrap();
        if case == "two-turn-quota" {
            // Restrict the SAME original account across BOTH consuming turns.
            // Logical ballast stays reserved through drain; no fresh budget,
            // counter reset or replenishment occurs at the helper boundary.
            b.charge_work((WORK - b.work()).checked_sub(complete_work).unwrap())
                .unwrap();
            let available = b.storage_limit() - b.storage();
            b.reserve_storage(available.saturating_sub(complete_scratch))
                .unwrap();
            assert_eq!(WORK - b.work(), complete_work);
            assert!(b.storage_limit() - b.storage() <= complete_scratch);
        }
        let launch_work = b.work();
        let launch_storage = b.storage();
        let launch_peak = b.peak_storage();
        turn(&mut native, &mut b);
        let helper = native
            .request
            .as_ref()
            .unwrap()
            .helper_pid_for_test()
            .expect("actual helper exec/READY must complete");
        assert!(disk::metadata(format!("/proc/{}/exe", helper.as_raw_pid())).is_ok());
        assert_no_ack(&client, sender);
        assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        assert!(b.work() - launch_work <= complete_work);
        assert!(b.peak_storage() <= launch_peak.max(launch_storage + complete_scratch));
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(&b as *const Budget<'_> as usize, address);
        eprintln!(
            "ROOT_REQUEST_HELPER_EXEC case={case} pid={}",
            helper.as_raw_pid()
        );
        if case == "helper-unwind" {
            panic!("unwind after genuine helper exec");
        }
        if let Some((phase, action)) = fault {
            exercise_postclone_failure(&mut native, &mut b, &client, sender, phase, action);
            finish(native, &mut b, files, f, &case);
            return;
        }
        turn(&mut native, &mut b);
        let compiler = native
            .request
            .as_ref()
            .unwrap()
            .compiler_pid_for_test()
            .expect("original compiler channel must be joined behind the closed gate");
        // The compiler has not exec'd rustc: its running image is still this test
        // coordinator. The approved image and exact argv/FDs are staged only.
        assert_eq!(
            identity(&File::open(format!("/proc/{}/exe", compiler.as_raw_pid())).unwrap()),
            identity(&File::open("/proc/self/exe").unwrap())
        );
        assert_no_ack(&client, sender);
        assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        assert!(b.work() - launch_work <= complete_work);
        assert!(b.peak_storage() <= launch_peak.max(launch_storage + complete_scratch));
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(&b as *const Budget<'_> as usize, address);
        eprintln!(
            "ROOT_REQUEST_COMPILER_CHANNEL_GATE_CLOSED case={case} pid={}",
            compiler.as_raw_pid()
        );
        if case == "compiler-unwind" {
            panic!("unwind after genuine compiler channel");
        }
        if case == "two-turn-quota" {
            // This control stops before the separate V4-refusal turn. Its real
            // helper/trace owners still require the original pool to drain.
            eprintln!(
                "ROOT_REQUEST_TWO_TURN_QUOTA work={} allowance={complete_work} storage={} scratch={complete_scratch}",
                b.work() - launch_work,
                b.storage() - launch_storage
            );
            finish(native, &mut b, files, f, &case);
            return;
        }
    }
    native.continuity(&mut b).unwrap();
    let mut retained = b.storage();
    match case.as_str() {
        "short-work" | "compiler-work" => {
            let entry = root::LOCAL_WORK + Backing::LOCAL_WORK;
            b.charge_work(WORK - b.work() - entry + 1).unwrap();
            assert!(matches!(
                native.intake(&mut b),
                Err(Failure::Resource(Resource::Work(_)))
            ));
            assert!(b.failed_work().is_some());
        }
        "foreign-account" => {
            foreign.reserve_storage(retained).unwrap();
            let spent = b.work();
            assert!(matches!(
                native.intake(&mut foreign),
                Err(Failure::Resource(Resource::Accounting))
            ));
            assert_eq!(b.work(), spent);
        }
        "moved-account" => {
            let spent = b.work();
            std::mem::swap(&mut b, &mut displaced);
            let same_ledger = displaced.work_ledger_identity_v1() == ledger;
            let distinct_address = &displaced as *const Budget<'_> as usize != address;
            let outcome = native.intake(&mut displaced);
            // Restore the original account to its original address before assertions.
            std::mem::swap(&mut b, &mut displaced);
            assert!(same_ledger && distinct_address);
            assert!(matches!(
                outcome,
                Err(Failure::Resource(Resource::Accounting))
            ));
            assert_eq!(b.work() - spent, root::LOCAL_WORK + Backing::LOCAL_WORK);
        }
        "exhausted-storage" | "compiler-storage" => {
            b.reserve_storage(STORAGE - b.storage()).unwrap();
            retained = b.storage();
            assert!(matches!(
                native.intake(&mut b),
                Err(Failure::Resource(Resource::Storage(_)))
            ));
            assert!(b.failed_storage().is_some());
        }
        "trailing" => {
            proxy.send(last.as_ref().unwrap().canonical_bytes(), None);
            assert!(matches!(
                native.intake(&mut b),
                Err(Failure::Invalid {
                    reason: "trailing input before refusal ACK",
                    ..
                })
            ));
        }
        _ => {
            assert!(case.starts_with("mask"));
            let work = b.work();
            let peak = b.peak_storage();
            assert!(native.intake(&mut b).unwrap());
            let quote = RootCompilerRequest::refusal_quota().unwrap();
            assert!(b.work() - work <= quote.work());
            assert!(b.peak_storage() <= peak.max(retained + quote.scratch()));
            let bytes = launch_io::receive_authenticated_packet::<N>(client.as_fd(), sender)
                .unwrap()
                .unwrap();
            let (expected, _) =
                Record::enforcement_unavailable(last.as_ref().unwrap(), &mut sender_budget)
                    .unwrap();
            assert_eq!(
                &bytes,
                expected.canonical_bytes(),
                "V4 means only runtime-enforcement refusal"
            );
            assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        }
    }
    if !case.starts_with("mask") {
        assert_no_ack(&client, sender);
        if matches!(
            case.as_str(),
            "foreign-account" | "moved-account" | "trailing"
        ) {
            assert!(b.failed_work().is_none() && b.failed_storage().is_none());
        }
        assert_failed_retry(&mut native, &mut b, &client, sender);
    }
    assert_eq!(b.storage(), retained);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    finish(native, &mut b, files, f, &case);
}

fn exercise_postclone_failure<'work>(
    native: &mut Native<'work>,
    b: &mut Budget<'work>,
    client: &OwnedFd,
    sender: MessageSender,
    phase: &'static str,
    action: &'static str,
) {
    native.continuity(b).unwrap();
    native
        .request
        .as_ref()
        .unwrap()
        .arm_postclone_fault_for_test(phase, action, b);
    let storage = b.storage();
    let result = native.intake(b);
    match action {
        "work" => assert!(
            matches!(result, Err(Failure::Resource(Resource::Work(_)))),
            "{result:?}"
        ),
        "storage" => assert!(
            matches!(result, Err(Failure::Resource(Resource::Storage(_)))),
            "{result:?}"
        ),
        "unwind" => panic!("selected postclone unwind returned"),
        _ => unreachable!(),
    }
    let observed = RootCompilerRequest::postclone_fault_observation_for_test();
    assert_eq!((observed.phase, observed.action), (phase, action));
    assert_eq!(
        (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage()
        ),
        (
            observed.work,
            observed.peak,
            observed.failed_work,
            observed.failed_storage
        )
    );
    assert_eq!(
        b.storage(),
        if phase == "helper-ready" {
            observed.storage
        } else {
            storage
        }
    );
    if action == "work" {
        assert!(b.failed_work().is_some() && b.failed_storage().is_none());
    } else {
        assert!(b.failed_storage().is_some() && b.failed_work().is_none());
    }
    assert_no_ack(client, sender);
    assert_failed_retry(native, b, client, sender);
    eprintln!("ROOT_REQUEST_POSTCLONE_REFUSED phase={phase} kind={action}");
}

fn assert_no_ack(client: &OwnedFd, sender: MessageSender) {
    assert!(
        launch_io::receive_authenticated_packet::<N>(client.as_fd(), sender)
            .unwrap()
            .is_none(),
        "no negative or incomplete preparation may send an ACK"
    );
}

fn assert_failed_retry<'work>(
    native: &mut Native<'work>,
    b: &mut Budget<'work>,
    client: &OwnedFd,
    sender: MessageSender,
) {
    assert!(native.request.as_ref().unwrap().failed_for_test());
    let history = (
        b.storage(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    );
    let spent = b.work();
    let retry = native.intake(b);
    if WORK - spent < root::LOCAL_WORK + Backing::LOCAL_WORK {
        assert!(matches!(retry, Err(Failure::Resource(Resource::Work(_)))));
    } else {
        assert!(matches!(
            retry,
            Err(Failure::Invalid {
                reason: "compiler request cannot be reused",
                ..
            })
        ));
    }
    assert!(native.request.as_ref().unwrap().failed_for_test());
    assert!(b.work() >= spent);
    assert_eq!(
        (
            b.storage(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage()
        ),
        history
    );
    assert_no_ack(client, sender);
}

fn finish<'work>(
    mut native: Native<'work>,
    b: &mut Budget<'work>,
    files: [Option<RawFd>; 6],
    f: fixtures::Fixture,
    case: &str,
) {
    let retained = b.storage();
    let history = (
        b.work(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    );
    let ledger = b.work_ledger_identity_v1();
    let address = b as *const Budget<'_> as usize;
    let identities = files.map(|file| file.map(fd_identity));
    let (receiver, backing) = native.request.as_ref().unwrap().received_for_test();
    let capture = receiver
        .received_for_test()
        .0
        .map(|capture| capture.descriptor().rustc().argv().next().unwrap().as_ptr());
    let had_backing = backing.is_some();
    let request = native.request.as_ref().unwrap() as *const RootCompilerRequest;
    native.cancel();
    assert_eq!(
        native.request.as_ref().unwrap() as *const RootCompilerRequest,
        request
    );
    assert_eq!(files.map(|file| file.map(fd_identity)), identities);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        native.pump().unwrap();
        match native.shutdown() {
            Ok(()) => break,
            Err(Failure::Cleanup(CleanupError::Busy)) => {}
            other => panic!("original cleanup refused: {other:?}"),
        }
        assert!(Instant::now() < deadline, "original pool did not drain");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(files.map(|file| file.map(fd_identity)), identities);
    assert_eq!(
        native.request.as_ref().unwrap() as *const RootCompilerRequest,
        request
    );
    let (receiver, backing) = native.request.as_ref().unwrap().received_for_test();
    let (retained_capture, retained_files) = receiver.received_for_test();
    assert_eq!(retained_files, files);
    assert_eq!(
        retained_capture
            .map(|capture| { capture.descriptor().rustc().argv().next().unwrap().as_ptr() }),
        capture
    );
    assert_eq!(backing.is_some(), had_backing);
    drop(native);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(b as *const Budget<'_> as usize, address);
    assert_eq!(
        (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage()
        ),
        history
    );
    assert_eq!(
        b.storage(),
        retained,
        "Drop never refunds caller-owned reservations"
    );
    for file in files.iter().flatten() {
        assert!(disk::metadata(format!("/proc/self/fd/{file}")).is_err());
    }
    f.assert_unlocked();
    drop(f);
    assert!(
        matches!(disk::symlink_metadata(SOCKET), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
        "owned listener pathname must retire after the original cleanup pool"
    );
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 0);
    eprintln!("ROOT_REQUEST_DRAINED case={case}");
}

fn turn<'work>(native: &mut Native<'work>, b: &mut Budget<'work>) {
    native.continuity(b).unwrap();
    assert!(
        !native
            .intake(b)
            .expect("actual root intake and consuming preparation")
    );
}

fn identity(file: &File) -> (u64, u64) {
    let m = file.metadata().unwrap();
    (m.dev(), m.ino())
}

fn fd_identity(file: RawFd) -> (u64, u64) {
    let m = disk::metadata(format!("/proc/self/fd/{file}")).unwrap();
    (m.dev(), m.ino())
}
