//! Real fixed-slot refusals are isolated from the parent test process's FDs.
use super::super::tests::{assert_closed, install, isolated, leave_service_hole};
use super::*;
use fe2o3_compiler_execution_client::COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 as SERVICE_FD;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::fs::File;

#[test]
fn native_session_refusal_closes_both_inputs_without_resetting_the_account() {
    if isolated(concat!(
        module_path!(),
        "::native_session_refusal_closes_both_inputs_without_resetting_the_account"
    )) {
        return;
    }
    for (prepaid, limit) in [
        (Admitted::INPUT_STORAGE - 1, usize::MAX),
        (Admitted::INPUT_STORAGE, 0),
        (Admitted::INPUT_STORAGE, usize::MAX),
    ] {
        let file = File::open("/dev/null").unwrap();
        for fd in [POLICY_FD, SERVICE_FD] {
            install(&file, fd, false);
        }
        let mut work = Work::new(limit);
        let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
        b.reserve_storage(prepaid).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = Admitted::admit(&mut b);
        if prepaid < Admitted::INPUT_STORAGE {
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        } else if limit == 0 {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        } else {
            assert!(matches!(result, Err(Error::Policy(_))));
        }
        drop(result);
        assert_eq!(b.storage(), prepaid);
        assert!(b.work_ledger_identity_v1() == ledger);
        if limit == 0 {
            assert!(b.failed_work().is_some());
        }
        assert_closed();
    }
    missing_service_slot_cannot_become_a_private_policy_file();
}

fn missing_service_slot_cannot_become_a_private_policy_file() {
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3,
    };
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
    // Two fixed valid public points for an inert sealed-policy fixture only.
    let mut key = [0x66; 32];
    key[0] = 0x58;
    let mut anchor = key;
    anchor[31] ^= 0x80;
    let (policy, charge) = CompilerExecutionIssuerPolicyV3::new(
        1,
        Measurement::new([1; 32], 1).unwrap(),
        Measurement::new([2; 32], 1).unwrap(),
        key,
        anchor,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (policy, charge) = Policy::create(policy, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (file, charge) = policy.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    install(&file, POLICY_FD, false);
    let occupied = leave_service_hole(&file);
    b.reserve_storage(Admitted::INPUT_STORAGE).unwrap();
    let before = b.work();
    let floor = b.storage();
    let result = Admitted::admit(&mut b);
    assert!(matches!(&result, Err(Error::Descriptor(e)) if e.raw_os_error() == Some(libc::EBADF)));
    drop(result);
    assert_eq!(b.work(), before + 2);
    assert_eq!(b.storage(), floor);
    assert_closed();
    policy.revalidate(&mut b).unwrap();
    drop(occupied);
}
