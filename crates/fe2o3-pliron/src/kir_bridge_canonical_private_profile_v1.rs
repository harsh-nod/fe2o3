//! Certificate-only native profile. Ordinary projection admission stays closed.
use super::*;
use crate::production_analysis::canonical_ranked_checks_v1::private::{
    CanonicalPrivateRequirementV1 as Need, PrivateOperationKindV1 as Kind, refuse,
};
use pliron::builtin::op_interfaces::OneRegionInterface;

pub(crate) fn keys(
    context: &Context,
    operation: Ptr<Operation>,
) -> Option<&'static [&'static str]> {
    if Operation::is_op::<PreservedOperationOp>(operation, context) {
        Some(&["gpu_preserved_operation_kind"])
    } else if Operation::is_op::<LoadOp>(operation, context) {
        Some(&[
            "gpu_load_address_space",
            "gpu_load_alignment",
            "gpu_load_volatile",
        ])
    } else if Operation::is_op::<StoreOp>(operation, context) {
        Some(&[
            "gpu_store_address_space",
            "gpu_store_alignment",
            "gpu_store_volatile",
        ])
    } else if Operation::is_op::<CallOp>(operation, context) {
        Some(&["gpu_call_callee", "gpu_call_signature"])
    } else if Operation::is_op::<GetElementPointerOp>(operation, context) {
        Some(&[])
    } else {
        None
    }
}

pub(crate) fn check_calls(
    projection: &NativeCanonicalRankedProjectionV1<'_>,
    facts: &CanonicalPrivateGraphFactsV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    if !std::ptr::eq(projection.owner(), facts.inventory().owner()) {
        return Err(Failure::ExactGraph);
    }
    for call in facts.inventory().calls() {
        let c = call.coordinate;
        let target = call
            .target
            .ok_or_else(|| refuse(Need::DefinedAcyclicCalls, Some(c)))?;
        budget.charge_work(checked_bridge_add_v12(
            c.block.block as usize + 1,
            c.operation as usize + 1,
        )?)?;
        budget.charge_work(checked_bridge_add_v12(1, call.callee.len())?)?;
        let signature = projection.with_function(
            c.block.function.0 as usize,
            budget,
            |context, function| {
                let region = function.get_region(context).deref(context);
                let block = region
                    .iter(context)
                    .nth(c.block.block as usize)
                    .ok_or(Failure::NativeSchema)?;
                let raw = block.deref(context);
                let pointer = raw
                    .iter(context)
                    .nth(c.operation as usize)
                    .ok_or(Failure::NativeSchema)?;
                let native =
                    Operation::get_op::<CallOp>(pointer, context).ok_or(Failure::NativeSchema)?;
                // Borrow the actual string attribute; do not make an unpaid owned getter copy.
                let text = native
                    .get_attr_gpu_call_callee(context)
                    .ok_or(Failure::NativeSchema)?;
                if text.as_str() != call.callee {
                    return Err(refuse(Need::NativeJoin, Some(c)));
                }
                Ok::<_, Failure>(native.signature(context).ok_or(Failure::NativeSchema)?)
            },
        )??;
        budget.charge_work(1)?;
        let target_row = facts
            .inventory()
            .functions()
            .get(target.0 as usize)
            .ok_or_else(|| refuse(Need::NativeJoin, Some(c)))?;
        let actual = if target_row.function.body.is_none() {
            facts.charge_terminal_lookups(1, budget)?;
            if !facts
                .terminal_facts()
                .is_some_and(|proof| proof.is_call(c) && proof.is_declaration(target))
            {
                return Err(refuse(Need::NativeJoin, Some(c)));
            }
            projection
                .terminal
                .as_ref()
                .and_then(|terminal| terminal.signature())
                .ok_or_else(|| refuse(Need::NativeJoin, Some(c)))?
        } else {
            projection.with_function(target.0 as usize, budget, |context, function| {
                function.get_type(context)
            })?
        };
        if signature != actual {
            return Err(refuse(Need::NativeJoin, Some(c)));
        }
    }
    Ok(())
}

pub(crate) struct NativeCanonicalPrivateProjectionV1<'a> {
    projection: NativeCanonicalRankedProjectionV1<'a>,
    facts: &'a CanonicalPrivateGraphFactsV1<'a, 'a>,
}

