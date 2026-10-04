use super::*;
use fe2o3_kernel_ir::Constant;

fn dead_constant_module(name: &str, function: &str) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(KirOperation::effect_free(
        ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new(name);
    module.functions.push(Function::internal_helper(
        function,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

#[test]
fn actual_nonempty_check_has_independent_exact_and_one_under_limits() {
    let input = owner(&dead_constant_module("actual-map-work", "f"));
    let (output, _, map) = run(&input);
    // Constant/result/Return => N=3; DCE erases result then producer => E=2.
    // Sources S=2, I=3, O=1, total F=2, B=2; log=2 and target bound=6.
    assert_eq!(map.data.nodes.len(), 3);
    assert_eq!(map.data.events.len(), 2);
    assert_eq!(map.data.relations.len(), 2);
    assert!(
        map.data
            .events
            .iter()
            .all(|event| matches!(event.change, Change::Erase(_)))
    );
    // Census=(1+3)+(2+2+2)+(2+2+1)=15.
    // Check=128*(1+2+2+3+2+3*6+(4*3+3+1+6)*3)=12032.
    let exact_work = 12_047;
    let node_cap = (2 * input.canonical().canonical_bytes().len() + 64).min(131_072);
    let scratch = 1536 * node_cap + 4096;
    for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - work_under);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, 17 + scratch - storage_under);
        budget.reserve_storage(17).unwrap();
        let result = map.check_against(&input, &output, &mut budget);
        match (work_under, storage_under) {
            (1, 0) => {
                assert!(matches!(result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(error)))
                    if error.actual() == 12_058 && error.limit() == 12_057));
                assert_eq!(budget.work(), 26);
                assert_eq!(budget.peak_storage(), 17);
            }
            (0, 1) => {
                assert!(matches!(result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Storage(error)))
                    if error.actual() == 17 + scratch && error.limit() == 16 + scratch));
                assert_eq!(budget.work(), 12_058);
                assert_eq!(budget.peak_storage(), 17);
                assert_eq!(budget.failed_storage(), Some(17 + scratch));
            }
            _ => {
                result.unwrap();
                assert_eq!(budget.work(), 12_058);
                assert_eq!(budget.peak_storage(), 17 + scratch);
            }
        }
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn metadata_padding_does_not_change_actual_map_check_work() {
    for padding in [0, 256] {
        let name = format!("module-{}", "x".repeat(padding));
        let function = format!("function-{}", "x".repeat(padding));
        let input = owner(&dead_constant_module(&name, &function));
        let (output, _, map) = run(&input);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + 12_047);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(17).unwrap();
        map.check_against(&input, &output, &mut budget).unwrap();
        assert_eq!(budget.work(), 12_058);
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn declarations_without_endpoints_still_pay_both_module_scans() {
    for count in [0, 3] {
        let mut module = Module::new("declarations");
        for index in 0..count {
            module.functions.push(Function::declaration(
                format!("external-{index}"),
                Signature::new(vec![], vec![]),
            ));
        }
        let input = owner(&module);
        let (output, _, map) = run(&input);
        assert!(map.data.nodes.is_empty());
        assert!(map.data.events.is_empty());
        // Census=3+2F, check=128*(2+2F), independently of symbol bytes.
        let exact = 259 + 258 * count;
        for under in [0, 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact - under);
            work.charge_work(11).unwrap();
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(17).unwrap();
            let result = map.check_against(&input, &output, &mut budget);
            if under == 0 {
                result.unwrap();
                assert_eq!(budget.work(), 11 + exact);
            } else {
                assert!(matches!(
                    result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(
                        _
                    )))
                ));
                assert_eq!(budget.work(), 14 + 2 * count);
            }
            assert_eq!(budget.storage(), 17);
        }
    }
}

#[test]
fn immutable_output_census_rejects_growth_before_replay_scratch_allocation() {
    let input = owner(&Module::new("m"));
    let (_, _, mut map) = run(&input);
    assert_eq!(input.canonical().canonical_bytes().len(), 37);
    // The unchanged byte-derived cap is 138 nodes. A substituted but verified
    // output contains 138 constants/results plus Return, beyond that cap.
    let mut module = dead_constant_module("oversized-output", "f");
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.clear();
    for id in 0..138 {
        block.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
    }
    let output = owner(&module);
    map.data.output = *output.canonical().identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    assert_eq!(
        map.check_against(&input, &output, &mut budget),
        Err(KirOptimizationMapErrorV12::Limit),
    );
    // Node/input census=1+1; output census=2+2+139, all before Vec allocation.
    assert_eq!(budget.work(), 156);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert_eq!(budget.failed_storage(), None);
}
