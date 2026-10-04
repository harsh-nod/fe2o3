use super::*;

#[derive(Clone, Copy)]
enum Action {
    Retain,
    Remove,
    Copy(ValueId),
}

/// Private, linear-use recipe bound to the real input until mutation completes.
pub(super) struct Recipe<'a> {
    input: &'a Owner,
    actions: Vec<Action>,
    selected: Vec<Coordinate>,
    origins: Vec<Origin>,
    action_bytes: usize,
    functions: usize,
    blocks: usize,
}
impl<'a> Recipe<'a> {
    pub(super) fn prepare(
        inventory: &Inventory<'a>,
        census: &Census<'_, 'a>,
        meter: &mut Meter<'_, '_>,
    ) -> Result<Self> {
        meter.work(1)?;
        if !census.belongs_to(inventory) {
            return Err(Error::Recipe("actual census inventory"));
        }
        meter.reserve(size_of::<Self>())?;
        let count = inventory.operations().len();
        let (mut actions, action_bytes) = meter.table(count)?;
        for _ in 0..count {
            meter.push(&mut actions, Action::Retain)?;
        }
        let (mut selected, _) = meter.table(census.allocations().len())?;
        let mut previous = None;
        for row in census.allocations() {
            let at = ordinal(inventory, row.allocation, meter)?;
            if previous.is_some_and(|prior| prior >= at) {
                return Err(Error::Recipe("allocation occurrence order"));
            }
            set(&mut actions, at, Action::Remove, meter)?;
            meter.push(&mut selected, row.allocation)?;
            previous = Some(at);
        }
        for row in census.addresses() {
            meter.work(1)?;
            if row.producer == row.allocation {
                continue;
            }
            let at = ordinal(inventory, row.producer, meter)?;
            set(&mut actions, at, Action::Remove, meter)?;
        }
        // Access rows are grouped by cell, not physical operation order. Fill a
        // dense action/claim index once; never shift or rescan a block per row.
        let (mut copies, copy_bytes) = meter.table::<Option<OriginKind>>(count)?;
        for _ in 0..count {
            meter.push(&mut copies, None)?;
        }
        for row in census.accesses() {
            let at = ordinal(inventory, row.operation, meter)?;
            let action = match row.kind {
                AccessKind::Store { .. } => Action::Remove,
                AccessKind::Load {
                    result,
                    previous_store,
                    stored_value,
                } => {
                    meter.work(4)?;
                    let op = inventory.operations()[at].operation;
                    if !matches!(op.kind,Kind::Load {pointer,..} if pointer==row.pointer)
                        || op.results.len() != 1
                        || op.results[0].id != result
                    {
                        return Err(Error::Recipe("actual census Load"));
                    }
                    copies[at] = Some(OriginKind::LoadCopy {
                        allocation: row.allocation,
                        previous_store,
                        stored_value,
                    });
                    Action::Copy(stored_value)
                }
            };
            set(&mut actions, at, action, meter)?;
        }
        let mut output_count = 0usize;
        for action in &actions {
            meter.work(1)?;
            if !matches!(action, Action::Remove) {
                output_count = output_count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        let (mut origins, _) = meter.table(output_count)?;
        for block in inventory.blocks() {
            meter.work(1)?;
            let mut output = 0u32;
            for at in block.operations.clone() {
                meter.work(3)?;
                let kind = match actions[at] {
                    Action::Remove => continue,
                    Action::Retain => OriginKind::Retained,
                    Action::Copy(_) => copies[at].ok_or(Error::Recipe("prepared Load claim"))?,
                };
                meter.push(
                    &mut origins,
                    Origin {
                        input: inventory.operations()[at].coordinate,
                        output: Coordinate {
                            block: block.coordinate,
                            operation: output,
                        },
                        kind,
                    },
                )?;
                output = output.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if origins.len() != output_count {
            return Err(Error::Recipe("complete predicted output"));
        }
        drop(copies);
        meter.release(copy_bytes)?;
        Ok(Self {
            input: inventory.owner(),
            actions,
            selected,
            origins,
            action_bytes,
            functions: inventory.functions().len(),
            blocks: inventory.blocks().len(),
        })
    }

    /// Every possible work/storage denial precedes mutation. The stable pass
    /// contains no allocation, fallible budget call, or external callback.
    /// Its internal invariant failure still discards the entire candidate.
    pub(super) fn apply(
        self,
        candidate: &mut Module,
        meter: &mut Meter<'_, '_>,
    ) -> Result<(Vec<Coordinate>, Vec<Origin>)> {
        meter.work(self.input.canonical().canonical_bytes().len())?;
        if candidate != self.input.module() {
            return Err(Error::Recipe("exact private candidate"));
        }
        // Prepay stable-compaction Operation moves, fixed visits/updates and all
        // decoder-owned payload destruction. Removal and later final candidate
        // drop visit disjoint owned subtrees; the original encoding bounds both.
        let visits = self
            .actions
            .len()
            .checked_mul(
                size_of::<Operation>()
                    .checked_add(5)
                    .ok_or(Resource::Arithmetic)?,
            )
            .and_then(|n| n.checked_add(self.functions))
            .and_then(|n| n.checked_add(self.blocks))
            .and_then(|n| n.checked_add(self.input.canonical().canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?;
        meter.work(visits)?;
        let mut next = 0usize;
        let mut kept = 0usize;
        for function in &mut candidate.functions {
            if let Some(body) = &mut function.body {
                for block in &mut body.blocks {
                    block.operations.retain_mut(|operation| {
                        let action = self.actions[next];
                        next += 1;
                        match action {
                            Action::Remove => false,
                            Action::Retain => {
                                kept += 1;
                                true
                            }
                            Action::Copy(value) => {
                                operation.kind = Kind::Binary {
                                    op: BinaryOp::BitOr,
                                    lhs: value,
                                    rhs: value,
                                };
                                kept += 1;
                                true
                            }
                        }
                    });
                }
            }
        }
        if next != self.actions.len() || kept != self.origins.len() {
            return Err(Error::Recipe("mutation traversal endpoints"));
        }
        let Self {
            actions,
            selected,
            origins,
            action_bytes,
            ..
        } = self;
        drop(actions);
        meter.release(
            size_of::<Self>()
                .checked_add(action_bytes)
                .ok_or(Resource::Arithmetic)?,
        )?;
        Ok((selected, origins))
    }
}

fn set(actions: &mut [Action], at: usize, next: Action, meter: &mut Meter<'_, '_>) -> Result<()> {
    meter.work(2)?;
    if !matches!(actions[at], Action::Retain) {
        return Err(Error::Recipe("unique original action"));
    }
    actions[at] = next;
    Ok(())
}
fn ordinal(
    inventory: &Inventory<'_>,
    coordinate: Coordinate,
    meter: &mut Meter<'_, '_>,
) -> Result<usize> {
    meter.work(7)?;
    let function = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
        .filter(|row| row.coordinate == coordinate.block.function)
        .ok_or(Error::Recipe("actual input coordinate"))?;
    let block = function
        .blocks
        .start
        .checked_add(coordinate.block.block as usize)
        .filter(|at| *at < function.blocks.end)
        .and_then(|at| inventory.blocks().get(at))
        .filter(|row| row.coordinate == coordinate.block)
        .ok_or(Error::Recipe("actual input coordinate"))?;
    let at = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|at| *at < block.operations.end)
        .ok_or(Error::Recipe("actual input coordinate"))?;
    if inventory.operations()[at].coordinate != coordinate {
        return Err(Error::Recipe("actual input coordinate"));
    }
    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
        Function, MemoryAccess, ScalarType, Signature, Terminator, Type, ValueDef,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    #[test]
    fn corrupted_private_recipe_panic_discards_a_partially_edited_candidate() {
        let ty = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
        let access = MemoryAccess::new(AddressSpace::Private, 4);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(1), pointer),
                Kind::Alloca {
                    element: ty.clone(),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(1),
                    value: ValueId(0),
                    access,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), ty.clone()),
                Kind::Load {
                    pointer: ValueId(1),
                    access,
                },
            ),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(2)],
        });
        let mut module = Module::new("partial-private-candidate");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![ty.clone()], vec![ty]),
            vec![ValueId(0)],
            vec![block],
        ));
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 64 << 20);
        let (input, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(43 + receipt.retained_storage())
            .unwrap();
        {
            let input = &input;
            let floor = budget.storage();
            let result: Result<()> = scoped(&mut budget, |meter| {
                let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
                meter.reserve(is.retained_storage())?;
                let (census, cs) =
                    meter.derive(|b| Ok(Census::derive(&inventory, Limits::default(), b)?))?;
                meter.reserve(cs.retained_storage())?;
                let mut recipe = Recipe::prepare(&inventory, &census, meter)?;
                let (mut candidate, receipt) =
                    meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
                meter.reserve(receipt.retained_storage())?;
                // Deliberately violate the private action-count invariant. No
                // callback or mutable recipe is exposed by the real service.
                recipe.actions[0] = Action::Remove;
                recipe.actions.truncate(1);
                match catch_unwind(AssertUnwindSafe(|| recipe.apply(&mut candidate, meter))) {
                    Err(payload) => {
                        assert_ne!(&candidate, input.module());
                        resume_unwind(payload)
                    }
                    Ok(_) => Err(Error::Recipe("hostile private recipe must panic")),
                }
            });
            assert!(matches!(result, Err(Error::Panicked)));
            assert_eq!(budget.storage(), floor);
            assert_eq!(input.module(), &module);
        }
        drop(input);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 43);
    }
}