impl<'a> NativeCanonicalPrivateProjectionV1<'a> {
    pub(crate) fn import(
        facts: &'a CanonicalPrivateGraphFactsV1<'a, 'a>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let projection = NativeCanonicalRankedProjectionV1::import_profile(
            facts.inventory().owner(),
            budget,
            Some(facts),
        )?;
        Ok(Self { projection, facts })
    }

    pub(crate) fn check(&mut self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.projection.check_profile(budget, Some(self.facts))
    }

    pub(crate) fn check_epoch(&self) -> Result<(), Failure> {
        self.projection.check_epoch()
    }

    pub(crate) fn with_function<T>(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
        run: impl FnOnce(&NativeCanonicalPrivateAdmissionV1<'_>) -> T,
    ) -> Result<T, Failure> {
        let row = self
            .facts
            .inventory()
            .functions()
            .get(ordinal)
            .ok_or(Failure::InvalidQuery { function: ordinal })?;
        let count = row
            .operations
            .len()
            .checked_add(row.blocks.len())
            .ok_or(Resource::Arithmetic)?;
        let mut rows = Vec::new();
        let bytes = count
            .checked_mul(std::mem::size_of::<NativeRow>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(checked_bridge_add_v12(
            bytes,
            std::mem::size_of::<NativeCanonicalPrivateAdmissionV1<'_>>(),
        )?)?;
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            rows.capacity()
                .checked_sub(count)
                .and_then(|n| n.checked_mul(std::mem::size_of::<NativeRow>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.charge_work(checked_bridge_add_v12(count, row.blocks.len())?)?;
        self.facts.charge_terminal_lookups(count, budget)?;
        self.projection
            .with_function(ordinal, budget, |context, function| {
                let region = function.get_region(context).deref(context);
                let mut blocks = region.iter(context);
                for source in &self.facts.inventory().blocks()[row.blocks.clone()] {
                    let block = blocks.next().ok_or(Failure::NativeSchema)?;
                    let raw = block.deref(context);
                    let mut native = raw.iter(context);
                    for index in source.operations.clone() {
                        let pointer = native.next().ok_or(Failure::NativeSchema)?;
                        let kind = self.facts.operation_kind(index)?;
                        let matches = match kind {
                            Kind::Allocate => {
                                Operation::get_op::<PreservedOperationOp>(pointer, context)
                                    .is_some_and(|op| {
                                        op.kind(context) == Some(PreservedOperationKindAttr::Alloca)
                                    })
                            }
                            Kind::Address => {
                                Operation::is_op::<GetElementPointerOp>(pointer, context)
                            }
                            Kind::Read => Operation::is_op::<LoadOp>(pointer, context),
                            Kind::Write => Operation::is_op::<StoreOp>(pointer, context),
                            Kind::Call | Kind::TrapCall => {
                                Operation::is_op::<CallOp>(pointer, context)
                            }
                            Kind::TrapEnd => false,
                            Kind::Scalar => keys(context, pointer).is_none(),
                        };
                        if !matches {
                            return Err(refuse(Need::NativeJoin, None));
                        }
                        rows.push(NativeRow { pointer, kind });
                    }
                    let pointer = native.next().ok_or(Failure::NativeSchema)?;
                    let kind = self.facts.terminator_kind(source.coordinate);
                    if kind == Kind::TrapEnd && !super::trap_profile::terminal(context, pointer) {
                        return Err(refuse(Need::NativeJoin, None));
                    }
                    rows.push(NativeRow { pointer, kind });
                    if native.next().is_some() {
                        return Err(Failure::NativeSchema);
                    }
                }
                if blocks.next().is_some() || rows.len() != count {
                    return Err(Failure::NativeSchema);
                }
                let admission = NativeCanonicalPrivateAdmissionV1 {
                    facts: self.facts,
                    context,
                    function,
                    rows,
                    ordinal,
                    epoch: self.projection.epoch,
                };
                if !admission.authenticate(context, function) {
                    return Err(Failure::Mutation);
                }
                Ok(run(&admission))
            })?
    }

    #[cfg(test)]
    pub(crate) fn test_rebase_epoch(&mut self) {
        self.projection.test_rebase_epoch();
    }

    #[cfg(test)]
    pub(crate) fn test_live<T>(&self, run: impl FnOnce(&Context, Ptr<Operation>) -> T) -> T {
        self.projection.test_live(run)
    }
}

#[derive(Clone, Copy)]
struct NativeRow {
    pointer: Ptr<Operation>,
    kind: Kind,
}

/// Only the exact checked import constructs this borrowed occurrence capability.
pub(crate) struct NativeCanonicalPrivateAdmissionV1<'a> {
    facts: &'a CanonicalPrivateGraphFactsV1<'a, 'a>,
    context: &'a Context,
    function: &'a FuncOp,
    rows: Vec<NativeRow>,
    ordinal: usize,
    epoch: u64,
}

impl NativeCanonicalPrivateAdmissionV1<'_> {
    pub(crate) fn context(&self) -> &Context {
        self.context
    }
    pub(crate) fn function(&self) -> &FuncOp {
        self.function
    }
    pub(crate) fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn operation_count(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn authenticate(&self, context: &Context, function: &FuncOp) -> bool {
        std::ptr::eq(context, self.context)
            && function.get_operation() == self.function.get_operation()
            && epoch(context).ok() == Some(self.epoch)
            && self.facts.memory_belongs_to(self.facts.inventory())
    }

    pub(crate) fn operation(&self, context: &Context, pointer: Ptr<Operation>) -> Option<Kind> {
        if !self.authenticate(context, self.function) {
            return None;
        }
        self.rows
            .iter()
            .find(|row| row.pointer == pointer)
            .map(|row| row.kind)
    }

    pub(crate) fn attribute(
        &self,
        context: &Context,
        pointer: Ptr<Operation>,
        key: &str,
        dialect: &str,
        name: &str,
    ) -> bool {
        let Some(kind) = self.operation(context, pointer) else {
            return false;
        };
        if dialect != "gpu" {
            return false;
        }
        if kind == Kind::TrapEnd {
            return super::trap_profile::attribute(key, dialect, name);
        }
        matches!(
            (kind, key, name),
            (
                Kind::Allocate,
                "gpu_preserved_operation_kind",
                "preserved_operation_kind"
            ) | (Kind::Read, "gpu_load_alignment", "memory_alignment")
                | (Kind::Read, "gpu_load_volatile", "volatile")
                | (Kind::Write, "gpu_store_alignment", "memory_alignment")
                | (Kind::Write, "gpu_store_volatile", "volatile")
        )
    }

    // Per capture: closed attribute schemas have at most three extension fields,
    // each checked in prescan plus both encoding traversals. All row lookups are
    // bounded linear scans; ordinary rendering/SSA costs retain their own bound.
    pub(crate) fn identity_lookup_work(&self) -> Option<usize> {
        self.rows
            .len()
            .checked_add(1)?
            .checked_pow(2)?
            .checked_mul(512)
    }

    pub(crate) fn stage_lookup_work(&self) -> Option<usize> {
        self.rows
            .len()
            .checked_add(1)?
            .checked_pow(2)?
            .checked_mul(32)
    }
}

#[cfg(test)]
mod private_call_whole_entry_layout_premises {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn private_call_whole_entry_pinned_private_native_layout_equivalence_premises() {
        // Test-only access to actual private row types. No visibility change,
        // cost helper, proof construction or measured successful totals.
        type SignatureFields = (Ptr<Operation>, TypeHandle);
        type WitnessFields = (OperationHandle, [usize; 7], Vec<SignatureFields>);
        type TerminalFields = (
            Vec<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
            Option<TypeHandle>,
        );
        type ProjectionFields<'a> = (
            KirPlironGraphV12<'a>,
            WitnessFields,
            u64,
            Option<TerminalFields>,
        );
        type PrivateProjectionFields<'a> = (ProjectionFields<'a>, &'a ());
        type RowFields = (Ptr<Operation>, u8);
        type AdmissionFields<'a> = (&'a (), &'a Context, &'a FuncOp, Vec<RowFields>, usize, u64);
        fn check<A, E>(name: &str) {
            assert_eq!(
                (size_of::<A>(), align_of::<A>()),
                (size_of::<E>(), align_of::<E>()),
                "{name} pinned layout premise; do not retune entry totals"
            );
        }
        assert_eq!((size_of::<usize>(), align_of::<usize>()), (8, 8));
        check::<Kind, u8>("private operation kind");
        check::<NativeRow, RowFields>("native private row");
        check::<NativeCanonicalPrivateAdmissionV1<'_>, AdmissionFields<'_>>("private admission");
        check::<super::super::trap_profile::NativeTrapModuleV1, TerminalFields>("terminal module");
        check::<NativeCanonicalRankedProjectionV1<'_>, ProjectionFields<'_>>("ranked projection");
        check::<NativeCanonicalPrivateProjectionV1<'_>, PrivateProjectionFields<'_>>(
            "private projection",
        );
    }
}
