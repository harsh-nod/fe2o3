use crate::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, VerifiedCanonicalKernelIrV1 as Canonical,
    VerifiedCanonicalKernelIrV13, OperationKind, ReusablePhaseOperationV1, verify_module};
#[path = "phase_declared_tests/fixture.rs"]
mod fixture;

#[test]
fn declared_phase_capability_report_does_not_claim_support_under_older_versions() {
    let mut matrix = semantic_capability_matrix_v1();
    for row in &matrix.top_level_rows {
        if row.operation != SimulationOperationSurfaceV1::ReusablePhase { continue; }
        match row.kir_wire_version {
            SimulationKirWireVersionV1::V14 => assert!(matches!(row.capability, SimulationCapabilityDispositionV1::Owned { .. })),
            SimulationKirWireVersionV1::V7 | SimulationKirWireVersionV1::V9 | SimulationKirWireVersionV1::V10 |
            SimulationKirWireVersionV1::V11 | SimulationKirWireVersionV1::V12 | SimulationKirWireVersionV1::V13 =>
                assert!(matches!(row.capability, SimulationCapabilityDispositionV1::Unsupported { .. })),
        }
    }
    matrix.top_level_rows.retain(|r| r.kir_wire_version != SimulationKirWireVersionV1::V14
        && r.operation != SimulationOperationSurfaceV1::ReusablePhase);
    matrix.pointer_rows.retain(|r| r.kir_wire_version != SimulationKirWireVersionV1::V14);
    // The new rows do not change any historical version's serialized roster.
    assert_eq!(serde_json::to_vec(&matrix).unwrap().len() + 1, 4_846_425);
}

#[test]
fn declared_phase_admission_retains_exact_version_and_every_source_coordinate() {
    for phases in [1,2] { for original in [fixture::memory(phases), fixture::terminal_drops(fixture::memory(phases))] {
        let canonical = Canonical::from_module(original.clone(), Version::V14).unwrap();
        let identity = *canonical.identity();
        let admitted = AdmittedSimulationModuleV1::admit_declared(canonical, SimulationLimitsV1::default()).unwrap();
        assert_eq!(admitted.identity().wire_version(), 14);
        assert_eq!(admitted.identity().digest(), identity.digest());
        assert!(admitted.capability_projection_receipt_v13().is_none());
        let receipt = admitted.capability_projection_receipt().unwrap();
        assert_eq!(receipt.version(), Version::V14);
        assert!(!receipt.execution_coordinates().is_empty());
        let mut count = 0;
        for (f, function) in original.functions.iter().enumerate() {
            for block in &function.body.as_ref().unwrap().blocks {
                for (i, op) in block.operations.iter().enumerate() {
                    let OperationKind::ReusablePhase(phase) = &op.kind else {continue};
                    let rows = receipt.phase_coordinates().iter().filter(|r|
                        r.function() == f as u32 && r.block() == block.id && r.operation() == i as u32).collect::<Vec<_>>();
                    assert_eq!(rows.len(), op.results.len() + phase.operands.len());
                    for row in rows {
                        assert_eq!(row.source(), &phase.source);
                        match row.coordinate_kind() {
                            SimulationCapabilityCoordinateKindV13::OperationResultDefinition {result} => assert_eq!(row.value(), op.results[result as usize].id),
                            SimulationCapabilityCoordinateKindV13::OperationOperandUse {operand} => assert_eq!(row.value(), phase.operands[operand as usize]),
                            _ => panic!("non-operation phase coordinate"),
                        }
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(receipt.phase_coordinates().len(), count);
        verify_module(admitted.module()).unwrap();
        let operations = &admitted.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
        assert_eq!(operations.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupMemory(_))).count(), 1);
        assert_eq!(operations.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupBarrier(_))).count(), phases*2);
        assert!(operations.iter().all(|o| !matches!(o.kind, OperationKind::ExecutionCapability(_) | OperationKind::ReusablePhase(_))));
        assert!(!admitted.grants_execution_authority());
    } }
}

