//! Necessary scalar equations over retained snapshots, not transition authority.
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::original_scalar_v30::vector;
use crate::mixed_optimizer_refinement_v26::semantics::{block_index, operation_index};
use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CheckedBinaryOperator, ExecutionOperationV15 as Execution, ExecutionRoleV15, FormalIndexWidth,
    OperationKind, ScalarType, Terminator, ValueId,
};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Recipe {
    Actual(usize),
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
    let phi_refs = 9 * size_of::<&()>();
    let operand_refs = 4 * size_of::<&()>();
    let traversal_borrows =
        size_of::<&mut [Option<Recipe>]>() + size_of::<&mut [u8]>() + 4 * size_of::<&()>();
    let loader_frame = (5 + 2) * size_of::<&()>()
        + size_of::<usize>()
        + size_of::<Recipe>()
        + size_of::<Result<Recipe>>();
    let arm_iterator = size_of::<std::array::IntoIter<Block, 2>>();
    let snapshot_path = size_of::<[u32; 2]>() + size_of::<&[u32]>();
    let borrowed_rosters = size_of::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>()
        + size_of::<&[fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1]>()
        + size_of::<&[fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>]>();
    let frame_indices = (14 + 8 + 8 + 2) * size_of::<usize>();
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
        + 4 * size_of::<Option<Block>>()
        + 4 * size_of::<Block>()
        + 3 * size_of::<Function>()
        + 3 * size_of::<Definition>()
        + 3 * size_of::<std::ops::Range<usize>>()
        + size_of::<[Option<(Block, usize)>; 2]>()
        + wrapper_refs
        + recipe_refs
        + phi_refs
        + operand_refs
        + traversal_borrows
        + loader_frame
        + arm_iterator
        + snapshot_path
        + borrowed_rosters
        + frame_indices
        + 2 * size_of::<Type>()
        + 2 * size_of::<ScalarType>()
        + 2 * size_of::<ValueId>()
        + size_of::<FormalIndexWidth>()
        + size_of::<[bool; 2]>()
        + 2 * size_of::<bool>()
        + size_of::<u8>()
        + size_of::<u64>()
        + 3 * size_of::<Result<()>>()
        + 3 * size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>>()
        + size_of::<std::slice::Iter<'_, usize>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::Budget;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function as IrFunction, Module,
        Signature, StorageLayoutLimitsV1, ValueDef, VerifiedCanonicalKernelIrModuleV18 as Owner,
    };

    const LIMIT: usize = 100_000_000;
    const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
        rows: 64,
        edges: 256,
        containment_depth: 32,
        object_bytes: 4096,
    };

    fn diamond(nonclosed: bool, extra_predecessor: bool) -> Module {
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
    fn reconstruction_rejects_nonclosed_and_extra_predecessor_diamonds() {
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
            let Definition::BlockArgument { block, .. } = input.definitions()[original].coordinate
            else {
                panic!("fixture merge argument must retain its original coordinate");
            };
            let mut out = Writer::new(&mut budget).unwrap();
            let result = phi(&input, original, block, &mut out);
            if nonclosed || extra {
                assert!(matches!(result, Err(Error::Statement(_))));
            } else {
                let Recipe::Select {
                    condition,
                    on_true,
                    on_false,
                } = result.unwrap()
                else {
                    panic!("closed fixture must use both distinct original values");
                };
                assert_eq!(input.definitions()[condition].value, Some(ValueId(2)));
                assert_eq!(input.definitions()[on_true].value, Some(ValueId(0)));
                assert_eq!(input.definitions()[on_false].value, Some(ValueId(1)));
            }
            assert!(out.finish().unwrap().is_empty());
        }
    }

    #[test]
    fn reconstruction_traversal_rejects_cycles_and_out_of_range_dependencies() {
        for (root, child, accepted) in [
            (Recipe::Forward(1), Recipe::Actual(0), true),
            (Recipe::Forward(1), Recipe::Forward(0), false),
            (Recipe::Forward(0), Recipe::Actual(0), false),
            (Recipe::Forward(2), Recipe::Actual(0), false),
        ] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut out = Writer::new(&mut budget).unwrap();
            let mut recipes = [Some(root), None];
            let mut marks = [1, 0];
            let mut stack = vec![(0, 0)];
            let mut order = Vec::new();
            let result = traverse(
                &mut recipes,
                &mut marks,
                &mut stack,
                &mut order,
                &mut out,
                &mut |index, _| {
                    assert_eq!(index, 1);
                    Ok(child)
                },
            );
            if accepted {
                result.unwrap();
                assert_eq!(order, [1, 0]);
                assert_eq!(marks, [2, 2]);
                assert!(stack.is_empty());
            } else {
                assert!(matches!(result, Err(Error::Statement(_))));
                assert!(order.is_empty());
            }
            assert!(out.finish().unwrap().is_empty());
        }
    }
}

