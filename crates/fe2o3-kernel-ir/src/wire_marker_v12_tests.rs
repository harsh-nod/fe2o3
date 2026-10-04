use super::*;
use crate::{
    VerificationContractKeyV12, VerificationContractOperationV12, WorkgroupPipelineEventKindV12,
};

fn module(marker: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    if marker {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: VerificationContractKeyV12::new(11),
                    kind: WorkgroupPipelineEventKindV12::Stage,
                    storage: ValueId(0),
                    epoch: ValueId(1),
                },
            ),
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("wire-contract");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

#[test]
fn marker_count_and_comparison_have_fixed_scalar_work_and_zero_auxiliary_payload() {
    let mut counts = vec![];
    let mut comparisons = vec![];
    for marker in [false, true] {
        let module = module(marker);
        let bytes = encode_module_v12(&module).unwrap();
        let mut count = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let extent = count_module_v12_wire_extent_with_work_v1(&module, &mut count).unwrap();
        assert_eq!(extent.wire_bytes(), bytes.len());
        assert_eq!(extent.peak_auxiliary_bytes(), 0);
        counts.push((bytes.len(), count.work()));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut writer = Writer::comparing(KERNEL_IR_VERSION_V12, &bytes, Some(&mut work));
        write_module_v1(&module, &mut writer, true).unwrap();
        assert_eq!(writer.bytes.capacity(), 0);
        assert!(writer.finish_comparison().unwrap());
        comparisons.push(work.work());
    }
    assert_eq!(counts[1].0 - counts[0].0, 19);
    assert_eq!(counts[1].1 - counts[0].1, 7);
    assert_eq!(comparisons[1] - comparisons[0], 26);
}

#[test]
fn comparison_rejects_payload_changes_and_honors_exact_work_boundary() {
    let module = module(true);
    let bytes = encode_module_v12(&module).unwrap();
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    assert!(
        compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &bytes, Some(&mut budget))
            .unwrap()
    );
    let work = budget.work();
    assert!(
        compare_module_encoding_v1(
            &module,
            KERNEL_IR_VERSION_V12,
            &bytes,
            Some(&mut CanonicalKernelIrWorkBudgetV1::new(work))
        )
        .unwrap()
    );
    assert!(
        compare_module_encoding_v1(
            &module,
            KERNEL_IR_VERSION_V12,
            &bytes,
            Some(&mut CanonicalKernelIrWorkBudgetV1::new(work - 1))
        )
        .is_err()
    );
    let start = bytes
        .windows(7)
        .position(|window| window == [30, 1, 11, 0, 0, 0, 1])
        .unwrap();
    for offset in [start + 2, start + 6, start + 7, start + 11] {
        let mut hostile = bytes.clone();
        hostile[offset] ^= 1;
        assert!(
            !compare_module_encoding_v1(&module, KERNEL_IR_VERSION_V12, &hostile, None).unwrap()
        );
    }
}
