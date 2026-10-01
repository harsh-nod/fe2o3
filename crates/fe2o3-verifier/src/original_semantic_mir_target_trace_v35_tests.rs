use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV18;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Function as KirFunction, Module, Operation,
    OperationKind, ScalarType, Signature, StorageLayoutLimitsV1, SwitchCase, Terminator, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18,
};
use std::mem::align_of;

const LIMIT: usize = 100_000_000;

fn module(cyclic: bool, switch: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(if switch {
        Terminator::Switch {
            selector: ValueId(0),
            cases: vec![SwitchCase {
                value: 0,
                target: BlockId(1),
                arguments: vec![ValueId(1), ValueId(0)],
            }],
            default_target: BlockId(1),
            default_arguments: vec![ValueId(0), ValueId(1)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(1), ValueId(0)],
        }
    });
    let mut adapter = BasicBlock::new(BlockId(1));
    adapter.parameters.extend([
        ValueDef::new(ValueId(2), scalar.clone()),
        ValueDef::new(ValueId(3), scalar.clone()),
    ]);
    adapter.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4), scalar.clone()),
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(2),
            rhs: ValueId(3),
        },
    ));
    adapter.terminator = Some(Terminator::Branch {
        target: BlockId(if cyclic { 1 } else { 2 }),
        arguments: vec![ValueId(3), ValueId(2)],
    });
    let mut end = BasicBlock::new(BlockId(2));
    end.parameters.extend([
        ValueDef::new(ValueId(5), scalar.clone()),
        ValueDef::new(ValueId(6), scalar.clone()),
    ]);
    end.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("original-mir-concrete-connectors");
    module.functions.push(KirFunction::internal_helper(
        "connectors",
        Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, adapter, end],
    ));
    module
}

fn run(
    cyclic: bool,
    switch: bool,
    work: usize,
    storage: usize,
    consume: impl FnOnce(&Inventory<'_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize) {
    let module = module(cyclic, switch);
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        let (owner, receipt) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                StorageLayoutLimitsV1 {
                    rows: 1,
                    edges: 1,
                    containment_depth: 1,
                    object_bytes: 16,
                },
                &mut budget,
            )
            .map_err(|_| Error::Statement("connector canonical fixture"))?;
        budget.reserve_storage(receipt.retained_storage())?;
        let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        budget.reserve_storage(super::super::super::super::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut budget)?;
        consume(&inventory, &mut writer)
    })();
    let measured = (budget.work(), budget.peak_storage());
    budget.release_storage(budget.storage()).unwrap();
    (result, measured.0, measured.1)
}

fn targets(inventory: &Inventory<'_>, out: &mut Writer<'_, '_>) -> Result<Vec<TargetBlock>> {
    let mut rows = vector(inventory.blocks().len(), out)?;
    for block in 0..inventory.blocks().len() {
        rows.push(TargetBlock::derive(inventory, block, out)?);
    }
    Ok(rows)
}

fn capacity(rows: &[TargetBlock]) -> usize {
    2 + rows
        .iter()
        .map(|row| row.program.nodes.len())
        .sum::<usize>()
}

fn inspect(inventory: &Inventory<'_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let targets = targets(inventory, out)?;
    let mut trace = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
    let (end, path) = trace.connectors(
        Block {
            function: Function(0),
            block: 0,
        },
        &targets,
        &[false, false, true],
        out,
    )?;
    assert_eq!(
        end,
        Block {
            function: Function(0),
            block: 2
        }
    );
    assert_eq!(path, [0, 1]);
    assert_eq!(trace.nodes.len(), 3);
    let definition = |id, out: &mut Writer<'_, '_>| -> Result<usize> {
        Ok(inventory
            .definition_index_for_value(Function(0), ValueId(id), out.budget)?
            .unwrap())
    };
    let a = definition(0, out)?;
    let b = definition(1, out)?;
    assert_eq!(trace.nodes[0].expression, ExpressionV30::Argument(a as u32));
    assert_eq!(trace.nodes[1].expression, ExpressionV30::Argument(b as u32));
    assert_eq!(
        trace.nodes[2].expression,
        ExpressionV30::Binary {
            operation: super::super::OperatorV30::Xor,
            left: 1,
            right: 0
        }
    );
    assert_eq!(trace.values[definition(2, out)?], Some(1));
    assert_eq!(trace.values[definition(3, out)?], Some(0));
    assert_eq!(trace.values[definition(5, out)?], Some(0));
    assert_eq!(trace.values[definition(6, out)?], Some(1));
    Ok(())
}

#[test]
fn original_mir_actual_connector_trace_preserves_operations_and_simultaneous_swaps() {
    run(false, false, LIMIT, LIMIT, inspect).0.unwrap();
}

