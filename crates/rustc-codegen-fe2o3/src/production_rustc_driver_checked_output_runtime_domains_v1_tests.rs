use super::{SourceFailure, SourceStage};
use fe2o3_kernel_ir::{
    FormalAccessDomainV1, FormalMemoryAccessKind, FormalMemoryObligations,
    FormalMemoryReceiptEncodingV4, InertFormalMemoryReceiptFormatV4,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct RuntimeDomainObservation {
    pub(super) reads: usize,
    pub(super) writes: usize,
    pub(super) kernels: usize,
    pub(super) v4_policy3_receipts: usize,
}

// The callback passes the admitted owner's fresh actual-O reports. This
// diagnostic codec selection grants no authority and creates no substitute IR.
pub(super) fn observe(
    reports: &[FormalMemoryObligations],
) -> Result<RuntimeDomainObservation, SourceFailure> {
    let mut observed = RuntimeDomainObservation::default();
    for report in reports {
        let mut has_runtime = false;
        for access in report.accesses() {
            if !matches!(
                access.domain(),
                FormalAccessDomainV1::RuntimeSliceReadBounded(_)
            ) {
                continue;
            }
            has_runtime = true;
            match access.kind() {
                FormalMemoryAccessKind::Read => observed.reads += 1,
                FormalMemoryAccessKind::Write => observed.writes += 1,
                FormalMemoryAccessKind::Atomic => {
                    return Err(SourceFailure::new(
                        SourceStage::Observation,
                        "actual-O runtime read domain attached to an atomic access",
                    ));
                }
            }
        }
        if !has_runtime {
            continue;
        }
        observed.kernels += 1;
        let receipt = InertFormalMemoryReceiptFormatV4::from_current_obligations(report)
            .map_err(|error| SourceFailure::new(SourceStage::Observation, error))?;
        let encoding = receipt.metadata().encoding();
        if encoding != FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
            || encoding.wire_version() != 4
            || encoding.extraction_policy() != 3
            || receipt.grants_authority()
        {
            return Err(SourceFailure::new(
                SourceStage::Observation,
                "actual-O runtime read report did not select inert receipt V4/policy3",
            ));
        }
        observed.v4_policy3_receipts += 1;
    }
    Ok(observed)
}

pub(super) fn check_observation(observed: &super::Observation) -> Result<(), SourceFailure> {
    let runtime = observed.runtime_domains.as_ref().ok_or_else(|| {
        SourceFailure::new(
            SourceStage::Observation,
            "actual-O runtime-domain observation unavailable",
        )
    })?;
    if runtime.writes != 0
        || runtime.reads > observed.global_reads
        || runtime.kernels > observed.roots.len()
        || runtime.kernels > runtime.reads
        || (runtime.kernels == 0) != (runtime.reads == 0)
        || runtime.v4_policy3_receipts != runtime.kernels
    {
        return Err(SourceFailure::new(
            SourceStage::Observation,
            "actual-O runtime reads, roots and V4/policy3 receipt observations disagree",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, ExplicitLaunchExtent1d,
        FormalIndexWidth, Function, Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess,
        Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef,
        ValueId, derive_kernel_memory_obligations_from_verified, verify_module_ref,
    };

    fn report(runtime: bool) -> FormalMemoryObligations {
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
        let mut entry = BasicBlock::new(BlockId(10));
        entry.terminator = Some(Terminator::Return { values: vec![] });
        let mut successors = Vec::new();
        if runtime {
            entry.operations = vec![
                op(
                    2,
                    Type::INDEX,
                    OperationKind::SliceLength { slice: ValueId(0) },
                ),
                op(
                    3,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(1),
                        rhs: ValueId(2),
                    },
                ),
                op(
                    4,
                    pointer.clone(),
                    OperationKind::SliceData { slice: ValueId(0) },
                ),
            ];
            entry.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(20),
                then_arguments: vec![],
                else_target: BlockId(30),
                else_arguments: vec![],
            });
            let mut read = BasicBlock::new(BlockId(20));
            read.operations = vec![
                op(
                    5,
                    pointer,
                    OperationKind::GetElementPointer {
                        base: ValueId(4),
                        offset: ValueId(1),
                    },
                ),
                op(
                    6,
                    Type::Scalar(ScalarType::U32),
                    OperationKind::Load {
                        pointer: ValueId(5),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
            ];
            read.terminator = Some(Terminator::Return { values: vec![] });
            let mut exit = BasicBlock::new(BlockId(30));
            exit.terminator = Some(Terminator::Return { values: vec![] });
            successors.extend([read, exit]);
        }
        let mut blocks = vec![entry];
        blocks.extend(successors);
        let mut module = Module::new("runtime-observation-component");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    ),
                    Type::INDEX,
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1)],
            blocks,
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        let analysis = derive_kernel_memory_obligations_from_verified(
            verify_module_ref(&module).unwrap(),
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(64),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(analysis.is_complete(), "{analysis:?}");
        analysis.obligations().clone()
    }

    fn observation() -> super::super::Observation {
        super::super::Observation {
            source_route: None,
            roots: vec!["kernel".to_owned()],
            transparent_result_wrappers: None,
            internal_helpers: 0,
            helper_calls: 0,
            reads: 1,
            writes: 0,
            global_reads: 1,
            global_writes: 0,
            private_reads: 0,
            private_writes: 0,
            other_reads: 0,
            other_writes: 0,
            formal_accesses: 1,
            runtime_domains: None,
            simulation: None,
            policy: 4,
            output_digest: [1; 32],
            llvm_bytes: 1,
            descriptor_roots: 1,
            missing_proof_refused: false,
        }
    }

    #[test]
    fn genuine_reports_select_v4_policy3_only_for_runtime_domains() {
        let legacy = report(false);
        assert_eq!(
            observe(std::slice::from_ref(&legacy)).unwrap(),
            RuntimeDomainObservation::default()
        );
        let runtime = report(true);
        let observed = observe(&[legacy, runtime]).unwrap();
        assert_eq!(
            observed,
            RuntimeDomainObservation {
                reads: 1,
                writes: 0,
                kernels: 1,
                v4_policy3_receipts: 1
            }
        );
    }

    #[test]
    fn historical_json_without_runtime_observation_stays_unobserved() {
        let old = observation();
        let bytes = serde_json::to_vec(&old).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.get("runtime_domains").is_none());
        assert!(json.get("transparent_result_wrappers").is_none());
        let decoded: super::super::Observation = serde_json::from_slice(&bytes).unwrap();
        assert!(decoded.runtime_domains.is_none());
        assert!(decoded.transparent_result_wrappers.is_none());
        assert!(check_observation(&decoded).is_err());
        let mut current = observation();
        current.runtime_domains = Some(RuntimeDomainObservation::default());
        let decoded: super::super::Observation =
            serde_json::from_slice(&serde_json::to_vec(&current).unwrap()).unwrap();
        assert_eq!(
            decoded.runtime_domains,
            Some(RuntimeDomainObservation::default())
        );
        check_observation(&decoded).unwrap();
    }

    #[test]
    fn runtime_observation_rejects_writes_missing_receipts_and_impossible_counts() {
        let mut observed = observation();
        let valid = observe(&[report(true)]).unwrap();
        observed.runtime_domains = Some(valid);
        check_observation(&observed).unwrap();
        for bad in [
            RuntimeDomainObservation { writes: 1, ..valid },
            RuntimeDomainObservation { reads: 2, ..valid },
            RuntimeDomainObservation {
                kernels: 2,
                ..valid
            },
            RuntimeDomainObservation {
                v4_policy3_receipts: 0,
                ..valid
            },
            RuntimeDomainObservation {
                kernels: 0,
                v4_policy3_receipts: 0,
                ..valid
            },
        ] {
            observed.runtime_domains = Some(bad);
            assert!(check_observation(&observed).is_err(), "{bad:?}");
        }
    }
}
