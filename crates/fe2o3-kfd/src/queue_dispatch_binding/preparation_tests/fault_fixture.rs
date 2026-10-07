//! Exact preparation-stage and backend-fault qualification helpers.

use super::*;

pub(super) fn stage_fault(stage: PreparationStageV1, panic: bool, configured: bool) {
    let mut memory = Memory::new(configured);
    let data = memory.roster();
    let expected = inputs(&data);
    let before = memory.observation();
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    let original_bytes = owner.packets[0].kernarg_bytes.clone();
    owner.fault = Some((stage, panic));
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut owner, &mut memory)));
    if panic {
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<(&str, PreparationStageV1)>(),
            Some(&("dispatch preparation", stage))
        );
    } else {
        assert!(result.unwrap().is_err());
    }
    assert_eq!(owner.stage, stage);
    assert!(owner.failed);
    assert!(owner.take_completed().is_err());
    assert_eq!(owner.packets[0].kernarg_bytes, original_bytes);
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    assert_eq!(
        memory.observation().phase,
        if owner.native_started {
            SharedMemorySessionPhaseV1::Quarantined
        } else {
            SharedMemorySessionPhaseV1::Active
        }
    );
    if !owner.native_started {
        assert_eq!(memory.observation(), before);
    }
    let (prefix, controls) = match stage {
        PreparationStageV1::Generation
        | PreparationStageV1::Plan
        | PreparationStageV1::Capacity
        | PreparationStageV1::DataRetention => (0, 0),
        PreparationStageV1::CodeAllocate(index) => (index, index),
        PreparationStageV1::CodeMaterialize(index)
        | PreparationStageV1::CodeSeal(index)
        | PreparationStageV1::CodeMap(index)
        | PreparationStageV1::CodeRetain(index)
        | PreparationStageV1::CodeResolve(index) => (index, index + 1),
        PreparationStageV1::KernargAllocate => (3, 3),
        PreparationStageV1::Complete => {
            assert_eq!(owner.completed.as_ref().unwrap().code.len(), 3);
            (0, 4)
        }
        _ => (3, 4),
    };
    assert_eq!(owner.code.len(), prefix);
    assert_eq!(memory.observation().controls, controls);
    let unchanged = memory.observation();
    assert!(run(&mut owner, &mut memory).is_err());
    assert_eq!(memory.observation(), unchanged);
    assert_custody(&memory, &owner);
}

pub(super) fn native_fault(call: Call, fault: NativeFault, panic: bool, configured: bool) {
    let mut memory = Memory::new(configured);
    let data = memory.roster();
    let expected = inputs(&data);
    let before = memory.observation();
    memory.fault = Some((call, fault));
    let expected_stage = match call {
        Call::AllocateCode(i) => PreparationStageV1::CodeAllocate(i),
        Call::WriteCode(i) => PreparationStageV1::CodeMaterialize(i),
        Call::SealCode(i) => PreparationStageV1::CodeSeal(i),
        Call::MapCode(i) => PreparationStageV1::CodeMap(i),
        Call::AllocateKernarg => PreparationStageV1::KernargAllocate,
        Call::WriteKernarg => PreparationStageV1::KernargMaterialize,
        Call::MapKernarg => PreparationStageV1::KernargMap,
    };
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut owner, &mut memory)));
    if panic {
        let operation = match fault {
            NativeFault::Panic(operation) => operation,
            NativeFault::CurrentnessPanic(_) => "currentness",
            NativeFault::AccessPanic => "with_bytes_mut",
            _ => unreachable!("panic test requires panic fault"),
        };
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        );
    } else {
        assert!(
            result.unwrap().is_err(),
            "native fault {call:?} must reject"
        );
    }
    assert_eq!(owner.stage, expected_stage);
    assert!(owner.failed);
    assert_eq!(
        memory.observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    let prefix = match call {
        Call::AllocateCode(i) | Call::WriteCode(i) | Call::SealCode(i) | Call::MapCode(i) => i,
        _ => 3,
    };
    assert_eq!(owner.code.len(), prefix);
    assert_eq!(owner.code_identity.len(), prefix);
}
