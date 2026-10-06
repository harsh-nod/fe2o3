use super::super::ProtectedCompilerExecutionErrorV1 as StartupError;
use super::*;
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV1 as LegacyPolicy;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
use std::{os::fd::OwnedFd, sync::Mutex};

fn startup(policy: File, service: OwnedFd) -> Startup {
    Startup(Mutex::new(Some(Ok(OwnedExecutionInputs {
        policy: policy.into(),
        service,
    }))))
}

fn legacy_policy() -> LegacyPolicy {
    let mut key = [0x66; 32];
    key[0] = 0x58;
    let mut anchor = key;
    anchor[31] ^= 0x80;
    LegacyPolicy::create(
        CompilerExecutionIssuerPolicyV1::new(
            1,
            Measurement::new([1; 32], 1).unwrap(),
            Measurement::new([2; 32], 1).unwrap(),
            key,
            anchor,
        )
        .unwrap(),
    )
    .unwrap()
}

fn sealed(bytes: &[u8]) -> File {
    let file = File::from(
        rustix::fs::memfd_create(
            "dispatch-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    assert_eq!(rustix::io::pwrite(&file, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR).unwrap();
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK,
    )
    .unwrap();
    file
}

#[test]
fn dispatch_legacy_keeps_the_original_budget_and_consumes_once() {
    let policy = legacy_policy();
    let (client, _server) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let startup = startup(policy.try_clone_for_transfer().unwrap(), client);
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
    b.reserve_storage(91).unwrap();
    b.charge_work(19).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    {
        let selected = startup.admit_selected(&mut b).unwrap();
        match selected {
            Admitted::Legacy { session, budget } => {
                assert_eq!(session.policy.policy(), policy.policy());
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget as *const Budget<'_> as usize, address);
                assert!(budget.storage() > 91 && budget.work() > 19);
                session.policy.revalidate().unwrap();
                budget.charge_work(7).unwrap();
            }
            Admitted::Native(_) => panic!("legacy image selected native"),
        }
    }
    assert!(b.work_ledger_identity_v1() == ledger && b.work() > 26);
    assert!(matches!(
        startup.admit(),
        Err(StartupError::InputAlreadyConsumed)
    ));
    assert!(matches!(
        startup.admit_selected(&mut b),
        Err(native_v3::Error::Startup(
            StartupError::InputAlreadyConsumed
        ))
    ));
    policy.revalidate().unwrap();
}

#[test]
fn dispatch_refusal_closes_inputs_without_retry_or_replenishment() {
    use rustix::pipe::{PipeFlags, pipe_with};
    for case in 0..6 {
        let (reader, service) = pipe_with(PipeFlags::CLOEXEC).unwrap();
        let policy = legacy_policy();
        let mut bytes = *policy.policy().canonical_bytes();
        if case == 0 {
            bytes[7] = b'9';
        }
        if case == 1 {
            bytes[7] = b'2';
            bytes[8] = 2;
        }
        if case == 2 {
            bytes[7] = b'3';
            bytes[8] = 3;
        } // Valid V3 header, invalid V3 body/hash.
        let startup = startup(sealed(&bytes), service);
        let mut work = Work::new(if case == 3 { 0 } else { usize::MAX });
        let mut b = Budget::new(&mut work, if case == 4 { 91 } else { 4 * 1024 * 1024 });
        b.reserve_storage(91).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = startup.admit_selected(&mut b);
        match (case, &result) {
            (0 | 1 | 2, Err(native_v3::Error::Policy(_))) => {}
            (
                3,
                Err(native_v3::Error::Policy(
                    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Resource(
                        Resource::Work(_),
                    ),
                )),
            ) => {}
            (4, Err(native_v3::Error::Resource(Resource::Storage(_)))) => {}
            (5, Err(native_v3::Error::Startup(StartupError::Client(_)))) => {}
            _ => panic!("wrong selected refusal"),
        }
        drop(result);
        assert!(b.work_ledger_identity_v1() == ledger && b.storage() >= 91);
        let consumed = b.work();
        let storage = b.storage();
        assert_eq!(rustix::io::read(reader, &mut [0]).unwrap(), 0);
        assert!(matches!(
            startup.admit_native(&mut b),
            Err(native_v3::Error::Startup(
                StartupError::InputAlreadyConsumed
            ))
        ));
        assert!(matches!(
            startup.admit(),
            Err(StartupError::InputAlreadyConsumed)
        ));
        assert_eq!(b.work(), consumed);
        assert_eq!(b.storage(), storage);
    }
}

#[test]
fn driver_dispatch_keeps_matching_publications_and_existing_native_refusal() {
    let source = include_str!("protected_compiler_execution_dispatch_v236.rs");
    let selected = source.split("let family = ").nth(1).unwrap();
    assert_eq!(selected.matches("match family").count(), 1);
    assert_eq!(selected.matches("Legacy::from_owned(inputs)").count(), 1);
    assert_eq!(
        selected
            .matches("native_v3::Admitted::from_owned(inputs, budget)")
            .count(),
        1
    );
    assert!(!selected.contains(".or_else("));
    assert!(source.contains(".publish_worker_handoff(budget, session)"));
    assert!(source.contains(".publish_native_worker_handoff(session)"));
    let prefix = include_str!("production_pipeline_conditional_prefix_v1.rs");
    assert!(prefix.contains("!self.ranked.has_conditional_roots_v1()"));
    assert!(prefix.contains("!= ProductionHelperSourcePolicyV1::RawEmpty"));
    assert!(prefix.contains("conditional F prefix requires original Direct roots"));
}
