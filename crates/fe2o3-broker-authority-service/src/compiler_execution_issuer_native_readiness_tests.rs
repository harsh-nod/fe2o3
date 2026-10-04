//! Pipe mechanics only; isolated public service tests must be run separately.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::{
    io::Errno,
    pipe::{PipeFlags, pipe_with},
};

fn inputs(b: &mut Budget<'_>) -> (Policy, Manifest) {
    let m = Measurement::new([1; 32], 4096).unwrap();
    let key = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (p, s) = Policy::new(1, m, m, key(7), key(9), b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (m, s) = Manifest::new(
        Client::new(123, 42, 42).unwrap(),
        Anchor::new(43, 43).unwrap(),
        &p,
        b,
    )
    .unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    b.reserve_storage(WRITER_STORAGE).unwrap();
    (p, m)
}

#[test]
fn native_readiness_writes_exact_frame_only_after_validation_and_closes_writer() {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(17).unwrap();
    let (p, m) = inputs(&mut b);
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let floor = b.storage();
    let original = b.work_ledger_identity_v1();
    publish(&m, &p, writer, &mut b, |_| {
        assert_eq!(
            rustix::io::read(&reader, &mut [0; BYTES]),
            Err(Errno::AGAIN)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == original);
    let mut bytes = [0; BYTES];
    assert_eq!(rustix::io::read(&reader, &mut bytes).unwrap(), BYTES);
    assert_eq!(rustix::io::read(&reader, &mut [0; 1]).unwrap(), 0);
    b.reserve_storage(BYTES).unwrap();
    let (ready, storage) = Ready::decode(&bytes, &mut b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    assert!(
        ready
            .matches_launch(std::process::id(), &m, &p, &mut b)
            .unwrap()
    );
}

#[test]
fn native_readiness_failed_revalidation_publishes_nothing() {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (p, m) = inputs(&mut b);
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let floor = b.storage();
    let e = publish(&m, &p, writer, &mut b, |_| {
        Err(Error::rejected("changed retained custody"))
    })
    .unwrap_err();
    assert!(e.to_string().contains("changed retained custody"));
    assert_eq!(b.storage(), floor);
    assert_eq!(rustix::io::read(&reader, &mut [0; BYTES]).unwrap(), 0);
}

#[test]
fn native_readiness_denied_original_work_never_calls_validation_or_writes() {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (p, m) = inputs(&mut b);
    b.charge_work(10_000_000 - b.work()).unwrap();
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let floor = b.storage();
    let e = publish(&m, &p, writer, &mut b, |_| panic!("work denied")).unwrap_err();
    assert!(e.resource().is_some());
    assert_eq!(b.storage(), floor);
    assert_eq!(rustix::io::read(&reader, &mut [0; BYTES]).unwrap(), 0);
}

#[test]
fn native_readiness_requires_nonblocking_pipe_writer_and_no_retry_on_full_pipe() {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (p, m) = inputs(&mut b);
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC).unwrap();
    assert!(check_writer(&reader, &mut b).is_err());
    assert!(check_writer(&writer, &mut b).is_err());
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let mut filled = false;
    for _ in 0..4096 {
        match rustix::io::write(&writer, &[0; 4096]) {
            Ok(n) => assert!(n > 0),
            Err(Errno::AGAIN) => {
                filled = true;
                break;
            }
            Err(e) => panic!("fill pipe: {e}"),
        }
    }
    assert!(filled);
    let floor = b.storage();
    assert!(publish(&m, &p, writer, &mut b, |_| Ok(())).is_err());
    assert_eq!(b.storage(), floor);
    // Keep the reader open throughout the attempted write; no SIGPIPE fixture.
    drop(reader);
}
