//! Canonical planning/accounting probes, not original-source admission evidence.
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function as IrFunction, Module,
    Signature, StorageLayoutLimitsV1, ValueDef, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

#[test]
fn control_reconstruction_plan_headers_match_independent_owner_and_scratch_fields() {
    #[allow(dead_code)]
    struct Fields<'a, 'g> {
        input: &'a Inventory<'g>,
        slot: usize,
        ledger: CanonicalKernelIrWorkLedgerIdentityV1,
        floor: usize,
        retained: usize,
        failure: std::cell::Cell<Option<Resource>>,
        root: Term,
        controls: Vec<Select>,
    }
    assert_eq!(size_of::<Fields<'_, '_>>(), size_of::<Plan<'_, '_>>());
    let vectors = size_of::<Vec<Select>>()
        + size_of::<Vec<u8>>()
        + 2 * size_of::<Vec<Option<Term>>>()
        + size_of::<Vec<(usize, usize)>>();
    let allocations = size_of::<Result<Vec<Select>>>()
        + size_of::<Result<Vec<u8>>>()
        + 2 * size_of::<Result<Vec<Option<Term>>>>()
        + size_of::<Result<Vec<(usize, usize)>>>();
    let returns = 4 * size_of::<Result<Term>>()
        + 6 * size_of::<Result<usize>>()
        + 2 * size_of::<Result<()>>()
        + size_of::<Result<Fields<'_, '_>>>()
        + 4 * size_of::<Result<Block>>()
        + 2 * size_of::<Result<bool>>();
    let query = 4 * size_of::<Term>()
        + 3 * size_of::<Select>()
        + 4 * size_of::<Block>()
        + 2 * size_of::<Function>()
        + 2 * size_of::<Definition>()
        + size_of::<RefusalFacts>()
        + 2 * size_of::<Type>()
        + 6 * size_of::<std::ops::Range<usize>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 4 * size_of::<Option<Block>>()
        + 4 * size_of::<Option<Term>>()
        + 4 * size_of::<bool>()
        + size_of::<std::slice::Iter<'_, Select>>();
    let owner_check =
        2 * size_of::<Option<Resource>>() + 2 * size_of::<Resource>() + size_of::<Result<()>>();
    let expected =
        size_of::<Fields<'_, '_>>() + vectors + allocations + returns + query + owner_check;
    assert_eq!(headers(), expected);
    with_input(true, |input, original, retained| {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT, retained);
        budget
            .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
            .unwrap();
        let mut out = Writer::new(&mut budget).unwrap();
        let floor = out.budget.storage();
        let plan = derive(input, original, &mut out).unwrap();
        assert_eq!(
            out.budget.storage() - floor,
            expected + plan.controls.capacity() * size_of::<Select>()
        );
        plan.discard(&mut out).unwrap();
        assert_eq!(out.budget.storage(), floor);
        assert!(out.finish().unwrap().is_empty());
    });
}

