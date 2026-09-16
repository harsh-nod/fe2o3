use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, ScalarType, Signature, Terminator,
    Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12,
};

fn function(name: &str, calls: &[&str], reads: usize) -> fe2o3_kernel_ir::Function {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut block = BasicBlock::new(BlockId(4_000_000_000));
    for index in 0..reads {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(20 + index as u32), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(index as u32 % 2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    for (index, target) in calls.iter().enumerate() {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: (*target).into(),
                arguments: if index % 2 == 0 {
                    vec![ValueId(0), ValueId(1)]
                } else {
                    vec![ValueId(1), ValueId(0)]
                },
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let signature = Signature::new(vec![pointer.clone(), pointer], vec![]);
    if name.starts_with("root") {
        fe2o3_kernel_ir::Function::kernel_entry(
            name,
            signature,
            vec![ValueId(0), ValueId(1)],
            vec![block],
        )
    } else {
        fe2o3_kernel_ir::Function::internal_helper(
            name,
            signature,
            vec![ValueId(0), ValueId(1)],
            vec![block],
        )
    }
}

fn module(rows: &[(&str, &[&str], usize)]) -> Module {
    let mut module = Module::new("call-effects");
    for (name, calls, reads) in rows {
        module.functions.push(function(name, calls, *reads));
        if name.starts_with("root") {
            module.kernels.push(Kernel::new(
                *name,
                *name,
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(4),
                },
            ));
        }
    }
    module
}

fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}

fn inventory(owner: &VerifiedCanonicalKernelIrModuleV12) -> (Inventory<'_>, usize) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (inventory, storage) = Inventory::derive(owner, &mut budget).unwrap();
    (inventory, storage.retained_storage())
}

fn report<'i, 'g>(inventory: &'i Inventory<'g>) -> (CanonicalKirCallEffectsV1<'i, 'g>, usize) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (report, storage) = CanonicalKirCallEffectsV1::derive(inventory, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (report, storage.retained_storage())
}

#[test]
fn repeated_diamond_calls_preserve_root_arguments_and_leaf_occurrences() {
    let source = module(&[
        ("root", &["left", "left", "right"], 0),
        ("right", &["leaf"], 0),
        ("leaf", &[], 2),
        ("left", &["leaf"], 0),
        ("root_second", &["leaf"], 0),
        ("empty", &[], 0),
    ]);
    let (owner, graph_bytes) = admit(&source);
    let (inventory, inventory_bytes) = inventory(&owner);
    let (report, report_bytes) = report(&inventory);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let floor = graph_bytes + inventory_bytes + report_bytes;
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::CompleteNonempty
    );
    assert_eq!(
        report.decision(Function(5), &mut budget).unwrap(),
        Decision::CompleteEmpty
    );
    let mut events = Vec::new();
    report
        .try_visit(Function(0), &mut budget, |event| {
            let CanonicalKirCallEffectKindV1::Physical(effect) = event.kind() else {
                panic!("physical read");
            };
            events.push((
                event.root(),
                event.function(),
                event.call_path().to_vec(),
                effect.coordinate,
            ));
            Ok::<_, Error>(())
        })
        .unwrap();
    let expected = [vec![0, 4], vec![1, 4], vec![2, 3]]
        .into_iter()
        .flat_map(|path| {
            inventory
                .effects()
                .iter()
                .map(move |effect| (Function(0), Function(2), path.clone(), effect.coordinate))
        })
        .collect::<Vec<_>>();
    assert_eq!(events, expected);
    assert_eq!(budget.storage(), floor);
    assert!(
        events
            .iter()
            .all(|event| event.0 == Function(0) && event.1 == Function(2) && event.2.len() == 2)
    );
    assert_ne!(events[0].2, events[2].2);
    assert_ne!(events[2].2, events[4].2);
    assert_eq!(events[0].3, events[2].3);
    assert_ne!(events[0].3, events[1].3);
    let args = |path: &[usize]| match &inventory.calls()[path[0]].operation.kind {
        OperationKind::Call { arguments, .. } => arguments.clone(),
        _ => unreachable!(),
    };
    assert_eq!(args(&events[0].2), vec![ValueId(0), ValueId(1)]);
    assert_eq!(args(&events[2].2), vec![ValueId(1), ValueId(0)]);
    let mut second_root = Vec::new();
    report
        .try_visit(Function(4), &mut budget, |event| {
            let CanonicalKirCallEffectKindV1::Physical(effect) = event.kind() else {
                panic!("physical read");
            };
            second_root.push((
                event.root(),
                event.function(),
                event.call_path().to_vec(),
                effect.coordinate,
            ));
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(
        second_root,
        inventory
            .effects()
            .iter()
            .map(|effect| (Function(4), Function(2), vec![5], effect.coordinate))
            .collect::<Vec<_>>()
    );
    let (foreign_owner, _) = admit(&source);
    let (foreign_inventory, _) = self::inventory(&foreign_owner);
    assert!(!report.belongs_to(&foreign_inventory));
    assert!(report.belongs_to(&inventory));
}

#[test]
fn cycles_declarations_and_their_callers_are_incomplete_before_callbacks() {
    for calls in [vec!["cycle"], vec!["other"]] {
        let mut source = module(&[
            ("root", &["cycle"], 0),
            ("cycle", &calls, 1),
            ("other", &["cycle"], 0),
            ("pure", &[], 0),
        ]);
        for declaration in [false, true] {
            if declaration {
                source.functions[1].body = None;
                source.functions[1].role = fe2o3_kernel_ir::FunctionRole::ExternalImport;
            }
            let (owner, _) = admit(&source);
            let (inventory, _) = inventory(&owner);
            let (report, _) = report(&inventory);
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            for index in 0..3 {
                assert_eq!(
                    report.decision(Function(index), &mut budget).unwrap(),
                    Decision::Incomplete
                );
                assert_eq!(
                    report.try_visit(Function(index), &mut budget, |_| -> Result<()> {
                        panic!("incomplete must not publish");
                    }),
                    Err(Error::Incomplete(Function(index)))
                );
            }
            assert_eq!(
                report.decision(Function(3), &mut budget).unwrap(),
                Decision::CompleteEmpty
            );
        }
    }
}

#[test]
fn closure_and_path_walk_have_exact_work_storage_limits_and_retry_cleanup() {
    let (owner, graph_bytes) = admit(&module(&[
        ("root", &["reader", "reader"], 0),
        ("reader", &[], 2),
    ]));
    let (inventory, inventory_bytes) = inventory(&owner);
    let floor = graph_bytes + inventory_bytes + 71;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let (report, retained) = CanonicalKirCallEffectsV1::derive(&inventory, &mut budget).unwrap();
    let cost = budget.work();
    let peak = budget.peak_storage();
    for (work_limit, storage_limit, success) in [
        (cost, peak, true),
        (cost - 1, peak, false),
        (cost, peak - 1, false),
        (0, peak, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            CanonicalKirCallEffectsV1::derive(&inventory, &mut budget).is_ok(),
            success
        );
        assert_eq!(budget.storage(), floor);
    }
    let floor = floor + retained.retained_storage();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    report
        .try_visit(Function(0), &mut budget, |_| Ok::<_, Error>(()))
        .unwrap();
    let (cost, peak) = (budget.work(), budget.peak_storage());
    for (work_limit, storage_limit, success) in [
        (cost, peak, true),
        (cost - 1, peak, false),
        (cost, peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            report
                .try_visit(Function(0), &mut budget, |_| Ok::<_, Error>(()))
                .is_ok(),
            success
        );
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(
        report.try_visit(Function(0), &mut budget, |_| Err::<(), _>(
            Error::InvalidFunction(Function(999))
        )),
        Err(Error::InvalidFunction(Function(999)))
    );
    assert_eq!(budget.storage(), floor);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        report.try_visit(Function(0), &mut budget, |_| -> Result<()> {
            panic!("callback panic");
        })
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), floor);
    report
        .try_visit(Function(0), &mut budget, |_| Ok::<_, Error>(()))
        .unwrap();
}

#[test]
fn deep_calls_are_iterative_and_path_expansion_is_budgeted() {
    let names = (0..128).map(|i| format!("helper{i}")).collect::<Vec<_>>();
    let mut source = module(&[("root", &[&names[0]], 0)]);
    for (index, name) in names.iter().enumerate() {
        source.functions.push(if index + 1 < names.len() {
            function(name, &[&names[index + 1]], 0)
        } else {
            function(name, &[], 1)
        });
    }
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let mut count = 0;
    report
        .try_visit(Function(0), &mut budget, |effect| {
            assert_eq!(effect.call_path().len(), 128);
            count += 1;
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(count, 1);
    let mut work = Work::new(500);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(matches!(
        report.try_visit(Function(0), &mut budget, |_| Ok::<_, Error>(())),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 0);
}

#[path = "canonical_kir_call_effects_v1/special_tests.rs"]
mod special_tests;

#[path = "canonical_kir_call_effects_v1/graph_tests.rs"]
mod graph_tests;