#[test]
fn declared_phase_public_simulator_executes_two_memory_generations_deterministically() {
    for original in [fixture::memory(2), fixture::terminal_drops(fixture::memory(2))] {
    let kernel = original.kernels[0].id.as_str().to_owned();
    let canonical = Canonical::from_module(original, Version::V14).unwrap();
    let admitted = AdmittedSimulationModuleV1::admit_declared(canonical, SimulationLimitsV1::default()).unwrap();
    let request = SimulationRequestV1::new(kernel, [64,1,1], [64,1,1], vec![]);
    let result = admitted.simulate(&request, SimulationTargetV1::amdgpu_64(), SimulationLimitsV1::default()).unwrap();
    let replay = admitted.simulate(&request, SimulationTargetV1::amdgpu_64(), SimulationLimitsV1::default()).unwrap();
    assert_eq!(result, replay);
    }
}

#[test]
fn declared_phase_strict_v13_owner_cannot_relabel_phase_bytes() {
    let original = fixture::memory(1);
    let canonical = Canonical::from_module(original, Version::V14).unwrap();
    assert!(matches!(VerifiedCanonicalKernelIrV13::from_canonical_bytes(canonical.into_canonical_bytes()),
        Err(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV13::NotExactV13 {version:14})));
}

#[test]
fn declared_phase_legacy_v13_admission_and_receipt_stay_exact() {
    for module in fixture::legacy_modules() {
        let old = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let bytes = old.canonical_bytes().to_vec();
        assert_eq!(old.as_common().version(), Version::V13);
        assert!(std::ptr::eq(old.canonical_bytes().as_ptr(), old.as_common().canonical_bytes().as_ptr()));
        let legacy = AdmittedSimulationModuleV1::admit_v13(old, SimulationLimitsV1::default()).unwrap();
        let common = Canonical::from_canonical_bytes(bytes, Version::V13).unwrap();
        let declared = AdmittedSimulationModuleV1::admit_declared(common, SimulationLimitsV1::default()).unwrap();
        assert_eq!(legacy.identity(), declared.identity());
        assert_eq!(legacy.module(), declared.module());
        assert_eq!(legacy.capability_projection_receipt_v13(), declared.capability_projection_receipt_v13());
        assert!(declared.capability_projection_receipt().unwrap().phase_coordinates().is_empty());
    }
}

#[test]
fn declared_phase_missing_scalar_witness_and_resident_limit_remain_rejections() {
    let canonical = Canonical::from_module(fixture::without_memory(), Version::V14).unwrap();
    assert!(matches!(AdmittedSimulationModuleV1::admit_declared(canonical, SimulationLimitsV1::default()),
        Err(SimulationAdmissionErrorV1::IncompleteExecutionCapabilityV13(IncompleteExecutionCapabilityOperationV13::LdsAllocate))));
    let canonical = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let mut limits = SimulationLimitsV1::default();
    limits.max_resident_bytes = 1;
    assert!(matches!(AdmittedSimulationModuleV1::admit_declared(canonical, limits), Err(SimulationAdmissionErrorV1::ResidentBytesLimit { .. })));
}

#[test]
fn declared_phase_missing_end_or_changed_source_never_reaches_admission() {
    for mutation in 0..2 {
        let mut module = fixture::memory(2);
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        if mutation == 0 {
            ops.retain(|o| !matches!(&o.kind, OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::End { .. })));
        } else {
            let op = ops.iter_mut().find(|o| matches!(&o.kind, OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::RelayClosure { .. }))).unwrap();
            let OperationKind::ReusablePhase(p) = &mut op.kind else {unreachable!()};
            let fe2o3_kernel_ir::PhaseOperationSourceV1::ClosureReturn { source_protocol, .. } = &mut p.source else {unreachable!()};
            *source_protocol = [254;32];
        }
        assert!(matches!(Canonical::from_module(module, Version::V14), Err(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV1::Verification(_))));
    }
}
