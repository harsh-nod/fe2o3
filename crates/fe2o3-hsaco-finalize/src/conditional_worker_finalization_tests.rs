//! Real V5 artifact bytes, not a fabricated conditional source/worker owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "../tests/fixtures/nominal_v5_descriptor.rs"]
mod descriptor;
#[allow(dead_code)]
mod elf {
    use crate as fe2o3_hsaco_finalize;
    include!("../tests/fixtures/worker_v3_hsaco_test_support.rs");
    pub(super) fn artifact(wire: &[u8], target: &str, version: u8) -> Vec<u8> {
        let mut bytes =
            slice_fixture_with_descriptor_table_workgroup_target(wire, 256, target).bytes;
        let index = bytes
            .windows(13)
            .position(|v| v == b".fe2o3.kd.v1\0")
            .unwrap();
        bytes[index + 11] = b'0' + version;
        bytes
    }
}

#[test]
fn conditional_artifact_preserves_complete_contract_for_both_targets() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let (abi, _) = descriptor::wires(target, 1, 0, Some(256), "conditional-worker");
        let raw = elf::artifact(&abi, target, 5);
        let floor = raw.len() + abi.len();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, floor + SCRATCH);
        b.reserve_storage(floor).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        derive_launch(&raw, &mut b).unwrap();
        let output = finalize_artifact(&raw, &abi, &mut b).unwrap();
        let checked = crate::inspect_finalized_nominal_hsaco_v5(
            output.as_bytes(),
            SCRATCH,
            &mut descriptor::free,
        )
        .unwrap();
        let offset = checked.location().digest_offset();
        assert_eq!(&output.as_bytes()[..offset], &raw[..offset]);
        assert_eq!(&output.as_bytes()[offset + 32..], &raw[offset + 32..]);
        assert_eq!(output.digest(), checked.digest());
        assert_eq!(
            reconstruct_artifact(output.as_bytes(), &mut b).unwrap(),
            raw
        );
        assert!(!checked.grants_launch_authority());
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor + SCRATCH);
        assert!(b.work() > 19);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn conditional_artifact_rejects_resealed_cpu_substitution_and_downgrade() {
    let target = "gfx942:xnack-";
    let (abi, v3) = descriptor::wires(target, 1, 0, Some(256), "conditional-worker");
    let substituted = descriptor::substitute_cpu(&abi, true);
    let raw = elf::artifact(&substituted, target, 5);
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, raw.len() + abi.len() + SCRATCH);
    b.reserve_storage(raw.len() + abi.len()).unwrap();
    let floor = b.storage();
    // Coherent content is structurally valid, but cannot replace the exact source ABI.
    derive_launch(&raw, &mut b).unwrap();
    assert!(matches!(
        finalize_artifact(&raw, &abi, &mut b),
        Err(Error::Artifact { .. })
    ));
    for (wire, version) in [(&v3, 3), (&v3, 5), (&abi, 4)] {
        let old = elf::artifact(wire, target, version);
        assert!(derive_launch(&old, &mut b).is_err());
        assert!(finalize_artifact(&old, &abi, &mut b).is_err());
    }
    assert_eq!(b.storage(), floor);
}

#[test]
fn conditional_artifact_exact_and_short_resource_limits_keep_original_account() {
    let target = "gfx942:xnack-";
    let (abi, _) = descriptor::wires(target, 1, 0, Some(256), "conditional-worker");
    let raw = elf::artifact(&abi, target, 5);
    let floor = raw.len() + abi.len();
    let mut needed = usize::MAX;
    for case in 0..4 {
        let mut work = Work::new(if case == 2 { needed - 1 } else { needed });
        let mut b = Budget::new(&mut work, floor + SCRATCH - usize::from(case == 3));
        b.reserve_storage(floor).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = derive_launch(&raw, &mut b)
            .and_then(|_| finalize_artifact(&raw, &abi, &mut b))
            .and_then(|artifact| reconstruct_artifact(artifact.as_bytes(), &mut b));
        assert_eq!(result.is_ok(), case < 2);
        if case == 0 {
            needed = b.work();
        }
        if let Err(e) = result {
            assert!(matches!(e, Error::Resource(_)), "{e:?}");
        }
        if case == 3 {
            assert_eq!(b.failed_storage(), Some(floor + SCRATCH));
        }
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn conditional_artifact_replay_rejects_corruption_and_other_descriptor_families() {
    let target = "gfx942:xnack-";
    let (abi, v3) = descriptor::wires(target, 1, 0, Some(256), "conditional-worker");
    let raw = elf::artifact(&abi, target, 5);
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, raw.len() + abi.len() + SCRATCH);
    b.reserve_storage(raw.len() + abi.len()).unwrap();
    let finalized = finalize_artifact(&raw, &abi, &mut b).unwrap();
    let mut corrupted = finalized.as_bytes().to_vec();
    let offset =
        crate::inspect_finalized_nominal_hsaco_v5(&corrupted, SCRATCH, &mut descriptor::free)
            .unwrap()
            .location()
            .digest_offset();
    corrupted[offset] ^= 1;
    assert!(reconstruct_artifact(&corrupted, &mut b).is_err());
    assert!(reconstruct_artifact(&raw, &mut b).is_err());
    for (wire, version) in [(&v3, 3), (&v3, 5), (&abi, 4)] {
        assert!(reconstruct_artifact(&elf::artifact(wire, target, version), &mut b).is_err());
    }
}

#[test]
fn conditional_resource_errors_are_not_reclassified_as_artifact_mismatches() {
    for resource in [
        Resource::Accounting,
        Resource::Arithmetic,
        Resource::Allocation,
    ] {
        for error in [
            NominalFinalizationErrorV5::Work(resource),
            NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Nominal(
                DescriptorWireErrorV3::Work(resource),
            )),
            NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Contract(
                ConditionalInvocationWireErrorV1::Work(resource),
            )),
        ] {
            assert!(
                matches!(descriptor_error("test", error), Error::Resource(actual) if actual == resource)
            );
        }
    }
}
