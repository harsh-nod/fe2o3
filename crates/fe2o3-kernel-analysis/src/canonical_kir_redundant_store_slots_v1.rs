//! Complete direct-allocation use census. Rebuilt independently during replay;
//! no producer alias, initialization or MemorySSA report is accepted by replay.
use super::*;

pub(super) struct Slots<'i, 'g> {
    inventory: &'i Inventory<'g>,
    alignment: Vec<u32>,
}
impl<'i, 'g> Slots<'i, 'g> {
    pub(super) fn derive(
        inventory: &'i Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        scoped(budget, |budget| {
            budget.reserve_storage(size_of::<Self>())?;
            let (mut alignment, bytes) = reserve::<u32>(inventory.definitions().len(), budget)?;
            budget.charge_work(inventory.definitions().len())?;
            alignment.resize(inventory.definitions().len(), 0);
            for row in inventory.operations() {
                budget.charge_work(5)?;
                if let Kind::Alloca {
                    element: Type::Scalar(ty),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: value,
                } = row.operation.kind
                    && integer(&Type::Scalar(ty))
                    && value.is_power_of_two()
                    && value
                        >= u32::from(ty.bit_width().ok_or(Error::Rule("fixed slot width"))? / 8)
                    && row.results.len() == 1
                    && row.effects.len() == 1
                    && matches!(
                        inventory.effects()[row.effects.start].effect,
                        KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
                    )
                {
                    alignment[row.results.start] = value;
                }
            }
            for row in inventory.operations() {
                budget.charge_work(1)?;
                for operand in &inventory.uses()[row.operands.clone()] {
                    budget.charge_work(4)?;
                    if alignment[operand.definition] == 0 {
                        continue;
                    }
                    let allowed = match row.operation.kind {
                        Kind::Load { pointer, access } => {
                            pointer == operand.value
                                && access.address_space == AddressSpace::Private
                                && !access.volatile
                        }
                        Kind::Store {
                            pointer,
                            value,
                            access,
                        } => {
                            pointer == operand.value
                                && value != operand.value
                                && access.address_space == AddressSpace::Private
                                && !access.volatile
                        }
                        _ => false,
                    };
                    if !allowed {
                        alignment[operand.definition] = 0;
                    }
                }
            }
            for block in inventory.blocks() {
                budget.charge_work(1)?;
                for operand in &inventory.uses()[block.terminator_uses.clone()] {
                    budget.charge_work(2)?;
                    alignment[operand.definition] = 0;
                }
            }
            let retained = size_of::<Self>()
                .checked_add(bytes)
                .ok_or(Resource::Arithmetic)?;
            Ok((
                Self {
                    inventory,
                    alignment,
                },
                retained,
            ))
        })
    }

    pub(super) fn store(&self, ordinal: usize, budget: &mut Budget<'_>) -> Result<Option<Store>> {
        budget.charge_work(8)?;
        let row = self
            .inventory
            .operations()
            .get(ordinal)
            .ok_or(Error::Rule("Store ordinal"))?;
        let Kind::Store {
            pointer,
            value,
            access,
        } = row.operation.kind
        else {
            return Ok(None);
        };
        if access.address_space != AddressSpace::Private
            || access.volatile
            || access.alignment == 0
            || !row.operation.results.is_empty()
            || row.operands.len() != 2
            || row.effects.len() != 1
            || !matches!(
                self.inventory.effects()[row.effects.start].effect,
                KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
            )
        {
            return Ok(None);
        }
        let operands = &self.inventory.uses()[row.operands.clone()];
        if operands[0].value != pointer || operands[1].value != value {
            return Err(Error::Rule("exact Store operand census"));
        }
        let alignment = self.alignment[operands[0].definition];
        if alignment == 0
            || access.alignment > alignment
            || !integer(self.inventory.definitions()[operands[1].definition].ty)
        {
            return Ok(None);
        }
        Ok(Some(Store {
            coordinate: row.coordinate,
            pointer,
            value,
            access,
        }))
    }
}
