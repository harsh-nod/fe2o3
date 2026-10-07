use super::*;
use fe2o3_compiler_execution_protocol::{
    NativeApplicationProofCustodianConfigurationV1 as Config,
    NativeProofCustodianConfigurationPartsV1 as Parts,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn profile(budget: &mut Budget<'_>) -> Config {
    let (config, charge) = Config::new(
        Parts {
            credentials: (61_021, 61_022),
            controller: ([1; 32], 1234),
            analyzer_executable: ([2; 32], 5678),
            analyzer_runtime_closure: ([3; 32], 9012),
            analyzer_identity: [4; 32],
            toolchain_identity: [5; 32],
            verus_identity: [6; 32],
            compiler_policy_identity: [7; 32],
            semantic_policy: ([8; 32], 648),
        },
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    config
}
fn transcript(budget: &mut Budget<'_>, seed: u8) -> Transcript {
    let (value, charge) =
        Transcript::from_untrusted_parts([seed; 32], [seed + 1; 32], [seed + 2; 32], budget)
            .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    value
}

#[test]
fn native_controller_session_requires_exact_independent_profile_and_roles() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let config = profile(&mut budget);
    let transcript = transcript(&mut budget, 20);
    for mutation in 0..7 {
        let mut deployment = config.identity();
        let mut credentials = (1234, 61_021, 61_022);
        if mutation == 1 {
            deployment[0] ^= 1;
        }
        if mutation == 2 {
            credentials.1 += 1;
        }
        if mutation == 3 {
            credentials.2 += 1;
        }
        if mutation >= 4 {
            credentials.0 = mutation - 3;
        }
        let (session, charge) =
            Session::new(transcript, deployment, [40; 32], credentials, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(
            check_session(&session, transcript, &config, 1, 2, 3).is_ok(),
            mutation == 0
        );
    }
}

#[test]
fn ordinary_ready_cannot_decode_native_controller_session() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let transcript = transcript(&mut budget, 20);
    let (ready, charge) = Message::ready(transcript, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_ne!(ready.kind(), Kind::CustodianReady);
    assert!(ready.decode_proof_session(&mut budget).is_err());
}

#[test]
fn native_controller_session_refuses_other_transcript() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let config = profile(&mut budget);
    let first = transcript(&mut budget, 20);
    let second = transcript(&mut budget, 30);
    let (session, charge) = Session::new(
        first,
        config.identity(),
        [40; 32],
        (1234, 61_021, 61_022),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert!(check_session(&session, second, &config, 1, 2, 3).is_err());
}

#[test]
fn native_input_sealing_is_read_only_exact_nonaliased_and_immutable() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    let bytes = b"native content source";
    let deadline = Instant::now() + Duration::from_secs(2);
    let first = seal(bytes, deadline, &mut budget).unwrap();
    let second = seal(bytes, deadline, &mut budget).unwrap();
    let mut read = [0; 32];
    let length = rustix::io::pread(&first, &mut read, 0).unwrap();
    assert_eq!(&read[..length], bytes);
    assert_eq!(
        rustix::io::write(&first, b"x"),
        Err(rustix::io::Errno::BADF)
    );
    let seals = rustix::fs::fcntl_get_seals(&first).unwrap();
    assert!(seals.contains(
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE
    ));
    let left = rustix::fs::fstat(&first).unwrap();
    let right = rustix::fs::fstat(&second).unwrap();
    assert_ne!((left.st_dev, left.st_ino), (right.st_dev, right.st_ino));
    assert_eq!(budget.work(), 2 * (1 + bytes.len() + 1 + 6));
}

#[test]
fn native_input_sealing_refuses_work_or_expired_deadline() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        seal(b"x", Instant::now() + Duration::from_secs(1), &mut budget),
        Err(NativeApplicationChannelErrorV1::Resource(Resource::Work(_)))
    ));
    assert!(budget.failed_work().is_some());
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        seal(b"x", Instant::now(), &mut budget),
        Err(NativeApplicationChannelErrorV1::Transport(
            ApplicationProofChannelErrorV1::Timeout
        ))
    ));
    assert_eq!(budget.work(), 1);
}
