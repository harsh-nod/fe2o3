use super::*;
use crate::tests::Fixture;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_static_executable::ProtectedStaticExecutableOperationV2 as Operation;

type Program = AdmittedIssuerProgramV3;
type Failure = IssuerProgramAdmissionErrorV3;
pub(crate) const WORK_LIMIT: usize = 1_000_000_000;
pub(crate) const STORAGE_LIMIT: usize = 10_000_000;
pub(crate) const SEED: [u8; 32] = [0x51; 32];

pub(crate) fn policy(
    executable: CompilerExecutionIssuerMeasurementV1,
    runtime: CompilerExecutionIssuerMeasurementV1,
    generation: u64,
    budget: &mut Budget<'_>,
) -> Policy {
    let (policy, delta) = Policy::new(
        generation,
        executable,
        runtime,
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    policy
}

fn capability(
    f: &Fixture,
    runtime: CompilerExecutionIssuerMeasurementV1,
    b: &mut Budget<'_>,
) -> PolicyCapability {
    let policy = policy(f.issuer_measurement(), runtime, 7, b);
    let (cap, delta) = PolicyCapability::create(policy, b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    cap
}

pub(crate) fn measurement(f: &Fixture) -> Measurement {
    let m = f.measurement();
    Measurement::new(
        m.sha256(),
        m.byte_len(),
        crate::MAX_PROVISIONED_EXECUTABLE_BYTES_V1,
    )
    .unwrap()
}

pub(crate) fn new_program(f: &Fixture, b: &mut Budget<'_>) -> Program {
    let cap = capability(f, sealed_static_issuer_runtime_measurement_v1(), b);
    b.reserve_storage(2 * Image::file_storage(measurement(f)).unwrap())
        .unwrap();
    let (program, delta) = Program::provision(f.open(), f.measurement(), f.open(), cap, b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    program
}

pub(crate) fn revalidation_work(f: &Fixture) -> usize {
    Program::WORK
        + PolicyCapability::IO_WORK
        + 2 * Image::quota(measurement(f), Operation::Revalidate)
            .unwrap()
            .work()
}

#[test]
fn original_account_covers_v3_program_and_exact_distinct_image_transfers() {
    let f = Fixture::new("v3-program-account");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let account = b.work_ledger_identity_v1();
    b.reserve_storage(19).unwrap();
    let cap = capability(&f, sealed_static_issuer_runtime_measurement_v1(), &mut b);
    let identity = cap.policy().identity();
    b.reserve_storage(2 * Image::file_storage(measurement(&f)).unwrap())
        .unwrap();
    let floor = b.storage();
    let start = b.work();
    let source = f.open();
    let duplicate = source.try_clone().unwrap();
    let (program, delta) =
        Program::provision(source, f.measurement(), duplicate, cap, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(
        b.work() - start,
        Program::WORK
            + 2 * PolicyCapability::IO_WORK
            + 2 * Image::quota(measurement(&f), Operation::Admit)
                .unwrap()
                .work()
            + 2 * Image::quota(measurement(&f), Operation::Revalidate)
                .unwrap()
                .work()
    );
    assert_eq!(
        program.retained_storage(),
        floor - 19 + delta.additional_storage()
    );
    b.reserve_storage(delta.additional_storage()).unwrap();
    let _: &Policy = program.policy();
    assert_eq!(program.policy().identity(), identity);
    assert_ne!(
        program.launcher_object_identity(),
        program.issuer_object_identity()
    );
    let start = b.work();
    program.revalidate(&mut b).unwrap();
    assert_eq!(b.work() - start, revalidation_work(&f));
    let (launcher, delta) = program.try_clone_launcher_for_launch(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    program
        .revalidate_launcher_clone(&launcher, &mut b)
        .unwrap();
    assert!(matches!(
        program.revalidate_issuer_clone(&launcher, &mut b),
        Err(Failure::Image(_))
    ));
    let (issuer, delta) = program.try_clone_issuer_for_launch(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    program.revalidate_issuer_clone(&issuer, &mut b).unwrap();
    let (policy, delta) = program.try_clone_policy_for_launch(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    program.revalidate_policy_clone(&policy, &mut b).unwrap();
    drop((launcher, issuer, policy));
    b.release_storage(
        2 * Image::file_storage(measurement(&f)).unwrap() + PolicyCapability::FILE_STORAGE,
    )
    .unwrap();
    assert_eq!(b.storage(), 19 + program.retained_storage());
    assert!(b.work_ledger_identity_v1() == account);
    let retained = program.retained_storage();
    drop(program);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 19);
}

#[test]
fn v3_program_refusals_close_consumed_sources_and_preserve_prepaid_account() {
    let f = Fixture::new("v3-program-refusal");
    for mode in 0..4 {
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let account = b.work_ledger_identity_v1();
        let cap = capability(
            &f,
            CompilerExecutionIssuerMeasurementV1::new([3; 32], 123).unwrap(),
            &mut b,
        );
        b.reserve_storage(2 * Image::file_storage(measurement(&f)).unwrap())
            .unwrap();
        if mode == 0 {
            b.charge_work(WORK_LIMIT - b.work() - (ENTRY - 1)).unwrap();
        } else if mode == 1 {
            b.release_storage(1).unwrap();
        } else if mode == 3 {
            b.reserve_storage(STORAGE_LIMIT - b.storage() - Program::SCRATCH + 1)
                .unwrap();
        }
        let floor = b.storage();
        let start = b.work();
        let (lr, lw) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .unwrap();
        let (ir, iw) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .unwrap();
        let result = Program::provision(lw.into(), f.measurement(), iw.into(), cap, &mut b);
        match mode {
            0 => {
                assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
                assert_eq!(b.work(), start);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Accounting))
                ));
                assert_eq!(b.work() - start, ENTRY);
            }
            2 => {
                assert!(matches!(result, Err(Failure::RuntimePolicyMismatch)));
                assert_eq!(b.work() - start, Program::WORK + PolicyCapability::IO_WORK);
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                ));
                assert_eq!(b.work() - start, Program::WORK);
                assert_eq!(b.failed_storage(), Some(STORAGE_LIMIT + 1));
            }
        }
        // Unique anonymous pipes witness closure, not stale numeric FD slots.
        assert_eq!(rustix::io::read(&lr, &mut [0; 1]).unwrap(), 0);
        assert_eq!(rustix::io::read(&ir, &mut [0; 1]).unwrap(), 0);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == account);
    }
}