fn with_input<T>(nested: bool, run: impl FnOnce(&Inventory<'_>, usize, usize) -> T) -> T {
    let module = super::super::tests::diamond(false, nested);
    with_module(&module, run)
}

fn with_module<T>(module: &Module, run: impl FnOnce(&Inventory<'_>, usize, usize) -> T) -> T {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (input, input_storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let original = input
        .definitions()
        .iter()
        .position(|row| row.value == Some(ValueId(4)))
        .unwrap();
    run(
        &input,
        original,
        owner_storage.retained_storage() + input_storage.retained_storage(),
    )
}

#[test]
fn control_reconstruction_identity_preserves_bypassed_and_nonclosed_joins() {
    let word = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(90));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(17),
        then_arguments: vec![ValueId(0)],
        else_target: BlockId(63),
        else_arguments: vec![],
    });
    let mut arm = BasicBlock::new(BlockId(17));
    arm.parameters.push(ValueDef::new(ValueId(4), word.clone()));
    arm.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut bypass = BasicBlock::new(BlockId(63));
    bypass.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut single = Module::new("single-incoming-bypassed-phi");
    single.functions.push(IrFunction::internal_helper(
        "entry",
        Signature::new(
            vec![word.clone(), Type::Scalar(ScalarType::Bool)],
            vec![word],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, arm, bypass],
    ));
    let mut equal = super::super::tests::diamond(true, false);
    let right = equal.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == BlockId(400))
        .unwrap();
    let Some(Terminator::Branch { arguments, .. }) = &mut right.terminator else {
        panic!("right arm branch");
    };
    arguments[0] = ValueId(0);
    for module in [single, equal] {
        with_module(&module, |input, original, retained| {
            let mut work = Work::new(LIMIT);
            let mut budget = prepared(&mut work, LIMIT, retained);
            budget
                .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
                .unwrap();
            let mut out = Writer::new(&mut budget).unwrap();
            let floor = out.budget.storage();
            let source = identity(input, original, &mut out).unwrap().unwrap();
            assert_eq!(input.definitions()[source].value, Some(ValueId(0)));
            assert!(matches!(
                input.definitions()[source].coordinate,
                Definition::FunctionArgument { .. }
            ));
            assert_eq!(out.budget.storage(), floor);
            let failure = match derive(input, original, &mut out) {
                Ok(plan) => {
                    plan.discard(&mut out).unwrap();
                    panic!("side exit must not be a closed-region witness");
                }
                Err(error) => error,
            };
            assert!(
                matches!(failure, Error::SourceReconstruction { facts, .. } if facts.phase == "cfg-region")
            );
            assert!(out.finish().unwrap().is_empty());
        });
    }
}

fn prepared<'w>(work: &'w mut Work, limit: usize, retained: usize) -> Budget<'w> {
    let mut budget = Budget::new(work, limit);
    budget.reserve_storage(FLOOR + retained).unwrap();
    budget
}