#[test]
fn original_mir_actual_connector_trace_has_exact_and_one_short_resources() {
    let measured = run(false, false, LIMIT, LIMIT, inspect);
    measured.0.unwrap();
    let exact = run(false, false, measured.1, measured.2, inspect);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    assert!(matches!(
        run(false, false, measured.1 - 1, measured.2, inspect).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(false, false, measured.1, measured.2 - 1, inspect).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn original_mir_actual_connectors_refuse_cycles_branches_and_missing_boundaries() {
    for fault in 0..3 {
        let result = run(fault == 0, fault == 1, LIMIT, LIMIT, |inventory, out| {
            let targets = targets(inventory, out)?;
            let mut trace = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
            trace
                .connectors(
                    Block {
                        function: Function(0),
                        block: 0,
                    },
                    &targets,
                    if fault == 2 {
                        &[false, false, false]
                    } else {
                        &[false, false, true]
                    },
                    out,
                )
                .map(|_| ())
        });
        assert!(matches!(
            result.0,
            Err(Error::Statement(
                "original MIR concrete connector trace differs"
            ))
        ));
    }
}

#[test]
fn original_mir_actual_edge_token_cannot_cross_trace_or_epoch() {
    for fault in 0..2 {
        run(false, false, LIMIT, LIMIT, |inventory, out| {
            let targets = targets(inventory, out)?;
            let mut first = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
            let mut second = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
            let old = first.append(&targets[0], out)?;
            if fault == 0 {
                assert!(matches!(
                    second.edge(old, 0, out),
                    Err(Error::Statement(
                        "original MIR concrete connector trace differs"
                    ))
                ));
            } else {
                let current = first.append(&targets[0], out)?;
                assert!(matches!(
                    first.edge(old, 0, out),
                    Err(Error::Statement(
                        "original MIR concrete connector trace differs"
                    ))
                ));
                assert_eq!(
                    first.edge(current, 0, out)?,
                    Block {
                        function: Function(0),
                        block: 1
                    }
                );
            }
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_actual_trace_query_refuses_foreign_ledger_before_work_or_storage() {
    run(false, false, LIMIT, LIMIT, |inventory, out| {
        let targets = targets(inventory, out)?;
        let mut trace = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(out.budget.storage())?;
        let mut foreign = Writer::new(&mut budget)?;
        let before = (
            foreign.budget.work(),
            foreign.budget.storage(),
            foreign.budget.peak_storage(),
        );
        assert!(matches!(
            trace.append(&targets[0], &mut foreign),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(
            (
                foreign.budget.work(),
                foreign.budget.storage(),
                foreign.budget.peak_storage()
            ),
            before
        );
        assert!(foreign.text.is_empty());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_connector_refuses_equal_byte_foreign_canonical_owner() {
    run(false, false, LIMIT, LIMIT, |inventory, out| {
        let target = targets(inventory, out)?;
        let mut trace = ConcreteTrace::arguments(inventory, 0, capacity(&target), out)?;
        let same_bytes = module(false, false);
        let (foreign_owner, credit) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &same_bytes,
                StorageLayoutLimitsV1 {
                    rows: 1,
                    edges: 1,
                    containment_depth: 1,
                    object_bytes: 16,
                },
                out.budget,
            )
            .map_err(|_| Error::Statement("foreign connector fixture"))?;
        out.budget.reserve_storage(credit.retained_storage())?;
        assert_eq!(foreign_owner.module(), inventory.owner().module());
        let (foreign, credit) = CanonicalKirInventoryV18::derive_v18(&foreign_owner, out.budget)?;
        out.budget.reserve_storage(credit.retained_storage())?;
        let foreign_block = TargetBlock::derive(&foreign, 0, out)?;
        let before = (
            out.budget.work(),
            out.budget.storage(),
            out.budget.peak_storage(),
        );
        assert!(matches!(
            trace.append(&foreign_block, out),
            Err(Error::Statement(
                "original MIR target block has a foreign owner"
            ))
        ));
        assert_eq!(
            (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage()
            ),
            before
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_edge_token_requires_its_complete_retained_floor() {
    let result = run(false, false, LIMIT, LIMIT, |inventory, out| {
        let targets = targets(inventory, out)?;
        let mut trace = ConcreteTrace::arguments(inventory, 0, capacity(&targets), out)?;
        let appended = trace.append(&targets[0], out)?;
        out.budget.release_storage(1)?;
        let before = (
            out.budget.work(),
            out.budget.storage(),
            out.budget.peak_storage(),
        );
        let result = trace.edge(appended, 0, out).map(|_| ());
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(
            (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage()
            ),
            before
        );
        result
    });
    assert!(matches!(
        result.0,
        Err(Error::Resource(Resource::Accounting))
    ));
}

fn trace_header_oracle() -> usize {
    #[allow(dead_code)]
    struct Fields<'a> {
        inventory: &'a Inventory<'a>,
        function: Function,
        definitions: Range<usize>,
        nodes: Vec<NodeV30>,
        values: Vec<Option<usize>>,
        slot: usize,
        ledger: Ledger,
        required: usize,
        epoch: usize,
    }
    type Token<'a> = (&'a TargetBlock, Vec<usize>, usize, usize, usize);
    assert_eq!(
        (
            size_of::<ConcreteTrace<'_, '_>>(),
            align_of::<ConcreteTrace<'_, '_>>()
        ),
        (size_of::<Fields<'_>>(), align_of::<Fields<'_>>())
    );
    assert_eq!(
        (
            size_of::<AppendedBlock<'_>>(),
            align_of::<AppendedBlock<'_>>()
        ),
        (size_of::<Token<'_>>(), align_of::<Token<'_>>())
    );
    type Frame<'a> = (
        Fields<'a>,
        Result<Fields<'a>>,
        &'a Inventory<'a>,
        &'a mut Writer<'a, 'a>,
        &'a TargetBlock,
        &'a [TargetBlock],
        &'a [bool],
        &'a [usize],
        Vec<usize>,
        Vec<bool>,
        Result<Vec<usize>>,
        Result<(Block, Vec<usize>)>,
        Token<'a>,
        Result<Token<'a>>,
        NodeV30,
        ExpressionV30,
        Result<ExpressionV30>,
        Result<Block>,
        Result<()>,
        Result<usize>,
        bool,
        [usize; 12],
        &'a mut Fields<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        &'a Token<'a>,
    );
    size_of::<Frame<'_>>() + align_of::<Frame<'_>>()
}

#[test]
fn original_mir_actual_trace_frames_have_independent_field_oracles() {
    assert_eq!(headers(), trace_header_oracle());
    type TargetFields = (
        usize,
        Block,
        super::super::canonical::CanonicalProgramV30,
        Vec<super::super::canonical::control::TargetEdge>,
        TargetBranch,
        Vec<bool>,
    );
    assert_eq!(
        (size_of::<TargetBlock>(), align_of::<TargetBlock>()),
        (size_of::<TargetFields>(), align_of::<TargetFields>())
    );
}

#[test]
fn original_mir_actual_trace_uses_independent_exact_work_and_storage_arithmetic() {
    run(false, false, LIMIT, LIMIT, |inventory, out| {
        let rows = targets(inventory, out)?;
        assert_eq!(inventory.functions()[0].definitions.len(), 7);
        assert_eq!(
            rows.iter()
                .map(|row| row.program.nodes.len())
                .collect::<Vec<_>>(),
            [2, 3, 1]
        );
        assert_eq!(rows[0].edges[0].arguments.len(), 2);
        assert_eq!(rows[1].edges[0].arguments.len(), 2);
        // Two seed arguments, seven definition slots; two connector blocks,
        // three stop/visit checks, five translated terms, and four edge writes.
        // This oracle never reads a previous execution's work/storage counters.
        let work = 7 + 3 * 2 + 3 + 4 * 3 + 5 * (2 + 3) + 2 * 7 * 2 + 3 * 4;
        assert_eq!(work, 93);
        let payload = 8 * size_of::<NodeV30>()
            + 7 * size_of::<Option<usize>>()
            + 3 * size_of::<bool>()
            + (3 + 2 + 3) * size_of::<usize>();
        let floor = 31 + super::super::super::super::SOURCE_LIMIT;
        let storage = floor + trace_header_oracle() + payload;
        for (limit_work, limit_storage, failure) in [
            (work, storage, 0),
            (work - 1, storage, 1),
            (work, storage - 1, 2),
        ] {
            let mut ledger = Work::new(limit_work);
            let mut budget = Budget::new(&mut ledger, limit_storage);
            budget.reserve_storage(floor)?;
            let result = (|| {
                let mut writer = Writer::new(&mut budget)?;
                let mut trace = ConcreteTrace::arguments(inventory, 0, 8, &mut writer)?;
                let (end, path) = trace.connectors(
                    Block {
                        function: Function(0),
                        block: 0,
                    },
                    &rows,
                    &[false, false, true],
                    &mut writer,
                )?;
                assert_eq!(end.block, 2);
                assert_eq!(path, [0, 1]);
                Ok(())
            })();
            match failure {
                0 => {
                    result?;
                    assert_eq!((budget.work(), budget.storage()), (work, storage));
                }
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            budget.release_storage(budget.storage() - floor)?;
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_trace_keeps_old_and_new_translation_backing_paid() {
    run(false, false, LIMIT, LIMIT, |inventory, out| {
        let rows = targets(inventory, out)?;
        let mut trace = ConcreteTrace::arguments(inventory, 0, 8, out)?;
        let base = out.budget.storage();
        let work = out.budget.work();
        let old = trace.append(&rows[0], out)?;
        let current = trace.append(&rows[0], out)?;
        assert_eq!(out.budget.storage() - base, 4 * size_of::<usize>());
        assert_eq!(out.budget.work() - work, 2 * (5 * 2 + 2 * 7));
        assert_eq!(old.translated.len(), 2);
        assert_eq!(current.translated.len(), 2);
        assert!(matches!(trace.edge(old, 0, out), Err(Error::Statement(_))));
        assert_eq!(trace.edge(current, 0, out)?.block, 1);
        Ok(())
    })
    .0
    .unwrap();
}