#[test]
fn v3_program_revalidation_requires_exact_policy_runtime_image_and_object_joins() {
    let f = Fixture::new("v3-program-joins");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut program = new_program(&f, &mut b);
    for runtime in [true, false] {
        let changed = policy(
            if runtime {
                f.issuer_measurement()
            } else {
                CompilerExecutionIssuerMeasurementV1::new(
                    [9; 32],
                    f.issuer_measurement().byte_len(),
                )
                .unwrap()
            },
            if runtime {
                CompilerExecutionIssuerMeasurementV1::new([8; 32], 123).unwrap()
            } else {
                sealed_static_issuer_runtime_measurement_v1()
            },
            7,
            &mut b,
        );
        let (changed, delta) = PolicyCapability::create(changed, &mut b).unwrap();
        b.reserve_storage(delta.additional_storage()).unwrap();
        let original = std::mem::replace(&mut program.policy, changed);
        let floor = b.storage();
        let start = b.work();
        let result = program.revalidate(&mut b);
        if runtime {
            assert!(matches!(result, Err(Failure::RuntimePolicyMismatch)));
        } else {
            assert!(matches!(result, Err(Failure::PolicyImageMismatch)));
        }
        assert_eq!(b.work() - start, Program::WORK + PolicyCapability::IO_WORK);
        assert_eq!(b.storage(), floor);
        let changed = std::mem::replace(&mut program.policy, original);
        let charge = changed.retained_storage();
        drop(changed);
        b.release_storage(charge).unwrap();
    }
    let floor = b.storage();
    b.release_storage(1).unwrap();
    let start = b.work();
    assert!(matches!(
        program.try_clone_launcher_for_launch(&mut b),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), floor - 1);
    assert_eq!(b.work() - start, ENTRY);
    b.reserve_storage(1).unwrap();
    program.revalidate(&mut b).unwrap();
}

#[test]
fn v2_policy_image_is_rejected_by_actual_v3_decode_before_program_admission() {
    use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2 as OtherCap;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionAttestationErrorV3 as PolicyError,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy,
    };
    let f = Fixture::new("v3-program-family");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (other, delta) = OtherPolicy::new(
        7,
        f.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (other, delta) = OtherCap::create(other, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (file, delta) = other.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let floor = b.storage();
    let error = PolicyCapability::from_file(file, &mut b).err().unwrap();
    assert!(matches!(
        error,
        CapabilityError::PolicyV3(PolicyError::Framing(_))
    ));
    assert!(matches!(
        Failure::from(error),
        Failure::Policy(CapabilityError::PolicyV3(_))
    ));
    assert_eq!(b.storage(), floor);
}