fn run(
    input: &Inventory<'_>,
    original: usize,
    retained: usize,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = prepared(&mut work, storage, retained);
    let floor = budget.storage();
    let result = (|| {
        // Writer's fixed allocation is prepaid by its owning generation scope.
        budget.reserve_storage(
            crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT + super::super::headers(),
        )?;
        let mut out = Writer::new(&mut budget)?;
        let count = input.definitions().len();
        let mut recipes = vector(count, &mut out)?;
        let mut marks = vector(count, &mut out)?;
        let mut stack = vector(count, &mut out)?;
        let mut order = vector(count, &mut out)?;
        out.budget
            .charge_work(count.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        recipes.resize(count, None);
        marks.resize(count, 0u8);
        recipes[original] = Some(Recipe::Region(original));
        marks[original] = 1;
        stack.push((original, 0));
        let facts = RefusalFacts::new(
            original,
            input.definitions()[original].coordinate,
            input.definitions()[original].ty,
        );
        traverse(
            &mut recipes,
            &mut marks,
            &mut stack,
            &mut order,
            facts,
            Some(input),
            &mut out,
            &mut |index, _| {
                assert!(
                    index < input.definitions().len(),
                    "auxiliary selectors cannot enter original lookup"
                );
                assert!(matches!(
                    input.definitions()[index].coordinate,
                    Definition::FunctionArgument { .. }
                ));
                Ok(Recipe::Actual(index))
            },
        )?;
        assert!(
            recipes.len() > count,
            "nested source must exercise paid arena growth"
        );
        assert!(order.iter().any(|index| *index >= count));
        assert!(stack.is_empty());
        assert!(order.iter().all(|index| marks[*index] == 2));
        emit_nodes(input, &recipes, &order, &mut out)?;
        let text = out.finish()?;
        assert!(text.contains("reconstructed_branch_"));
        assert!(text.contains("else { reconstructed_ok_"));
        drop(text);
        Ok(())
    })();
    // All planner, traversal and writer objects have dropped before owner cleanup.
    let used = budget.work();
    let peak = budget.peak_storage();
    budget.rollback_storage(floor).unwrap();
    assert_eq!(budget.storage(), FLOOR + retained);
    (result, used, peak)
}

#[test]
fn control_reconstruction_preserves_diamond_emission_and_original_dependency_cycles() {
    for equal in [false, true] {
        let mut module = super::super::tests::diamond(false, false);
        if equal {
            let right = module.functions[0]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == BlockId(400))
                .unwrap();
            let Some(Terminator::Branch { arguments, .. }) = &mut right.terminator else {
                panic!("right branch");
            };
            arguments[0] = ValueId(0);
        }
        with_module(&module, |input, original, retained| {
            let emit = |old: bool, cycle: bool| {
                let mut work = Work::new(LIMIT);
                let mut budget = prepared(&mut work, LIMIT, retained);
                budget
                    .reserve_storage(
                        crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT
                            + super::super::headers(),
                    )
                    .unwrap();
                let mut out = Writer::new(&mut budget).unwrap();
                let count = input.definitions().len();
                let mut recipes = vector(count, &mut out).unwrap();
                let mut marks = vector(count, &mut out).unwrap();
                let mut stack = vector(count, &mut out).unwrap();
                let mut order = vector(count, &mut out).unwrap();
                recipes.resize(count, None);
                marks.resize(count, 0);
                let Definition::BlockArgument { block, .. } =
                    input.definitions()[original].coordinate
                else {
                    panic!("original phi");
                };
                recipes[original] = Some(if old {
                    super::super::phi(input, original, block, &mut out).unwrap()
                } else {
                    Recipe::Region(original)
                });
                marks[original] = 1;
                stack.push((original, 0));
                let facts = RefusalFacts::new(
                    original,
                    input.definitions()[original].coordinate,
                    input.definitions()[original].ty,
                );
                let result = traverse(
                    &mut recipes,
                    &mut marks,
                    &mut stack,
                    &mut order,
                    facts,
                    Some(input),
                    &mut out,
                    &mut |index, _| {
                        // This cycle is deliberately synthetic recipe dependency coverage,
                        // not a claim that malformed original SSA passed admission.
                        Ok(if cycle {
                            Recipe::Forward(original)
                        } else {
                            Recipe::Actual(index)
                        })
                    },
                );
                if cycle {
                    assert!(
                        matches!(result, Err(Error::SourceReconstruction { facts, .. }) if facts.phase == "traversal-cycle")
                    );
                    assert!(order.is_empty());
                } else {
                    result.unwrap();
                    emit_nodes(input, &recipes, &order, &mut out).unwrap();
                }
                out.finish().unwrap()
            };
            let before = emit(true, false);
            let after = emit(false, false);
            assert!(!after.is_empty());
            assert_eq!(after.as_bytes(), before.as_bytes());
            assert!(emit(false, true).is_empty());
        });
    }
}