fn function(coordinate: Definition) -> Function {
    match coordinate {
        Definition::FunctionArgument { function, .. } => function,
        Definition::BlockArgument { block, .. } => block.function,
        Definition::Result { operation, .. } => operation.block.function,
    }
}

fn traverse(
    recipes: &mut [Option<Recipe>],
    marks: &mut [u8],
    stack: &mut Vec<(usize, u8)>,
    order: &mut Vec<usize>,
    out: &mut Writer<'_, '_>,
    load: &mut impl FnMut(usize, &mut Writer<'_, '_>) -> Result<Recipe>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    if marks.len() != recipes.len() {
        return Err(mismatch());
    }
    while let Some(&(current, ordinal)) = stack.last() {
        out.budget.charge_work(5)?;
        let recipe = recipes
            .get(current)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        if let Some(child) = recipe.child(ordinal) {
            stack.last_mut().ok_or_else(mismatch)?.1 += 1;
            if *marks.get(child).ok_or_else(mismatch)? == 1 {
                return Err(mismatch());
            }
            if marks[child] == 0 {
                out.budget.charge_work(3)?;
                if stack.len() >= recipes.len() {
                    return Err(mismatch());
                }
                recipes[child] = Some(load(child, out)?);
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

// Only an exact two-arm diamond is accepted. Each arm has one incoming edge,
// and its sole outgoing edge carries the selected original phi argument.
fn phi(
    input: &Inventory<'_>,
    original: usize,
    block: Block,
    out: &mut Writer<'_, '_>,
) -> Result<Recipe> {
    out.budget.charge_work(4)?;
    let owner = input
        .functions()
        .get(block.function.0 as usize)
        .ok_or_else(mismatch)?;
    let row = input.definitions().get(original).ok_or_else(mismatch)?;
    if owner.coordinate != block.function || !owner.definitions.contains(&original) {
        return Err(mismatch());
    }
    let mut arms = [None; 2];
    let mut count = 0usize;
    let mut distinct = false;
    for edge in input
        .edge_arguments()
        .get(owner.edge_arguments.clone())
        .ok_or_else(mismatch)?
    {
        out.budget.charge_work(1)?;
        if edge.target_definition != original {
            continue;
        }
        out.budget.charge_work(5)?;
        if edge.coordinate.edge.source.function != block.function
            || !owner.definitions.contains(&edge.incoming_definition)
        {
            return Err(mismatch());
        }
        let incoming = input
            .definitions()
            .get(edge.incoming_definition)
            .ok_or_else(mismatch)?;
        if incoming.value != Some(edge.value) || incoming.ty != row.ty {
            return Err(mismatch());
        }
        if let Some((_, first)) = arms[0] {
            distinct |= first != edge.incoming_definition;
        }
        if count >= arms.len() {
            if distinct {
                return Err(mismatch());
            }
        } else {
            arms[count] = Some((edge.coordinate.edge.source, edge.incoming_definition));
        }
        count += 1;
    }
    out.budget.charge_work(3)?;
    let (first, first_value) = arms[0].ok_or_else(mismatch)?;
    if !distinct {
        return Ok(Recipe::Forward(first_value));
    }
    let (second, second_value) = arms[1].ok_or_else(mismatch)?;
    if first_value == second_value {
        return Ok(Recipe::Forward(first_value));
    }
    if first == second || first == block || second == block {
        return Err(mismatch());
    }
    for arm in [first, second] {
        out.budget.charge_work(6)?;
        let arm = &input.blocks()[block_index(input, arm)?];
        let [edge] = input.edges().get(arm.edges.clone()).ok_or_else(mismatch)? else {
            return Err(mismatch());
        };
        if !arm.parameters.is_empty()
            || !matches!(arm.terminator, Terminator::Branch { .. })
            || edge.target != block
        {
            return Err(mismatch());
        }
    }
    let mut common = None;
    let mut seen = [false; 2];
    let mut on_true = None;
    let mut on_false = None;
    for edge in input
        .edges()
        .get(owner.edges.clone())
        .ok_or_else(mismatch)?
    {
        out.budget.charge_work(2)?;
        let arm = if edge.target == first {
            0
        } else if edge.target == second {
            1
        } else {
            continue;
        };
        out.budget.charge_work(5)?;
        if seen[arm]
            || !edge.arguments.is_empty()
            || common.is_some_and(|prior| prior != edge.coordinate.source)
        {
            return Err(mismatch());
        }
        seen[arm] = true;
        common = Some(edge.coordinate.source);
        let value = if arm == 0 { first_value } else { second_value };
        match edge.coordinate.successor {
            0 if on_true.is_none() => on_true = Some(value),
            1 if on_false.is_none() => on_false = Some(value),
            _ => return Err(mismatch()),
        }
    }
    out.budget.charge_work(6)?;
    let common = common.ok_or_else(mismatch)?;
    if !seen[0] || !seen[1] || common == block || common == first || common == second {
        return Err(mismatch());
    }
    let branch = &input.blocks()[block_index(input, common)?];
    let Terminator::ConditionalBranch { condition, .. } = branch.terminator else {
        return Err(mismatch());
    };
    if branch.edges.len() != 2 {
        return Err(mismatch());
    }
    Ok(Recipe::Select {
        condition: operand(
            input,
            block.function,
            *condition,
            &Type::Scalar(ScalarType::Bool),
            out,
        )?,
        on_true: on_true.ok_or_else(mismatch)?,
        on_false: on_false.ok_or_else(mismatch)?,
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
        out.budget.charge_work(1)?;
        if !descendants.is_empty() {
            return Ok(Recipe::Actual(self.source_definition(original, out)?));
        }
        match row.coordinate {
            Definition::BlockArgument { block, .. } => phi(input, original, block, out),
            Definition::Result { operation, result } => {
                out.budget.charge_work(10)?;
                let op = &input.operations()[operation_index(input, operation)?];
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = &op.operation.kind
                else {
                    return Err(mismatch());
                };
                if result > 1
                    || op.results.len() != 2
                    || !op.effects.is_empty()
                    || op.operation.results.len() != 2
                    || op.operation.results[0].ty != Type::Scalar(ScalarType::U32)
                    || op.operation.results[1].ty != Type::Scalar(ScalarType::Bool)
                    || row.ty != &op.operation.results[result as usize].ty
                {
                    return Err(mismatch());
                }
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
                    overflow: result == 1,
                })
            }
            _ => Err(mismatch()),
        }
    }

    pub(super) fn emit_source_original_relation(
        &self,
        root: usize,
        original: Option<usize>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
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
        out.budget.charge_work(3)?;
        if !matches!(source.ty, Type::Scalar(ScalarType::U32 | ScalarType::Bool)) {
            let index = self.source_transport_definition(original, out)?;
            out.budget.charge_work(1)?;
            if !actual_function.definitions.contains(&index) {
                return Err(mismatch());
            }
            return self.emit_actual_relation(Some(index), width, out);
        }
        let first = self.reconstruction_recipe(input, original, out)?;
        if let Recipe::Actual(index) = first {
            out.budget.charge_work(2)?;
            if !actual_function.definitions.contains(&index)
                || actual.definitions()[index].ty != source.ty
            {
                return Err(mismatch());
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
            out,
            &mut |child, out| {
                out.budget.charge_work(3)?;
                let child_row = input.definitions().get(child).ok_or_else(mismatch)?;
                if function(child_row.coordinate) != function(source.coordinate) {
                    return Err(mismatch());
                }
                let recipe = self.reconstruction_recipe(input, child, out)?;
                if let Recipe::Actual(index) = recipe {
                    if !actual_function.definitions.contains(&index)
                        || actual.definitions().get(index).map(|row| row.ty) != Some(child_row.ty)
                    {
                        return Err(mismatch());
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
        for &node in &order {
            out.budget.charge_work(2)?;
            match recipes[node].ok_or_else(mismatch)? {
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
