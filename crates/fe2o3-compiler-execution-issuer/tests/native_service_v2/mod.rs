//! Actual public native admission/service and client cancellation. This is not
//! inherited-entrypoint, supervisor installation, observed-source or GPU proof.
use super::*;
use fe2o3_compiler_execution_client::CompilerExecutionClientV2 as WireClient;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionClientProcessIdentityV1 as ClientIdentity,
    CompilerExecutionServiceLaunchManifestV2 as Manifest, CompilerExecutionServiceReadyV2 as Ready,
};
use rustix::pipe::{PipeFlags, pipe_with};

pub(super) const CASES: &[&str] = &[
    "serve-ready-cancel",
    "serve-bad-manifest",
    "serve-dead-client",
    "serve-corrupt-journal",
    "serve-foreign-ledger",
    "serve-exhausted-ledger",
];

pub(super) fn run(case: &str) {
    let measurements = current_static_issuer_measurements_v1().unwrap();
    let root = Root::create();
    let mut client = Peer::spawn("client-service", CLIENT_ID);
    let client_peer = receive_fd(&client.control);
    let mut anchor = Peer::spawn("anchor", ANCHOR_ID);
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, 256 * 1024 * 1024);
    b.reserve_storage(EXTRA).unwrap();
    b.charge_work(PREFIX_WORK).unwrap();
    let original = b.work_ledger_identity_v1();
    let Inputs {
        process,
        service,
        policy,
        key,
        anchor: a,
        ..
    } = inputs("success", measurements, &root, &client, &anchor, &mut b);
    let (admission, charge) = Admission::admit(process, service, policy, key, a, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let pid = client.child.0.id() + u32::from(case == "serve-bad-manifest");
    let (manifest, charge) = Manifest::new(
        ClientIdentity::new(pid, CLIENT_ID, CLIENT_ID).unwrap(),
        AnchorIdentity::new(ANCHOR_ID, ANCHOR_ID).unwrap(),
        admission.policy(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    b.reserve_storage(Admission::READINESS_WRITER_STORAGE)
        .unwrap();
    let state = root.path.join("compiler-execution-issuer-v3.state");
    if case == "serve-corrupt-journal" {
        fs::write(&state, b"invalid signed state").unwrap();
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).unwrap();
    }
    if case == "serve-dead-client" {
        client.stop();
    }
    let floor = b.storage();
    if case == "serve-ready-cancel" {
        // This independently expected inert frame cannot cause server publication.
        let (expected, charge) =
            Ready::new(std::process::id(), &manifest, admission.policy(), &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let bytes = *expected.canonical_bytes();
        let policy_bytes = *admission.policy().canonical_bytes();
        let observer = thread::spawn(move || {
            let mut actual = [0; READY_BYTES];
            let mut offset = 0;
            while offset < actual.len() {
                let n = wait_io(IPC_TIMEOUT, || {
                    rustix::io::read(&reader, &mut actual[offset..])
                });
                assert!(n > 0, "readiness closed before the exact frame");
                offset += n;
            }
            assert_eq!(actual, bytes);
            assert_eq!(
                wait_io(IPC_TIMEOUT, || rustix::io::read(&reader, &mut [0; 1])),
                0
            );
            let mut client_work = Work::new(WORK_LIMIT);
            let mut cb = Budget::new(&mut client_work, 256 * 1024 * 1024);
            cb.reserve_storage(policy_bytes.len()).unwrap();
            let (p, charge) = Policy::decode(&policy_bytes, &mut cb).unwrap();
            cb.reserve_storage(charge.additional_storage()).unwrap();
            cb.reserve_storage(WireClient::PEER_STORAGE).unwrap();
            WireClient::admit(client_peer, IPC_TIMEOUT, &mut cb)
                .unwrap()
                .cancel(&p)
                .unwrap();
        });
        admission
            .serve_native_with_readiness(&manifest, writer, &mut b)
            .unwrap();
        observer.join().unwrap();
        assert_eq!(b.storage(), floor + charge.additional_storage());
        assert!(
            state.is_file(),
            "readiness must follow durable genesis recovery"
        );
    } else {
        let result = if case == "serve-foreign-ledger" {
            let mut foreign_work = Work::new(WORK_LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, 256 * 1024 * 1024);
            foreign.reserve_storage(floor).unwrap();
            let used = b.work();
            let result = admission.serve_native_with_readiness(&manifest, writer, &mut foreign);
            assert_eq!(
                result.as_ref().unwrap_err().resource(),
                Some(Resource::Accounting)
            );
            assert_eq!(b.work(), used);
            assert_eq!(foreign.storage(), floor);
            result
        } else {
            if case == "serve-exhausted-ledger" {
                b.charge_work(WORK_LIMIT - b.work()).unwrap();
            }
            admission.serve_native_with_readiness(&manifest, writer, &mut b)
        };
        assert!(result.is_err(), "{case}");
        assert_eq!(b.storage(), floor);
        assert_eq!(
            rustix::io::read(&reader, &mut [0; READY_BYTES]).unwrap(),
            0,
            "refusal must close without any readiness bytes"
        );
        if case == "serve-corrupt-journal" {
            assert_eq!(fs::read(&state).unwrap(), b"invalid signed state");
        } else {
            assert_eq!(fs::read_dir(&root.path).unwrap().count(), 0);
        }
        drop(client_peer);
    }
    assert!(b.work_ledger_identity_v1() == original);
    client.stop();
    anchor.stop();
    // Only this fixture's create-new directory and known journal files are ours.
    for entry in fs::read_dir(&root.path).unwrap() {
        let entry = entry.unwrap();
        assert_eq!(entry.file_name(), "compiler-execution-issuer-v3.state");
        assert!(entry.file_type().unwrap().is_file());
        fs::remove_file(entry.path()).unwrap();
    }
    println!("FE2O3_NATIVE_SERVICE_READINESS_CASE_OK case={case}");
}