#[test]
fn control_reconstruction_arena_has_exact_and_one_short_resource_limits() {
    with_input(true, |input, original, retained| {
        let measured = run(input, original, retained, LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(input, original, retained, measured.1, measured.2);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(
            matches!(run(input, original, retained, measured.1 - 1, measured.2).0,
            Err(Error::Resource(Resource::Work(error))) | Err(Error::Flow(fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1)
        );
        assert!(
            matches!(run(input, original, retained, measured.1, measured.2 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) | Err(Error::Flow(fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1::Resource(Resource::Storage(error))))
            if error.actual() == measured.2 && error.limit() == measured.2 - 1)
        );
    });
}

#[test]
fn control_reconstruction_plan_rejects_foreign_owner_account_and_reduced_floor() {
    with_input(true, |input, original, retained| {
        with_input(true, |other, _, _| {
            for violation in 0..4 {
                let mut work = Work::new(LIMIT);
                let mut other_work = Work::new(LIMIT);
                let mut budget = prepared(&mut work, LIMIT, retained);
                budget
                    .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
                    .unwrap();
                let mut out = Writer::new(&mut budget).unwrap();
                let floor = out.budget.storage();
                let plan = derive(input, original, &mut out).unwrap();
                let paid = out.budget.storage();
                let error = match violation {
                    0 => plan.check(other, &mut out),
                    1 => {
                        let mut other_budget = prepared(&mut other_work, LIMIT, retained);
                        other_budget.reserve_storage(paid).unwrap();
                        let mut foreign = Writer::new(&mut other_budget).unwrap();
                        let error = plan.check(input, &mut foreign);
                        assert!(foreign.finish().unwrap().is_empty());
                        error
                    }
                    2 => {
                        let mut replacement = Budget::new(&mut other_work, LIMIT);
                        replacement.reserve_storage(paid).unwrap();
                        let saved = std::mem::replace(out.budget, replacement);
                        let error = plan.check(input, &mut out);
                        let rejected = std::mem::replace(out.budget, saved);
                        drop(rejected);
                        error
                    }
                    3 => {
                        out.budget.release_storage(1).unwrap();
                        let error = plan.check(input, &mut out);
                        out.budget.reserve_storage(1).unwrap();
                        error
                    }
                    _ => unreachable!(),
                };
                assert!(matches!(error, Err(Error::Resource(Resource::Accounting))));
                // Restoring a slot, ledger or floor does not clear the owner latch.
                assert!(matches!(
                    plan.check(input, &mut out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert!(matches!(
                    plan.discard(&mut out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(out.budget.storage(), paid);
                out.budget.rollback_storage(floor).unwrap();
                assert!(out.finish().unwrap().is_empty());
            }
        });
    });
}

#[test]
fn control_reconstruction_owned_scratch_drops_before_unwind_refund() {
    with_input(true, |input, original, retained| {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT, retained);
        budget
            .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
            .unwrap();
        let mut out = Writer::new(&mut budget).unwrap();
        let floor = out.budget.storage();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let plan = derive(input, original, &mut out).unwrap();
            assert!(!plan.controls.is_empty());
            plan.check(input, &mut out).unwrap();
            std::panic::panic_any(291u32);
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&291));
        assert!(
            out.budget.storage() > floor,
            "unwinding cannot silently refund live owner accounting"
        );
        out.budget.rollback_storage(floor).unwrap();
        assert_eq!(out.budget.storage(), floor);
        assert!(out.finish().unwrap().is_empty());
    });
}

#[test]
fn control_reconstruction_plan_keeps_first_denial_across_foreign_and_restored_queries() {
    with_input(true, |input, original, retained| {
        let mut measure_work = Work::new(LIMIT);
        let mut measure = prepared(&mut measure_work, LIMIT, retained);
        measure
            .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
            .unwrap();
        let mut out = Writer::new(&mut measure).unwrap();
        let plan = derive(input, original, &mut out).unwrap();
        let used = out.budget.work();
        drop(plan);
        assert!(out.finish().unwrap().is_empty());

        let mut work = Work::new(used + 3);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT, retained);
        budget
            .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
            .unwrap();
        let mut out = Writer::new(&mut budget).unwrap();
        let floor = out.budget.storage();
        let plan = derive(input, original, &mut out).unwrap();
        let Error::Resource(first) = plan.check(input, &mut out).unwrap_err() else {
            panic!("exact query denial");
        };
        assert!(
            matches!(first, Resource::Work(error) if error.actual() == used + 4 && error.limit() == used + 3)
        );
        let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
        foreign_budget
            .reserve_storage(out.budget.storage())
            .unwrap();
        let mut foreign = Writer::new(&mut foreign_budget).unwrap();
        assert!(
            matches!(plan.check(input, &mut foreign), Err(Error::Resource(next)) if next == first)
        );
        assert!(foreign.finish().unwrap().is_empty());
        out.budget.release_storage(1).unwrap();
        assert!(matches!(plan.check(input, &mut out), Err(Error::Resource(next)) if next == first));
        out.budget.reserve_storage(1).unwrap();
        assert!(matches!(plan.discard(&mut out), Err(Error::Resource(next)) if next == first));
        out.budget.rollback_storage(floor).unwrap();
        assert!(out.finish().unwrap().is_empty());
    });
}
