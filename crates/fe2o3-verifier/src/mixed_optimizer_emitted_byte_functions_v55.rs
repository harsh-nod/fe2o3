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
    transition_bodies: Vec<Option<TransitionBodyV56<'a>>>,
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

#[derive(Clone, Copy)]
enum BodyReuseScope {
    Parent,
    LocalPredecessor,
    ParentPredecessor,
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
                + 2 * size_of::<TransitionBodyV56<'a>>()
                + size_of::<Option<TransitionBodyV56<'a>>>()
                + size_of::<Result<Option<TransitionBodyV56<'a>>>>()
                + size_of::<Option<(&Self, BodyReuseScope)>>()
                + size_of::<([&(); 12], [usize; 6], [Result<()>; 2], BodyReuseScope)>(),
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
        let mut transition_bodies = vector(inventory.operations().len(), out)?;
        out.budget.charge_work(inventory.operations().len())?;
        transition_bodies.resize(inventory.operations().len(), None);
        Ok(Self {
            inventory,
            physical,
            allocations,
            interpretation,
            namespaces,
            sites,
            scalar_bodies,
            transition_bodies,
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
        model: &ByteFunctionV30<'a, 'owner, R>,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_recorded::<R>(model, namespace, None, out)
    }

    pub(in super::super) fn emit_after_local_predecessor_v93<S: ByteAllocationResolverV30>(
        &mut self,
        model: &ByteFunctionV30<'a, 'owner, R>,
        namespace: usize,
        predecessor: &EmittedByteFunctionsV55<'_, '_, S>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_recorded(
            model,
            namespace,
            Some((predecessor, BodyReuseScope::LocalPredecessor)),
            out,
        )
    }

    pub(in super::super) fn emit_after_parent_predecessor_v95<S: ByteAllocationResolverV30>(
        &mut self,
        model: &ByteFunctionV30<'a, 'owner, R>,
        namespace: usize,
        predecessor: &EmittedByteFunctionsV55<'_, '_, S>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_recorded(
            model,
            namespace,
            Some((predecessor, BodyReuseScope::ParentPredecessor)),
            out,
        )
    }

    fn emit_recorded<S: ByteAllocationResolverV30>(
        &mut self,
        model: &ByteFunctionV30<'a, 'owner, R>,
        namespace: usize,
        predecessor: Option<(&EmittedByteFunctionsV55<'_, '_, S>, BodyReuseScope)>,
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
            match predecessor {
                Some((previous, scope)) => {
                    previous.emit_reusing_bodies(model, namespace, scope, out)?
                }
                None => model.emit_with_bodies_v56(namespace, ByteBodiesV56::Define, out)?,
            }
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
                self.transition_bodies[operation] = TransitionBodyV56::capture(plan, out)?;
            }
            self.namespaces[function] = Some(namespace);
            self.last_namespace = Some(namespace);
            self.emitted_bytes = out.text.len();
            self.check(out)
        })();
        self.retain(result)
    }

    // Reuse only an exact total transition, never the input owner's registry
    // predicate. The output keeps its own PC, input checks and refusal path.
    pub(in super::super) fn emit_output_reusing_bodies_v56<S: ByteAllocationResolverV30>(
        &self,
        model: &ByteFunctionV30<'_, '_, S>,
        namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_reusing_bodies(model, namespace, BodyReuseScope::Parent, out)
    }

    fn emit_reusing_bodies<S: ByteAllocationResolverV30>(
        &self,
        model: &ByteFunctionV30<'_, '_, S>,
        namespace: usize,
        scope: BodyReuseScope,
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
            let local = matches!(scope, BodyReuseScope::LocalPredecessor);
            let predecessor = !matches!(scope, BodyReuseScope::Parent);
            if original.coordinate != current.coordinate
                || (!predecessor
                    && (original.operations != current.operations
                        || original.blocks != current.blocks))
                || (local && source == namespace)
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
                let right = &model.inventory.operations()[operation];
                // Motion can change occurrence coordinates. Such operations
                // keep a new complete body, even if their arithmetic agrees.
                let Some(left) = self
                    .inventory
                    .operations()
                    .get(operation)
                    .filter(|_| original.operations.contains(&operation))
                else {
                    reuse.push(false);
                    continue;
                };
                if left.coordinate != right.coordinate
                    || block_index(self.inventory, left.coordinate.block)?
                        != block_index(model.inventory, right.coordinate.block)?
                {
                    if predecessor {
                        reuse.push(false);
                        continue;
                    }
                    return Err(emitted_mismatch());
                }
                let scalar = matches!(plan, ByteOperationV30::Scalar(_));
                let shared = if let (Some(original), ByteOperationV30::Scalar(scalar)) =
                    (self.scalar_bodies[operation], plan)
                {
                    original == scalar.body_descriptor_v55(out)?
                } else if let (Some(original), Some(current)) = (
                    self.transition_bodies[operation],
                    TransitionBodyV56::capture(plan, out)?,
                ) {
                    (!original.requires_same_interpretation()
                        || self.interpretation.same_descriptor(&model.interpretation))
                        && original.same(current, out)?
                } else {
                    false
                };
                // Predecessor allocation transport is independently derived;
                // leave its body distinct rather than reuse an origin label.
                let shared =
                    shared && !(predecessor && matches!(plan, ByteOperationV30::Alloca(_)));
                if shared && let ByteOperationV30::Alloca(alloca) = plan {
                    let site = alloca.allocation_site();
                    if self.sites[operation] != Some(site)
                        || self.allocations.site(left.coordinate, out)? != site
                        || model.allocations.site(right.coordinate, out)? != site
                    {
                        return Err(emitted_mismatch());
                    }
                }
                reuse.push(shared);
                if shared {
                    let (name, version) = if scalar {
                        ("byte_scalar_body", 55)
                    } else {
                        ("byte_transition_body", 56)
                    };
                    emit!(
                        out,
                        "use {}::{name}_{source}_{operation}_v{version} as {name}_{namespace}_{operation}_v{version};\n",
                        if local { "self" } else { "super" }
                    );
                }
            }
            model.emit_with_bodies_v56(
                namespace,
                if predecessor {
                    ByteBodiesV56::DefineOrReuse(&reuse)
                } else {
                    ByteBodiesV56::Reuse(&reuse)
                },
                out,
            )?;
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
