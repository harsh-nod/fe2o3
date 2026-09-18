//! Bounded direct scalar-slot premise for the new rule, not a general private
//! memory analyzer. Every accepted seed has explicit allocation/init evidence.
use super::*;

pub(super) struct InitializedPrivateLoadsV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    alignment: Vec<u32>,
    initialized: Option<usize>,
}
impl<'i, 'g> InitializedPrivateLoadsV1<'i, 'g> {
    pub(super) fn derive(
        inventory: &'i Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let floor = budget.storage();
        let result = Self::build(inventory, budget);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        result
    }
    fn build(inventory: &'i Inventory<'g>, budget: &mut Budget<'_>) -> Result<(Self, usize)> {
        budget.charge_work(3)?;
        let requested = inventory
            .definitions()
            .len()
            .checked_mul(size_of::<u32>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(
            size_of::<Self>()
                .checked_add(requested)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut alignment = Vec::new();
        alignment
            .try_reserve_exact(inventory.definitions().len())
            .map_err(|_| Resource::Allocation)?;
        let actual = alignment
            .capacity()
            .checked_mul(size_of::<u32>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        budget.charge_work(inventory.definitions().len())?;
        alignment.resize(inventory.definitions().len(), 0);
        for row in inventory.operations() {
            budget.charge_work(5)?;
            if let Kind::Alloca {
                element: Type::Scalar(ty),
                count: None,
                address_space: AddressSpace::Private,
                alignment: bytes,
            } = row.operation.kind
                && integer(ty)
                && bytes.is_power_of_two()
                && bytes >= u32::from(ty.bit_width().ok_or(Error::Rule("fixed integer width"))? / 8)
                && row.results.len() == 1
            {
                alignment[row.results.start] = bytes;
            }
        }
        // Complete definition-use census: no derived alias, address escape,
        // reference storage, call, assembly or control-edge transport.
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
            .checked_add(actual)
            .ok_or(Resource::Arithmetic)?;
        Ok((
            Self {
                inventory,
                alignment,
                initialized: None,
            },
            retained,
        ))
    }
    pub(super) fn enter_block(&mut self) {
        self.initialized = None;
    }
    pub(super) fn observe(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Load>> {
        budget.charge_work(6)?;
        let inventory = self.inventory;
        let row = inventory
            .operations()
            .get(ordinal)
            .ok_or(Error::Rule("initialization operation ordinal"))?;
        match row.operation.kind {
            Kind::Store { access, .. } if access.address_space == AddressSpace::Private => {
                let definition = inventory.uses()[row.operands.start].definition;
                let alignment = self.alignment[definition];
                self.initialized = (alignment != 0
                    && !access.volatile
                    && access.alignment != 0
                    && access.alignment <= alignment)
                    .then_some(definition);
            }
            Kind::Load { access, .. } => {
                let definition = inventory.uses()[row.operands.start].definition;
                if self.initialized == Some(definition)
                    && self.alignment[definition] != 0
                    && access.alignment != 0
                    && access.alignment <= self.alignment[definition]
                    && let Some(load) = load(row.operation, row.coordinate)
                {
                    return Ok(Some(load));
                }
                self.initialized = None;
            }
            Kind::Store { access, .. }
                if access.address_space == AddressSpace::Global && !access.volatile => {}
            Kind::Compare { .. } => {}
            _ if transparent(row.operation) => {}
            _ => self.initialized = None,
        }
        Ok(None)
    }
}
