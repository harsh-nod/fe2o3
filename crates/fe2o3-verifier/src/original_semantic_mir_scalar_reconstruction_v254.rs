//! Necessary scalar equations over retained snapshots, not transition authority.
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::operation_index;
use crate::mixed_optimizer_refinement_v26::semantics::original_scalar_v30::vector;
use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CheckedBinaryOperator, Constant, ExecutionOperationV15 as Execution, ExecutionRoleV15,
    FormalIndexWidth, OperationKind, ScalarType, Terminator, ValueId,
};
use std::fmt::Write as _;

#[path = "original_semantic_mir_control_reconstruction_v291.rs"]
mod control_reconstruction;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Recipe {
    // Unexpanded original phi. Never emitted or interpreted as a control-node ID.
    Region(usize),
    Actual(usize),
    Literal(u32),
    Forward(usize),
    CheckedAdd {
        left: usize,
        right: usize,
        overflow: bool,
    },
    Select {
        condition: usize,
        on_true: usize,
        on_false: usize,
    },
}

impl Recipe {
    fn child(self, ordinal: u8) -> Option<usize> {
        match (self, ordinal) {
            (Self::Forward(value), 0) => Some(value),
            (Self::CheckedAdd { left, .. }, 0) => Some(left),
            (Self::CheckedAdd { right, .. }, 1) => Some(right),
            (Self::Select { condition, .. }, 0) => Some(condition),
            (Self::Select { on_true, .. }, 1) => Some(on_true),
            (Self::Select { on_false, .. }, 2) => Some(on_false),
            _ => None,
        }
    }
}

