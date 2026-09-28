use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const LIMIT: usize = 1 << 30;
fn fixture() -> (Deployment, [u8; READY_BYTES]) {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let public = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (policy, c) = Policy::new(
        7,
        Measurement::new([1; 32], 11).unwrap(),
        Measurement::new([2; 32], 12).unwrap(),
        public(7),
        public(9),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (deployment, c) = Deployment::new(
        1000,
        1000,
        Service::new(2000, 2000).unwrap(),
        Measurement::new([3; 32], 13).unwrap(),
        Measurement::new([4; 32], 14).unwrap(),
        &policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (ready, c) = Ready::new(42, &deployment, &mut b).unwrap();
    assert_eq!(c.additional_storage(), READY_OWNER_STORAGE);
    (deployment, *ready.canonical_bytes())
}
fn pid(value: i32) -> rustix::process::Pid {
    rustix::process::Pid::from_raw(value).unwrap()
}
fn ready_resource(error: &Error) -> Option<Resource> {
    match error {
        Error::ReadyV2(
            fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV2::Resource(e),
        )
        | Error::ReadyV3(
            fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV3::Resource(e),
        ) => Some(*e),
        _ => None,
    }
}

#[test]
fn native_ready_adapter_uses_exact_pid_context_and_full_returned_charge() {
    let (deployment, bytes) = fixture();
    let floor = deployment.retained_storage() + bytes.len();
    let mut work = Work::new(READY_WORK);
    let mut b = Budget::new(&mut work, floor + READY_SCRATCH);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (ready, c) = decode_ready(&bytes, pid(42), &deployment, &mut b).unwrap();
    assert_eq!(c, READY_OWNER_STORAGE);
    assert_eq!(ready.retained_storage(), c);
    assert_eq!(ready.supervisor_pid(), 42);
    assert_eq!(ready.deployment_identity(), deployment.identity());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), READY_WORK);
    assert_eq!(b.peak_storage(), floor + READY_SCRATCH);
    assert!(b.work_ledger_identity_v1() == ledger);
    b.reserve_storage(c).unwrap();
    drop(ready);
    b.release_storage(c).unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn native_ready_adapter_refuses_wrong_pid_and_preserves_first_denial() {
    let (deployment, bytes) = fixture();
    let floor = deployment.retained_storage() + bytes.len();
    let mut work = Work::new(READY_WORK);
    let limit = floor + READY_SCRATCH;
    let mut b = Budget::new(&mut work, limit);
    assert!(b.reserve_storage(limit + 1).is_err());
    b.reserve_storage(floor).unwrap();
    let error = decode_ready(&bytes, pid(43), &deployment, &mut b).unwrap_err();
    assert!(matches!(error, Error::ReadyV2(fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV2::ContextMismatch)
        | Error::ReadyV3(fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV3::ContextMismatch)));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), READY_WORK);
    assert_eq!(b.failed_storage(), Some(limit + 1));
}

#[test]
fn native_ready_adapter_refuses_short_floor_work_and_scratch() {
    let (deployment, bytes) = fixture();
    let floor = deployment.retained_storage() + bytes.len();
    for mode in 0..3 {
        let paid = floor - usize::from(mode == 0);
        let mut work = Work::new(READY_WORK - usize::from(mode == 1));
        let mut b = Budget::new(&mut work, paid + READY_SCRATCH - usize::from(mode == 2));
        b.reserve_storage(paid).unwrap();
        let error = decode_ready(&bytes, pid(42), &deployment, &mut b).unwrap_err();
        match mode {
            0 => assert_eq!(ready_resource(&error), Some(Resource::Accounting)),
            1 => assert!(b.failed_work().is_some()),
            _ => assert!(b.failed_storage().is_some()),
        }
        assert_eq!(b.storage(), paid);
        assert_eq!(b.work(), if mode < 2 { 8 } else { READY_WORK });
    }
}

#[test]
fn native_ready_adapter_refuses_legacy_and_other_family_framing() {
    let (deployment, bytes) = fixture();
    for version in [b'1', if bytes[7] == b'2' { b'3' } else { b'2' }] {
        let mut altered = bytes;
        altered[7] = version;
        let mut work = Work::new(READY_WORK);
        let floor = deployment.retained_storage() + altered.len();
        let mut b = Budget::new(&mut work, floor + READY_SCRATCH);
        b.reserve_storage(floor).unwrap();
        let error = decode_ready(&altered, pid(42), &deployment, &mut b).unwrap_err();
        assert!(matches!(
            error,
            Error::ReadyV2(
                fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV2::Framing(
                    _
                )
            ) | Error::ReadyV3(
                fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorReadyErrorV3::Framing(
                    _
                )
            )
        ));
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), READY_WORK);
    }
}

#[test]
fn native_launch_descriptor_table_and_retained_layout_cover_actual_types() {
    assert_eq!(DESTINATIONS, [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 220]);
    assert!(FRAME >= 4 * size_of::<Stage>() + 8 * size_of::<Error>());
    assert!(BINDINGS_STORAGE >= size_of::<[Binding<'static>; 11]>());
    assert_eq!(READY_BYTES, 88);
    assert_eq!(Channels::STORAGE, 6 * launch::FILE_STORAGE);
    // No fabricated Prepared/Managed value or protected-startup claim is needed.
    fn send<T: Send>() {}
    send::<Child>();
    let input = size_of::<super::super::Trust>() + size_of::<[Image; 3]>() + (1 << 20);
    assert!(Child::storage_for(input).unwrap() > input);
    assert!(Stage::spawn_retaining_scratch::<Prepared>(input).is_ok());
}

#[test]
fn maximum_compiler_launch_costs_are_inert_checked_and_cover_retention() {
    let retained = Prepared::maximum_retained_storage().unwrap();
    let quota = Prepared::maximum_launch_quota().unwrap();
    assert!(retained >= size_of::<Prepared>());
    assert!(quota.work() > Cleanup::retained_launch_work::<Prepared>(retained).unwrap());
    assert!(quota.scratch() >= Stage::spawn_retaining_scratch::<Prepared>(retained).unwrap());
    assert!(
        Prepared::maximum_preparation_quota().unwrap().work()
            >= Prepared::maximum_revalidation_quota().unwrap().work()
    );
    assert!(Prepared::transfer_source_storage_for([0, 1, 1]).is_err());
    assert!(Prepared::transfer_source_storage_for([1, u64::MAX, 1]).is_err());
}
