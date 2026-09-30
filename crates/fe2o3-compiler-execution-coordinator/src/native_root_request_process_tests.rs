//! Opt-in consuming-path tests, not Cargo authorship or compiler execution.
//! Requires FE2O3_RUN_PROVISIONED_ROOT_REQUEST=isolated-disposable-root, private
//! /tmp and /run/fe2o3, actual fixed V3 provisioning records/keys/five images, and
//! independently installed immutable matching compiler approval/runtime. Never
//! provisions or rewrites those inputs. No fixed test keys, image substitutions,
//! compiler exec fixture, or synthetic Prepared producer is used by this matrix.
//! Missing/mismatched prerequisites fail, never count as negative-control passes.
//! No compiler or proof-helper process is launched. This is not activation-ABI,
//! service-manager, Cargo-authorship, or executable-enforcement qualification.
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

const WORK: usize = 1 << 60;
const STORAGE: usize = 64 * 1024 * 1024 * 1024;
const HARNESS: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const CASE_ENV: &str = "FE2O3_NATIVE_ROOT_REQUEST_CASE";

impl Native {
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
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &test_name("complete_root_request_case"),
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CASE_ENV, case)
            .output()
            .unwrap();
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
        if case == "unwind" {
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
    let mut b = Budget::new(&mut work, STORAGE);
    let mut displaced = Budget::new(&mut displaced_work, STORAGE);
    b.reserve_storage(HARNESS + FRAME).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    let pool = Cleanup::admit(Account::new(Work::new(WORK), STORAGE)).unwrap();
    // SAFETY: this explicitly selected isolated subprocess owns its real pool;
    // an assertion/unwind cannot return while the creator is still armed.
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
    let client = net::socket_with(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&client, true).unwrap();
    net::connect(&client, &net::SocketAddrUnix::new(SOCKET).unwrap()).unwrap();
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
    assert!(
        launch_io::send_packet(client.as_fd(), hello.canonical_bytes())
            .unwrap()
            .is_some()
    );
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
        assert!(
            launch_io::send_packet_with_descriptor(
                client.as_fd(),
                record.canonical_bytes(),
                file.as_fd()
            )
            .unwrap()
            .is_some()
        );
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
    native.continuity(&mut b).unwrap();
    let mut retained = b.storage();
    match case.as_str() {
        "short-work" => {
            let entry = root::LOCAL_WORK + Backing::LOCAL_WORK;
            b.charge_work(WORK - b.work() - entry + 1).unwrap();
            assert!(matches!(
                native.intake(&mut b),
                Err(Failure::Resource(Resource::Work(_)))
            ));
            assert!(b.failed_work().is_some());
        }
        "foreign-account" => {
            let mut foreign_work = Work::new(WORK);
            let mut foreign = Budget::new(&mut foreign_work, STORAGE);
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
        "exhausted-storage" => {
            b.reserve_storage(STORAGE - b.storage()).unwrap();
            retained = b.storage();
            assert!(matches!(
                native.intake(&mut b),
                Err(Failure::Resource(Resource::Storage(_)))
            ));
            assert!(b.failed_storage().is_some());
        }
        "trailing" => {
            assert!(
                launch_io::send_packet(client.as_fd(), last.as_ref().unwrap().canonical_bytes())
                    .unwrap()
                    .is_some()
            );
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

fn assert_no_ack(client: &OwnedFd, sender: MessageSender) {
    assert!(
        launch_io::receive_authenticated_packet::<N>(client.as_fd(), sender)
            .unwrap()
            .is_none(),
        "no negative or incomplete preparation may send an ACK"
    );
}

fn assert_failed_retry(
    native: &mut Native,
    b: &mut Budget<'_>,
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
    if history.2.is_some() {
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

fn finish(
    mut native: Native,
    b: &mut Budget<'_>,
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

fn turn(native: &mut Native, b: &mut Budget<'_>) {
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
