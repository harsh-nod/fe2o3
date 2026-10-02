//! Private source-emission index for a single parent module. Reuse retains the
//! exact inventory, physical analysis, interpretation and allocation resolver;
//! it neither authorizes source objects nor replaces any derivation or proof.
use super::*;

pub(in super::super) struct EmittedByteFunctionsV55<'a, 'owner, R> {
    inventory: &'a Inventory<'owner>,
    physical: &'a Physical<'a, 'owner>,
    allocations: &'a R,
    interpretation: ByteInterpretationContextV39<'a, 'owner>,
    namespaces: Vec<Option<usize>>,
    sites: Vec<Option<ByteAllocationSiteV30>>,
    scalar_bodies: Vec<Option<CanonicalByteScalarBodyV55>>,
    last_namespace: Option<usize>,
    emitted_bytes: usize,
    buffer: usize,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

fn emitted_mismatch() -> Error {
    Error::Statement("reused byte function was not emitted for the exact model")
}

impl<'a, 'owner, R: ByteAllocationResolverV30> EmittedByteFunctionsV55<'a, 'owner, R> {
    pub(in super::super) fn new(
        inventory: &'a Inventory<'owner>,
        physical: &'a Physical<'a, 'owner>,
        allocations: &'a R,
        interpretation: ByteInterpretationContextV39<'a, 'owner>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        allocations.check_owner(inventory.owner(), out)?;
        interpretation.check_owner(inventory.owner(), out)?;
        out.budget.charge_work(2)?;
        if !physical.is_for(inventory) {
            return Err(emitted_mismatch());
        }
        out.budget.reserve_storage(
            size_of::<Self>()
                + size_of::<Result<Self>>()
                + size_of::<CanonicalByteScalarBodyV55>()
                + size_of::<Result<CanonicalByteScalarBodyV55>>()
                + size_of::<([&(); 6], [usize; 4], [Result<()>; 2])>(),
        )?;
        let mut namespaces = vector(inventory.functions().len(), out)?;
        out.budget.charge_work(inventory.functions().len())?;
        namespaces.resize(inventory.functions().len(), None);
        let mut sites = vector(inventory.operations().len(), out)?;
        out.budget.charge_work(inventory.operations().len())?;
        sites.resize(inventory.operations().len(), None);
        let mut scalar_bodies = vector(inventory.operations().len(), out)?;
        out.budget.charge_work(inventory.operations().len())?;
        scalar_bodies.resize(inventory.operations().len(), None);
        Ok(Self {
            inventory,
            physical,
            allocations,
            interpretation,
            namespaces,
            sites,
            scalar_bodies,
            last_namespace: None,
            emitted_bytes: out.text.len(),
            buffer: out.text.as_ptr() as usize,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn retain<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.slot != std::ptr::from_ref(out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
            || self.buffer != out.text.as_ptr() as usize
            || out.text.len() < self.emitted_bytes
        {
            self.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        self.retain((|| {
            out.budget.charge_work(2)?;
            self.allocations.check_owner(self.inventory.owner(), out)?;
            self.interpretation.check_owner(self.inventory.owner(), out)
        })())
    }

    fn check_model<S: ByteAllocationResolverV30>(
        &self,
        model: &ByteFunctionV30<'_, '_, S>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        model.check(out)?;
        out.budget.charge_work(4)?;
        if !std::ptr::eq(self.inventory, model.inventory)
            || !std::ptr::eq(self.physical, model._physical)
            || !self.interpretation.same_descriptor(&model.interpretation)
            || self
                .inventory
                .functions()
                .get(model.function.0 as usize)
                .is_none_or(|row| row.operations.len() != model.operations.len())
        {
            return Err(emitted_mismatch());
        }
        Ok(())
    }

    pub(in super::super) fn emit(
        &mut self,
        model: &ByteFunctionV30<'_, '_, R>,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = (|| {
            self.check_model(model, out)?;
            out.budget.charge_work(3)?;
            let function = model.function.0 as usize;
            if !std::ptr::eq(self.allocations, model.allocations)
                || self.namespaces.get(function) != Some(&None)
                || self.last_namespace.is_some_and(|last| last >= namespace)
            {
                return Err(emitted_mismatch());
            }
            model.emit_with_scalar_bodies(namespace, ScalarBodiesV55::Define, out)?;
            let row = &self.inventory.functions()[function];
            for (operation, plan) in row.operations.clone().zip(&model.operations) {
                out.budget.charge_work(1)?;
                self.sites[operation] = match plan {
                    ByteOperationV30::Alloca(alloca) => Some(alloca.allocation_site()),
                    _ => None,
                };
                self.scalar_bodies[operation] = match plan {
                    ByteOperationV30::Scalar(scalar) => Some(scalar.body_descriptor_v55(out)?),
                    _ => None,
                };
            }
            self.namespaces[function] = Some(namespace);
            self.last_namespace = Some(namespace);
            self.emitted_bytes = out.text.len();
            self.check(out)
        })();
        self.retain(result)
    }

    // Reuse only the total scalar transition, never the input owner's registry
    // predicate. The output keeps its own PC, input checks and refusal path.
    pub(in super::super) fn emit_output_reusing_scalar_bodies<S: ByteAllocationResolverV30>(
        &self,
        model: &ByteFunctionV30<'_, '_, S>,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = (|| {
            self.check(out)?;
            model.check(out)?;
            out.budget.charge_work(5)?;
            if self.interpretation.width != model.interpretation.width {
                return Err(emitted_mismatch());
            }
            let function = model.function.0 as usize;
            let original = self
                .inventory
                .functions()
                .get(function)
                .ok_or_else(emitted_mismatch)?;
            let current = &model.inventory.functions()[function];
            let source = self
                .namespaces
                .get(function)
                .copied()
                .flatten()
                .ok_or_else(emitted_mismatch)?;
            if original.coordinate != current.coordinate
                || original.operations != current.operations
                || original.blocks != current.blocks
            {
                return Err(emitted_mismatch());
            }
            let floor = out.budget.storage();
            out.budget.reserve_storage(
                size_of::<Vec<bool>>()
                    + size_of::<Result<Vec<bool>>>()
                    + size_of::<CanonicalByteScalarBodyV55>(),
            )?;
            let mut reuse = vector(model.operations.len(), out)?;
            for (operation, plan) in current.operations.clone().zip(&model.operations) {
                out.budget.charge_work(5)?;
                let left = &self.inventory.operations()[operation];
                let right = &model.inventory.operations()[operation];
                if left.coordinate != right.coordinate
                    || block_index(self.inventory, left.coordinate.block)?
                        != block_index(model.inventory, right.coordinate.block)?
                {
                    return Err(emitted_mismatch());
                }
                let shared = if let (Some(original), ByteOperationV30::Scalar(scalar)) =
                    (self.scalar_bodies[operation], plan)
                {
                    original == scalar.body_descriptor_v55(out)?
                } else {
                    false
                };
                reuse.push(shared);
                if shared {
                    emit!(
                        out,
                        "use super::byte_scalar_body_{source}_{operation}_v55 as byte_scalar_body_{namespace}_{operation}_v55;\n"
                    );
                }
            }
            model.emit_with_scalar_bodies(namespace, ScalarBodiesV55::Reuse(&reuse), out)?;
            drop(reuse);
            out.budget.release_storage(
                out.budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
            self.check(out)?;
            model.check(out)
        })();
        self.retain(result)
    }

    pub(in super::super) fn emit_parent_aliases<S: ByteAllocationResolverV30>(
        &self,
        model: &ByteFunctionV30<'_, '_, S>,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = (|| {
            self.check_model(model, out)?;
            out.budget.charge_work(2)?;
            let function = model.function.0 as usize;
            let source = self
                .namespaces
                .get(function)
                .copied()
                .flatten()
                .ok_or_else(emitted_mismatch)?;
            let row = &self.inventory.functions()[function];
            // A forwarding wrapper may retain the same resolver under a new
            // type. Check every allocation recipe, not merely the owner label.
            for (operation, plan) in row.operations.clone().zip(&model.operations) {
                out.budget.charge_work(2)?;
                let captured = match plan {
                    ByteOperationV30::Alloca(alloca) => Some(alloca.allocation_site()),
                    _ => None,
                };
                if self.sites[operation] != captured {
                    return Err(emitted_mismatch());
                }
                if let Some(site) = captured {
                    let coordinate = self.inventory.operations()[operation].coordinate;
                    if self.allocations.site(coordinate, out)? != site
                        || model.allocations.site(coordinate, out)? != site
                    {
                        return Err(emitted_mismatch());
                    }
                }
            }
            emit!(
                out,
                "use super::{{\n byte_inputs_{source}_v55 as byte_inputs_{namespace}_v55,\n"
            );
            for operation in row.operations.clone() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " byte_operation_{source}_{operation}_v30 as byte_operation_{namespace}_{operation}_v30,\n"
                );
            }
            for block in row.blocks.clone() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " byte_block_{source}_{block}_v30 as byte_block_{namespace}_{block}_v30,\n byte_control_{source}_{block}_v30 as byte_control_{namespace}_{block}_v30,\n"
                );
            }
            for name in ["micro_begin", "micro_step", "micro_finish", "block_step"] {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " byte_{name}_{source}_v30 as byte_{name}_{namespace}_v30,\n"
                );
            }
            emit!(out, "}};\n");
            self.check(out)?;
            model.check(out)
        })();
        self.retain(result)
    }
}
