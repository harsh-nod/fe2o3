use super::*;
use fe2o3_kernel_ir::{ExecutionCapabilitySourceV1, SubgroupPartitionOperationV1 as K};

// Observations of the live replayed owner, not constructed source-carrier authority.
pub(super) struct SourceMaximum {
    function: [u8; 32],
    operation: [u8; 32],
    source_block: u32,
    root: [u8; 32],
    expansion: [u8; 32],
    view: [u8; 32],
    instance: u32,
    expanded_block: u32,
}

pub(super) fn source_sites(owner: &ProductionSemanticSsaOwnerV1) -> Vec<SourceMaximum> {
    owner.verify_replay().unwrap();
    let source = owner.source_semantic();
    let [root] = source.roots() else {
        panic!("one actual registered root")
    };
    let view = owner.execution_view_for_root(*root).unwrap();
    view.body()
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(block, body)| {
            let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                return None;
            };
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = &source.callables()[call.callee().index() as usize]
            else {
                return None;
            };
            if !matches!(
                contract.operation(),
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                    SemanticSubgroupPartitionOperationV1::ReduceMaxF32 { .. }
                )
            ) {
                return None;
            }
            assert!(view.has_expanded_calls());
            let origin = &view.block_origins()[block];
            assert_eq!(
                origin.terminator(),
                fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source
            );
            Some(SourceMaximum {
                function: *source.functions()[origin.function().index() as usize]
                    .identity()
                    .as_bytes(),
                operation: *binding.identity().as_bytes(),
                source_block: origin.block().index(),
                root: *source.functions()[root.index() as usize]
                    .identity()
                    .as_bytes(),
                expansion: *owner.execution_expansion().identity(),
                view: *view.identity(),
                instance: origin.instance().index(),
                expanded_block: u32::try_from(block).unwrap(),
            })
        })
        .collect()
}

pub(super) fn check_module(
    module: &Module,
    mut expected: Vec<SourceMaximum>,
) -> Vec<ExecutionCapabilitySourceV1> {
    let mut sources = Vec::new();
    for operation in module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
    {
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            continue;
        };
        let E::SubgroupPartition(K::ReduceMaxF32 {
            partition_reference,
            partition,
            element,
            width,
            partition_width,
        }) = contract.operation
        else {
            continue;
        };
        assert_ne!(partition_reference, partition);
        assert_eq!((width, partition_width), (64, 16));
        assert_eq!(
            contract.signature.arguments().collect::<Vec<_>>(),
            [partition_reference, element]
        );
        assert_eq!(contract.signature.output(), element);
        assert_eq!(contract.operands.len(), 2);
        assert_eq!(operation.results.len(), 1);
        assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::F32));
        let occurrence = contract
            .source
            .occurrence
            .expect("max must retain expanded source custody");
        let index = expected
            .iter()
            .position(|site| {
                site.expanded_block == occurrence.expanded_block()
                    && site.instance == occurrence.caller_instance()
            })
            .expect("each maximum must correspond to one observed source call");
        let site = expected.remove(index);
        assert_eq!(contract.source.function, site.function);
        assert_eq!(contract.source.operation, site.operation);
        assert_eq!(contract.source.block, site.source_block);
        assert_eq!(occurrence.root_source_identity(), site.root);
        assert_eq!(occurrence.expansion_identity(), site.expansion);
        assert_eq!(occurrence.expanded_root_identity(), site.view);
        sources.push(contract.source);
    }
    assert!(
        expected.is_empty(),
        "a real maximum was dropped or relabeled"
    );
    for mutation in 0..3 {
        if sources.is_empty() {
            break;
        }
        let mut changed = module.clone();
        let operation = changed.functions.iter_mut().filter_map(|f| f.body.as_mut())
            .flat_map(|b| &mut b.blocks).flat_map(|b| &mut b.operations)
            .find(|op| matches!(&op.kind, OperationKind::ExecutionCapability(c) if matches!(c.operation, E::SubgroupPartition(K::ReduceMaxF32 { .. })))).unwrap();
        let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
            unreachable!()
        };
        match mutation {
            0 => contract.epoch_before.as_mut().unwrap()[0] ^= 1,
            1 => {
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    contract.obligations.bits() & !ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
                )
            }
            2 => contract.source.occurrence = None,
            _ => unreachable!(),
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(changed).is_err(),
            "accepted max mutation {mutation}"
        );
    }
    sources
}

pub(super) fn check_amd(ir: &str, maximums: usize) {
    if maximums == 0 {
        return;
    }
    assert_eq!(
        maximums, 2,
        "two real calls through the retained source helper"
    );
    assert_eq!(
        ir.lines()
            .filter(|line| line.contains(" = fcmp olt float "))
            .count(),
        4 * maximums
    );
    for forbidden in [
        "fadd float",
        "llvm.max",
        "maxnum",
        "fmax",
        "fcmp fast",
        "select fast",
        "nnan",
        "nsz",
    ] {
        assert!(
            !ir.contains(forbidden),
            "unexpected numerical transformation: {forbidden}"
        );
    }
}

pub(super) fn check_sim(
    admitted: &fe2o3_kir_sim::AdmittedSimulationModuleV1,
    expected: &[ExecutionCapabilitySourceV1],
) {
    let receipt = admitted.capability_projection_receipt_v13().unwrap();
    let actual = receipt.coordinates().iter().filter(|coordinate|
        coordinate.execution_family() == Some(fe2o3_kir_sim::SimulationExecutionCapabilityFamilyV13::SubgroupPartitionReduceMaxF32)
    ).map(|coordinate| coordinate.source().expect("sim projection must retain the exact max source")).collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len());
    for source in expected {
        assert_eq!(actual.iter().filter(|actual| *actual == source).count(), 1);
    }
}