pub(super) fn headers() -> usize {
    // Simultaneous wrapper, iterative traversal, and recipe-query frames.
    let wrapper_refs = 6 * size_of::<&()>();
    let recipe_refs = 10 * size_of::<&()>();
    let operand_refs = 4 * size_of::<&()>();
    let traversal_borrows =
        size_of::<&mut Vec<Option<Recipe>>>() + size_of::<&mut Vec<u8>>() + 4 * size_of::<&()>();
    let loader_frame = (5 + 2) * size_of::<&()>()
        + size_of::<usize>()
        + size_of::<Recipe>()
        + size_of::<Result<Recipe>>();
    // Both inputs, row, operation, binary/operand borrows and a temporary type
    // borrow coexist with coordinates and fallible operand lookup results.
    let pure_result_frame = 8 * size_of::<&()>()
        + 4 * size_of::<usize>()
        + size_of::<Definition>()
        + size_of::<Constant>()
        + 2 * size_of::<Type>()
        + size_of::<bool>()
        + size_of::<Option<usize>>()
        + 2 * size_of::<Result<Recipe>>()
        + 2 * size_of::<Result<usize>>();
    let snapshot_path = size_of::<[u32; 2]>() + size_of::<&[u32]>();
    let borrowed_descendants = size_of::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>();
    let frame_indices = (14 + 8 + 8 + 2) * size_of::<usize>();
    let diagnostic_results = 2 * size_of::<Result<Recipe>>() + 2 * size_of::<Result<()>>();
    let control_install = size_of::<control_reconstruction::Plan<'_, '_>>()
        + size_of::<Result<control_reconstruction::Plan<'_, '_>>>()
        + 3 * size_of::<control_reconstruction::Select>()
        + 4 * size_of::<control_reconstruction::Term>()
        + size_of::<Option<&Inventory<'_>>>()
        + size_of::<
            std::iter::Enumerate<
                std::iter::Copied<std::slice::Iter<'_, control_reconstruction::Select>>,
            >,
        >()
        + 4 * size_of::<Result<usize>>()
        + 2 * size_of::<Result<Recipe>>()
        + 16 * size_of::<&()>()
        + 12 * size_of::<usize>();
    // Growth pays the old and new allocations simultaneously; these are the
    // additional inline frames for each possible vector element type.
    let growth_frames = 2 * size_of::<Vec<Option<Recipe>>>()
        + 2 * size_of::<Vec<u8>>()
        + 2 * size_of::<Vec<(usize, u8)>>()
        + 2 * size_of::<Vec<usize>>()
        + size_of::<Result<Vec<Option<Recipe>>>>()
        + size_of::<Result<Vec<u8>>>()
        + size_of::<Result<Vec<(usize, u8)>>>()
        + size_of::<Result<Vec<usize>>>()
        + 4 * size_of::<Result<()>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>();
    let emission_frame = size_of::<&[Option<Recipe>]>()
        + size_of::<&[usize]>()
        + 2 * size_of::<&()>()
        + size_of::<Result<()>>();
    size_of::<Vec<Option<Recipe>>>()
        + size_of::<Vec<u8>>()
        + size_of::<Vec<(usize, u8)>>()
        + size_of::<Vec<usize>>()
        + 2 * size_of::<Result<Vec<Option<Recipe>>>>()
        + 2 * size_of::<Result<Vec<u8>>>()
        + 2 * size_of::<Result<Vec<(usize, u8)>>>()
        + 2 * size_of::<Result<Vec<usize>>>()
        + 4 * size_of::<Recipe>()
        + 4 * size_of::<Result<Recipe>>()
        + 4 * size_of::<Option<usize>>()
        + 3 * size_of::<Function>()
        + 3 * size_of::<Definition>()
        + 3 * size_of::<std::ops::Range<usize>>()
        + wrapper_refs
        + recipe_refs
        + operand_refs
        + traversal_borrows
        + loader_frame
        + pure_result_frame
        + snapshot_path
        + borrowed_descendants
        + frame_indices
        + diagnostic_results
        + control_install
        + growth_frames
        + emission_frame
        + 2 * size_of::<Type>()
        + 2 * size_of::<ScalarType>()
        + 2 * size_of::<ValueId>()
        + size_of::<FormalIndexWidth>()
        + 2 * size_of::<bool>()
        + size_of::<u8>()
        + size_of::<u64>()
        + 3 * size_of::<Result<()>>()
        + size_of::<std::slice::Iter<'_, usize>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::Budget;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
        Function as IrFunction, MemoryAccess, Module, Operation as Instruction, Signature,
        StorageLayoutLimitsV1, UnaryOp, ValueDef, VerifiedCanonicalKernelIrModuleV18 as Owner,
    };

    const LIMIT: usize = 100_000_000;
    const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
        rows: 64,
        edges: 256,
        containment_depth: 32,
        object_bytes: 4096,
    };

    pub(super) fn diamond(nonclosed: bool, extra_predecessor: bool) -> Module {
        let mut entry = BasicBlock::new(BlockId(100));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(300),
            then_arguments: vec![],
            else_target: BlockId(400),
            else_arguments: vec![],
        });
        let mut left = BasicBlock::new(BlockId(300));
        left.terminator = Some(if nonclosed {
            Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(500),
                then_arguments: vec![ValueId(0)],
                else_target: BlockId(600),
                else_arguments: vec![],
            }
        } else {
            Terminator::Branch {
                target: BlockId(500),
                arguments: vec![ValueId(0)],
            }
        });
        let mut right = BasicBlock::new(BlockId(400));
        right.terminator = Some(Terminator::Branch {
            target: BlockId(500),
            arguments: vec![ValueId(1)],
        });
        let mut merge = BasicBlock::new(BlockId(500));
        merge.parameters = vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))];
        merge.terminator = Some(Terminator::Return {
            values: vec![ValueId(4)],
        });
        let mut blocks = Vec::new();
        if extra_predecessor {
            let mut outer = BasicBlock::new(BlockId(50));
            outer.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(100),
                then_arguments: vec![],
                else_target: BlockId(200),
                else_arguments: vec![],
            });
            let mut extra = BasicBlock::new(BlockId(200));
            extra.terminator = Some(Terminator::Branch {
                target: BlockId(300),
                arguments: vec![],
            });
            blocks.extend([outer, extra]);
        }
        blocks.extend([entry, left, right, merge]);
        if nonclosed {
            let mut exit = BasicBlock::new(BlockId(600));
            exit.terminator = Some(Terminator::Return {
                values: vec![ValueId(0)],
            });
            blocks.push(exit);
        }
        let mut module = Module::new("scalar-reconstruction-diamond");
        module.functions.push(IrFunction::internal_helper(
            "entry",
            Signature::new(
                vec![
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::Bool),
                    Type::Scalar(ScalarType::Bool),
                ],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            blocks,
        ));
        module
    }

    #[test]
    fn reconstruction_control_regions_reject_side_exits_and_preserve_nested_conditions() {
        use control_reconstruction::{Term, derive};
        for (nonclosed, extra) in [(false, false), (true, false), (false, true)] {
            let module = diamond(nonclosed, extra);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let (owner, storage) =
                Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (input, storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let original = input
                .definitions()
                .iter()
                .position(|row| row.value == Some(ValueId(4)))
                .unwrap();
            budget
                .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)
                .unwrap();
            let mut out = Writer::new(&mut budget).unwrap();
            let result = derive(&input, original, &mut out);
            if nonclosed {
                let error = match result {
                    Ok(plan) => {
                        plan.discard(&mut out).unwrap();
                        panic!("side exit cannot form a complete region");
                    }
                    Err(error) => error,
                };
                let Error::SourceReconstruction { facts, .. } = error else {
                    panic!("control refusal must retain original coordinates");
                };
                assert_eq!(facts.original, original);
                assert_eq!(facts.coordinate, input.definitions()[original].coordinate);
                assert_eq!(facts.phase, "cfg-region");
            } else {
                let plan = result.unwrap();
                let index = |value| {
                    input
                        .definitions()
                        .iter()
                        .position(|row| row.value == Some(ValueId(value)))
                        .unwrap()
                };
                let Term::Control(root) = plan.root else {
                    panic!("distinct values require selection");
                };
                let selected = plan.controls[root];
                let inner = if extra {
                    assert_eq!(plan.controls.len(), 2);
                    assert_eq!(selected.condition, index(3));
                    assert_eq!(selected.on_false, Term::Original(index(0)));
                    let Term::Control(inner) = selected.on_true else {
                        panic!("outer true branch must select original inner condition");
                    };
                    plan.controls[inner]
                } else {
                    assert_eq!(plan.controls.len(), 1);
                    selected
                };
                assert_eq!(inner.condition, index(2));
                assert_eq!(inner.on_true, Term::Original(index(0)));
                assert_eq!(inner.on_false, Term::Original(index(1)));
                plan.discard(&mut out).unwrap();
            }
            assert!(out.finish().unwrap().is_empty());
        }
    }

    #[test]
    fn reconstruction_traversal_rejects_cycles_and_out_of_range_dependencies() {
        for (root, child, refused) in [
            (Recipe::Forward(1), Recipe::Actual(0), None),
            (
                Recipe::Forward(1),
                Recipe::Forward(0),
                Some(("traversal-cycle", (1, 0, 0))),
            ),
            (
                Recipe::Forward(0),
                Recipe::Actual(0),
                Some(("traversal-cycle", (0, 0, 0))),
            ),
            (
                Recipe::Forward(2),
                Recipe::Actual(0),
                Some(("traversal-bounds", (0, 2, 0))),
            ),
        ] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut out = Writer::new(&mut budget).unwrap();
            let mut recipes = vec![Some(root), None];
            let mut marks = vec![1, 0];
            let mut stack = vec![(0, 0)];
            let mut order = Vec::new();
            let result = traverse(
                &mut recipes,
                &mut marks,
                &mut stack,
                &mut order,
                synthetic_facts(0),
                None,
                &mut out,
                &mut |index, _| {
                    assert_eq!(index, 1);
                    Ok(child)
                },
            );
            if let Some((phase, dependency)) = refused {
                let Error::SourceReconstruction { facts, .. } = result.unwrap_err() else {
                    panic!("synthetic traversal refusal must retain its numeric edge");
                };
                assert_eq!(facts.phase, phase);
                assert_eq!(facts.original, 0);
                assert_eq!(facts.dependency, Some(dependency));
                assert!(order.is_empty());
            } else {
                result.unwrap();
                assert_eq!(order, [1, 0]);
                assert_eq!(marks, [2, 2]);
                assert!(stack.is_empty());
            }
            assert!(out.finish().unwrap().is_empty());
        }
    }

    fn synthetic_facts(original: usize) -> RefusalFacts {
        // Numeric error-path tests only, not an admitted source value.
        RefusalFacts::new(
            original,
            Definition::FunctionArgument {
                function: Function(0),
                argument: original as u32,
            },
            &Type::Scalar(ScalarType::U32),
        )
    }

    #[test]
    fn reconstruction_child_refusal_keeps_first_coordinates_and_traversal_edge() {
        for root in [
            Recipe::Forward(1),
            Recipe::Select {
                condition: 1,
                on_true: 0,
                on_false: 0,
            },
        ] {
            for kind in 0..5 {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                let mut out = Writer::new(&mut budget).unwrap();
                let mut recipes = vec![Some(root), None];
                let mut marks = vec![1, 0];
                let mut stack = vec![(0, 0)];
                let mut order = Vec::new();
                let mut expected = synthetic_facts(0);
                expected.target_function = Some(Function(9));
                expected.target_range = Some((10, 12));
                let error = traverse(
                    &mut recipes,
                    &mut marks,
                    &mut stack,
                    &mut order,
                    expected,
                    None,
                    &mut out,
                    &mut |index, _| {
                        assert_eq!(index, 1);
                        let mut first = synthetic_facts(index);
                        first.target_index = Some(17);
                        Err(match kind {
                            0 => trace_refusal(mismatch(), RefusalPhase::DependencyOwner, first),
                            1 => Error::Resource(Resource::Accounting),
                            2 => Error::Source(
                                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                                    Resource::Accounting,
                                ),
                            ),
                            3 => Error::Statement("generated source limit"),
                            _ => Error::GeneratedSourceLimit {
                                section: "child",
                                emitted_bytes: 17,
                                limit_bytes: 19,
                            },
                        })
                    },
                )
                .unwrap_err();
                match kind {
                    0 => {
                        let Error::SourceReconstruction { facts, .. } = error else {
                            panic!("first child facts lost")
                        };
                        assert_eq!(facts.original, 1);
                        assert_eq!(facts.coordinate, synthetic_facts(1).coordinate);
                        assert_eq!(facts.phase, "dependency-owner");
                        assert_eq!(facts.target_index, Some(17));
                        assert_eq!(facts.target_function, expected.target_function);
                        assert_eq!(facts.target_range, expected.target_range);
                        assert_eq!(facts.dependency, Some((0, 1, 0)));
                    }
                    1 => assert!(matches!(error, Error::Resource(Resource::Accounting))),
                    2 => assert!(matches!(
                        error,
                        Error::Source(
                            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                                Resource::Accounting
                            )
                        )
                    )),
                    3 => assert!(matches!(error, Error::Statement("generated source limit"))),
                    _ => assert!(matches!(
                        error,
                        Error::GeneratedSourceLimit {
                            section: "child",
                            emitted_bytes: 17,
                            limit_bytes: 19
                        }
                    )),
                }
                assert!(order.is_empty());
                assert!(out.finish().unwrap().is_empty());
            }
        }
    }

    #[test]
    fn reconstruction_pure_results_require_exact_constants_and_u32_addition() {
        // Canonical-inventory equation selection only, not a source-erasure or
        // source-to-target correspondence witness.
        let word = Type::Scalar(ScalarType::U32);
        let wide = Type::Scalar(ScalarType::U64);
        let pointer = Type::pointer(word.clone(), AddressSpace::Global, AccessMode::ReadOnly);
        let mut entry = BasicBlock::new(BlockId(0));
        for (id, value) in [
            (3, Constant::U32(u32::MAX)),
            (4, Constant::Bool(false)),
            (5, Constant::Bool(true)),
            (6, Constant::U32(0)),
            (7, Constant::U64(1)),
        ] {
            entry.operations.push(Instruction::effect_free(
                ValueDef::new(ValueId(id), value.ty()),
                OperationKind::Constant(value),
            ));
        }
        for (id, ty, binary, lhs, rhs) in [
            (8, word.clone(), BinaryOp::Add, 0, 3),
            (
                9,
                word.clone(),
                BinaryOp::Checked(CheckedBinaryOperator::Add),
                0,
                3,
            ),
            (11, word.clone(), BinaryOp::Subtract, 0, 1),
            (12, wide.clone(), BinaryOp::Add, 2, 7),
        ] {
            let mut results = vec![ValueDef::new(ValueId(id), ty)];
            if matches!(binary, BinaryOp::Checked(_)) {
                results.push(ValueDef::new(
                    ValueId(id + 1),
                    Type::Scalar(ScalarType::Bool),
                ));
            }
            entry.operations.push(Instruction::new(
                results,
                OperationKind::Binary {
                    op: binary,
                    lhs: ValueId(lhs),
                    rhs: ValueId(rhs),
                },
            ));
        }
        entry.operations.push(Instruction::effect_free(
            ValueDef::new(ValueId(13), word.clone()),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(0),
            },
        ));
        entry.operations.push(Instruction::new(
            vec![ValueDef::new(ValueId(15), word.clone())],
            OperationKind::Load {
                pointer: ValueId(14),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        entry.terminator = Some(Terminator::Return {
            values: vec![ValueId(8)],
        });
        let mut module = Module::new("pure-reconstruction-selection");
        module.functions.push(IrFunction::internal_helper(
            "entry",
            Signature::new(vec![word.clone(), word.clone(), wide, pointer], vec![word]),
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(14)],
            vec![entry],
        ));
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(headers()).unwrap();
        let (owner, storage) =
            Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (input, storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let index = |value| {
            input
                .definitions()
                .iter()
                .position(|row| row.value == Some(ValueId(value)))
                .unwrap()
        };
        let mut out = Writer::new(&mut budget).unwrap();
        let floor = out.budget.storage();
        for (value, bits) in [(3, u32::MAX), (4, 0), (5, 1), (6, 0)] {
            assert!(
                matches!(pure_result_recipe(&input, index(value), &mut out).unwrap(),
                Recipe::Literal(actual) if actual == bits)
            );
        }
        for (value, overflow) in [(8, false), (9, false), (10, true)] {
            assert!(
                matches!(pure_result_recipe(&input, index(value), &mut out).unwrap(),
                Recipe::CheckedAdd { left, right, overflow: actual }
                    if left == index(0) && right == index(3) && actual == overflow)
            );
        }
        let Definition::Result { operation, .. } = input.definitions()[index(15)].coordinate else {
            panic!("the admitted load has one real result");
        };
        assert!(
            !input.operations()[operation_index(&input, operation).unwrap()]
                .effects
                .is_empty()
        );
        for value in [0, 7, 11, 12, 13, 15] {
            assert!(matches!(
                pure_result_recipe(&input, index(value), &mut out),
                Err(Error::Statement(_))
            ));
        }
        assert!(matches!(
            pure_result_recipe(&input, input.definitions().len(), &mut out),
            Err(Error::Statement(_))
        ));
        assert_eq!(out.budget.storage(), floor);
        assert!(out.finish().unwrap().is_empty());
    }
}

fn function(coordinate: Definition) -> Function {
    match coordinate {
        Definition::FunctionArgument { function, .. } => function,
        Definition::BlockArgument { block, .. } => block.function,
        Definition::Result { operation, .. } => operation.block.function,
    }
}

fn emit_nodes(
    input: &Inventory<'_>,
    recipes: &[Option<Recipe>],
    order: &[usize],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for &node in order {
        out.budget.charge_work(2)?;
        match recipes[node].ok_or_else(mismatch)? {
            Recipe::Region(_) => return Err(mismatch()),
            Recipe::Literal(bits) => {
                emit!(
                    out,
                    "let reconstructed_{node} = MemoryValueV30::Scalar({bits}int); let reconstructed_ok_{node} = true; "
                );
            }
            Recipe::Actual(index) => {
                let limit = if input.definitions()[node].ty == &Type::Scalar(ScalarType::Bool) {
                    2u64
                } else {
                    1u64 << 32
                };
                emit!(
                    out,
                    "let reconstructed_{node} = target.values[{index}]; let reconstructed_ok_{node} = (match reconstructed_{node} {{ MemoryValueV30::Scalar(bits) => 0 <= bits < {limit}, _ => false }}); "
                );
            }
            Recipe::Forward(child) => {
                emit!(
                    out,
                    "let reconstructed_{node} = reconstructed_{child}; let reconstructed_ok_{node} = reconstructed_ok_{child}; "
                );
            }
            Recipe::CheckedAdd {
                left,
                right,
                overflow,
            } => {
                emit!(
                    out,
                    "let reconstructed_sum_{node} = (match reconstructed_{left} {{ MemoryValueV30::Scalar(bits) => bits, _ => 0int }}) + (match reconstructed_{right} {{ MemoryValueV30::Scalar(bits) => bits, _ => 0int }}); let reconstructed_ok_{node} = reconstructed_ok_{left} && reconstructed_ok_{right}; let reconstructed_{node} = MemoryValueV30::Scalar("
                );
                out.budget.charge_work(1)?;
                if overflow {
                    emit!(
                        out,
                        "if reconstructed_sum_{node} >= 4294967296 {{ 1int }} else {{ 0int }}"
                    );
                } else {
                    emit!(out, "reconstructed_sum_{node} % 4294967296");
                }
                emit!(out, "); ");
            }
            Recipe::Select {
                condition,
                on_true,
                on_false,
            } => {
                emit!(
                    out,
                    "let reconstructed_branch_{node} = reconstructed_{condition} == MemoryValueV30::Scalar(1); let reconstructed_{node} = if reconstructed_branch_{node} {{ reconstructed_{on_true} }} else {{ reconstructed_{on_false} }}; let reconstructed_ok_{node} = reconstructed_ok_{condition} && (if reconstructed_branch_{node} {{ reconstructed_ok_{on_true} }} else {{ reconstructed_ok_{on_false} }}); "
                );
            }
        }
    }
    Ok(())
}

fn grow<T: Copy>(values: &mut Vec<T>, needed: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(2)?;
    if needed <= values.capacity() {
        return Ok(());
    }
    let count = needed.max(
        values
            .capacity()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
    );
    let mut next = vector(count, out)?;
    out.budget.charge_work(values.len())?;
    next.extend_from_slice(values);
    let old = std::mem::replace(values, next);
    let released = old
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    drop(old);
    out.budget.release_storage(released)?;
    Ok(())
}

fn install_region(
    input: &Inventory<'_>,
    original: usize,
    recipes: &mut Vec<Option<Recipe>>,
    marks: &mut Vec<u8>,
    stack: &mut Vec<(usize, u8)>,
    order: &mut Vec<usize>,
    out: &mut Writer<'_, '_>,
) -> Result<Recipe> {
    use control_reconstruction::{Select, Term};
    let plan = control_reconstruction::derive(input, original, out)?;
    plan.check(input, out)?;
    let base = recipes.len();
    let root = match plan.root {
        Term::Original(value) => Recipe::Forward(value),
        Term::Control(root) => {
            let selected = *plan.controls.get(root).ok_or_else(mismatch)?;
            let end = base.checked_add(root).ok_or(Resource::Arithmetic)?;
            grow(recipes, end, out)?;
            grow(marks, end, out)?;
            grow(stack, end, out)?;
            grow(order, end, out)?;
            let term = |term: Term, before: usize| -> Result<usize> {
                match term {
                    Term::Original(value) if value < input.definitions().len() => Ok(value),
                    Term::Control(control) if control < before => base
                        .checked_add(control)
                        .ok_or_else(|| Resource::Arithmetic.into()),
                    _ => Err(mismatch()),
                }
            };
            let recipe = |node: Select, before: usize| -> Result<Recipe> {
                if node.condition >= input.definitions().len() {
                    return Err(mismatch());
                }
                Ok(Recipe::Select {
                    condition: node.condition,
                    on_true: term(node.on_true, before)?,
                    on_false: term(node.on_false, before)?,
                })
            };
            for (ordinal, node) in plan.controls[..root].iter().copied().enumerate() {
                out.budget.charge_work(6)?;
                recipes.push(Some(recipe(node, ordinal)?));
                marks.push(0);
            }
            out.budget.charge_work(6)?;
            recipe(selected, root)?
        }
    };
    plan.discard(out)?;
    Ok(root)
}

fn traverse(
    recipes: &mut Vec<Option<Recipe>>,
    marks: &mut Vec<u8>,
    stack: &mut Vec<(usize, u8)>,
    order: &mut Vec<usize>,
    mut facts: RefusalFacts,
    input: Option<&Inventory<'_>>,
    out: &mut Writer<'_, '_>,
    load: &mut impl FnMut(usize, &mut Writer<'_, '_>) -> Result<Recipe>,
) -> Result<()> {
    let mut phase = RefusalPhase::TraversalBounds;
    let result = (|| {
        out.budget.charge_work(2)?;
        if marks.len() != recipes.len() {
            return Err(mismatch());
        }
        while let Some(&(current, ordinal)) = stack.last() {
            let original_count = input.map_or(usize::MAX, |input| input.definitions().len());
            facts.dependency = (current < original_count).then_some((current, current, ordinal));
            out.budget.charge_work(8)?;
            let mut recipe = recipes
                .get(current)
                .copied()
                .flatten()
                .ok_or_else(mismatch)?;
            if let Recipe::Region(original) = recipe {
                if original != current || current >= original_count {
                    return Err(mismatch());
                }
                recipe = install_region(
                    input.ok_or_else(mismatch)?,
                    original,
                    recipes,
                    marks,
                    stack,
                    order,
                    out,
                )?;
                recipes[current] = Some(recipe);
            }
            if let Some(child) = recipe.child(ordinal) {
                facts.dependency = (current < original_count && child < original_count)
                    .then_some((current, child, ordinal));
                stack.last_mut().ok_or_else(mismatch)?.1 += 1;
                if *marks.get(child).ok_or_else(mismatch)? == 1 {
                    phase = RefusalPhase::TraversalCycle;
                    return Err(mismatch());
                }
                if marks[child] == 0 {
                    out.budget.charge_work(3)?;
                    if stack.len() >= recipes.len() {
                        return Err(mismatch());
                    }
                    if recipes[child].is_none() {
                        if child >= original_count {
                            return Err(mismatch());
                        }
                        recipes[child] = Some(load(child, out)?);
                    }
                    marks[child] = 1;
                    stack.push((child, 0));
                }
            } else {
                out.budget.charge_work(3)?;
                if order.len() >= recipes.len() {
                    return Err(mismatch());
                }
                marks[current] = 2;
                order.push(current);
                stack.pop();
            }
        }
        Ok(())
    })();
    result.map_err(|error| trace_refusal(error, phase, facts))
}

fn operand(
    input: &Inventory<'_>,
    owner: Function,
    value: ValueId,
    ty: &Type,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let index = input
        .definition_index_for_value(owner, value, out.budget)?
        .ok_or_else(mismatch)?;
    out.budget.charge_work(3)?;
    let row = input.definitions().get(index).ok_or_else(mismatch)?;
    if function(row.coordinate) != owner || row.value != Some(value) || row.ty != ty {
        return Err(mismatch());
    }
    Ok(index)
}

// This selects an equation from the original inventory only. The caller still
// authenticates erasure, root ownership and every retained dependency.
fn pure_result_recipe(
    input: &Inventory<'_>,
    original: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Recipe> {
    out.budget.charge_work(14)?;
    let row = input.definitions().get(original).ok_or_else(mismatch)?;
    let Definition::Result { operation, result } = row.coordinate else {
        return Err(mismatch());
    };
    let op = &input.operations()[operation_index(input, operation)?];
    if op.results.start.checked_add(result as usize) != Some(original)
        || !op.results.contains(&original)
        || op.operation.results.len() != op.results.len()
        || !op.effects.is_empty()
        || op
            .operation
            .results
            .get(result as usize)
            .map(|result| &result.ty)
            != Some(row.ty)
    {
        return Err(mismatch());
    }
    if let OperationKind::Constant(value) = &op.operation.kind {
        if result != 0 || op.results.len() != 1 || &value.ty() != row.ty {
            return Err(mismatch());
        }
        return match value {
            Constant::U32(value) => Ok(Recipe::Literal(*value)),
            Constant::Bool(value) => Ok(Recipe::Literal(u32::from(*value))),
            _ => Err(mismatch()),
        };
    }
    let OperationKind::Binary {
        op: binary,
        lhs,
        rhs,
    } = &op.operation.kind
    else {
        return Err(mismatch());
    };
    let overflow = match binary {
        BinaryOp::Add
            if result == 0 && op.results.len() == 1 && row.ty == &Type::Scalar(ScalarType::U32) =>
        {
            false
        }
        BinaryOp::Checked(CheckedBinaryOperator::Add)
            if result <= 1
                && op.results.len() == 2
                && op.operation.results[0].ty == Type::Scalar(ScalarType::U32)
                && op.operation.results[1].ty == Type::Scalar(ScalarType::Bool) =>
        {
            result == 1
        }
        _ => return Err(mismatch()),
    };
    Ok(Recipe::CheckedAdd {
        left: operand(
            input,
            operation.block.function,
            *lhs,
            &Type::Scalar(ScalarType::U32),
            out,
        )?,
        right: operand(
            input,
            operation.block.function,
            *rhs,
            &Type::Scalar(ScalarType::U32),
            out,
        )?,
        overflow,
    })
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    fn reconstruction_recipe(
        &self,
        input: &Inventory<'_>,
        original: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Recipe> {
        out.budget.charge_work(2)?;
        let row = input.definitions().get(original).ok_or_else(mismatch)?;
        if !matches!(row.ty, Type::Scalar(ScalarType::U32 | ScalarType::Bool)) {
            return Err(mismatch());
        }
        if let Definition::Result { operation, result } = row.coordinate {
            out.budget.charge_work(8)?;
            let op = &input.operations()[operation_index(input, operation)?];
            if op.results.start.checked_add(result as usize) != Some(original)
                || !op.results.contains(&original)
            {
                return Err(mismatch());
            }
            if let OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                fragment,
                lanes,
                elements,
            }) = &op.operation.kind
            {
                out.budget.charge_work(6)?;
                let elements = usize::from(*elements);
                if elements == 0
                    || op.results.len() != elements.checked_mul(2).ok_or(Resource::Arithmetic)?
                    || op.operation.results.len() != op.results.len()
                {
                    return Err(mismatch());
                }
                let role = Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
                    lanes: *lanes,
                    elements: u16::try_from(elements).map_err(|_| Resource::Arithmetic)?,
                });
                let fragment = operand(input, operation.block.function, *fragment, &role, out)?;
                let field = result as usize / elements;
                let element = result as usize % elements;
                let expected = if field == 0 {
                    ScalarType::U32
                } else {
                    ScalarType::Bool
                };
                if field > 1 || row.ty != &Type::Scalar(expected) {
                    return Err(mismatch());
                }
                let actual = self
                    .tile_leaf(fragment, &[field as u32, element as u32], out)?
                    .ok_or_else(mismatch)?;
                return Ok(Recipe::Actual(actual));
            }
        }
        let tile = self.slots.tile_owner_v176(out)?;
        let neutral = tile.neutral_source_v162(out.budget)?;
        let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
        out.budget.charge_work(5)?;
        let mut facts =
            RefusalFacts::new(original, row.coordinate, row.ty).descendants(descendants);
        if !descendants.is_empty() {
            return self
                .source_definition(original, out)
                .map(Recipe::Actual)
                .map_err(|error| trace_refusal(error, RefusalPhase::RecipeDescendants, facts));
        }
        match row.coordinate {
            Definition::BlockArgument { .. } => {
                control_reconstruction::identity(input, original, out)
                    .map(|identity| identity.map_or(Recipe::Region(original), Recipe::Forward))
                    .map_err(|error| trace_refusal(error, RefusalPhase::PhiIncoming, facts))
            }
            Definition::Result { operation, .. } => {
                out.budget.charge_work(2)?;
                let op = &input.operations()[operation_index(input, operation)?];
                if let OperationKind::Binary { op, .. } = &op.operation.kind {
                    facts.binary = Some(*op);
                }
                pure_result_recipe(input, original, out)
                    .map_err(|error| trace_refusal(error, RefusalPhase::ErasedRecipe, facts))
            }
            _ => Err(trace_refusal(mismatch(), RefusalPhase::ErasedRecipe, facts)),
        }
    }

    pub(super) fn emit_source_original_relation(
        &self,
        root: usize,
        original: Option<usize>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_source_original_relation_observed(root, original, width, out, None)
    }

    pub(super) fn emit_source_original_relation_observed(
        &self,
        root: usize,
        original: Option<usize>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
        observe: Option<&mut super::super::super::forwarding_observation::Observer<'_>>,
    ) -> Result<()> {
        self.check(out)?;
        out.budget.charge_work(2)?;
        if width == FormalIndexWidth::Unknown {
            return Err(mismatch());
        }
        let Some(original) = original else {
            return self.emit_actual_relation(None, width, out);
        };
        let input = self.slots.correspondence(out)?.inventory(out.budget)?;
        let source = input.definitions().get(original).ok_or_else(mismatch)?;
        let actual_owner = self.target.root_function(root, out)?;
        let actual = self.target.inventory(out)?;
        let actual_function = actual
            .functions()
            .get(actual_owner.0 as usize)
            .ok_or_else(mismatch)?;
        out.budget.charge_work(7)?;
        let mut facts = RefusalFacts::new(original, source.coordinate, source.ty);
        facts.target_function = Some(actual_owner);
        facts.target_range = Some((
            actual_function.definitions.start,
            actual_function.definitions.end,
        ));
        if !matches!(source.ty, Type::Scalar(ScalarType::U32 | ScalarType::Bool)) {
            let mut ignore =
                |_: super::super::super::forwarding_observation::ExpandedSupportForwardingV288,
                 _: &mut Writer<'_, '_>|
                 -> Result<()> { Ok(()) };
            let observer = observe.unwrap_or(&mut ignore);
            let index = self
                .source_transport_definition_observed(original, out, observer)
                .map_err(|error| trace_refusal(error, RefusalPhase::TargetLookup, facts))?;
            facts.target_index = Some(index);
            out.budget.charge_work(1)?;
            if !actual_function.definitions.contains(&index) {
                return Err(trace_refusal(mismatch(), RefusalPhase::TargetOwner, facts));
            }
            observer(super::super::super::forwarding_observation::ExpandedSupportForwardingV288::Target {
                actual: index, coordinate: actual.definitions().get(index).ok_or_else(mismatch)?.coordinate,
                function: actual_owner,
            }, out)?;
            return self.emit_actual_relation(Some(index), width, out);
        }
        let first = self
            .reconstruction_recipe(input, original, out)
            .map_err(|error| trace_refusal(error, RefusalPhase::TargetLookup, facts))?;
        if let Recipe::Actual(index) = first {
            facts.target_index = Some(index);
            out.budget.charge_work(2)?;
            if !actual_function.definitions.contains(&index)
                || actual.definitions()[index].ty != source.ty
            {
                return Err(trace_refusal(mismatch(), RefusalPhase::TargetOwner, facts));
            }
            return self.emit_actual_relation(Some(index), width, out);
        }
        let floor = out.budget.storage();
        let count = input.definitions().len();
        let mut recipes = vector(count, out)?;
        let mut marks = vector(count, out)?;
        let mut stack = vector(count, out)?;
        let mut order = vector(count, out)?;
        out.budget
            .charge_work(count.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        recipes.resize(count, None);
        marks.resize(count, 0u8);
        recipes[original] = Some(first);
        marks[original] = 1;
        stack.push((original, 0u8));
        traverse(
            &mut recipes,
            &mut marks,
            &mut stack,
            &mut order,
            facts,
            Some(input),
            out,
            &mut |child, out| {
                out.budget.charge_work(15)?;
                let child_row = input.definitions().get(child).ok_or_else(mismatch)?;
                let mut child_facts = RefusalFacts::new(child, child_row.coordinate, child_row.ty);
                child_facts.target_function = facts.target_function;
                child_facts.target_range = facts.target_range;
                if function(child_row.coordinate) != function(source.coordinate) {
                    return Err(trace_refusal(
                        mismatch(),
                        RefusalPhase::DependencyOwner,
                        child_facts,
                    ));
                }
                let recipe = self
                    .reconstruction_recipe(input, child, out)
                    .map_err(|error| {
                        trace_refusal(error, RefusalPhase::DependencyRecipe, child_facts)
                    })?;
                if let Recipe::Actual(index) = recipe {
                    child_facts.target_index = Some(index);
                    if !actual_function.definitions.contains(&index)
                        || actual.definitions().get(index).map(|row| row.ty) != Some(child_row.ty)
                    {
                        return Err(trace_refusal(
                            mismatch(),
                            RefusalPhase::DependencyOwner,
                            child_facts,
                        ));
                    }
                }
                Ok(recipe)
            },
        )?;
        let storage = out
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        emit!(
            out,
            "(target.values.len() == {} && ({{ ",
            self.actual_definitions
        );
        emit_nodes(input, &recipes, &order, out)?;
        emit!(
            out,
            "reconstructed_ok_{original} && invocation_value_related_v36(original, reconstructed_{original}, map, source.machine.memory, target.memory) }}))"
        );
        drop(order);
        drop(stack);
        drop(marks);
        drop(recipes);
        out.budget.release_storage(storage)?;
        Ok(())
    }
}
