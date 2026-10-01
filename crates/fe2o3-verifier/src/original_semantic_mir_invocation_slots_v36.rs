//! Exact original Alloca/frame index, constructed only inside retained visits.
//! Copied rows are inert. This owner-bound index supplies descriptor provenance,
//! not byte equivalence, source activation timing or target allocation lifetime.
use super::super::super::byte_function_v30::{ByteAllocationResolverV30, ByteAllocationSiteV30};
use super::super::invocations::InvocationPlan;
use super::{Error, Resource, Result, Writer, vector};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKirOperationCoordinateV1 as Operation,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceAllocationFrameV32 as Frame,
    ProductionSourceCorrespondenceV18 as Correspondence,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};
use std::{fmt::Write as _, mem::size_of};

type SourceKey = [usize; 5];

#[path = "original_semantic_mir_invocation_source_abi_v36.rs"]
mod source_abi;

pub(super) struct SourceSlots<'a, 'source> {
    relation: &'a Correspondence<'source>,
    operations: Vec<Operation>,
    frames: Vec<Option<Frame>>,
    source_order: Vec<(SourceKey, usize)>,
    abi: source_abi::SourceAbi,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR allocation descriptor census differs")
}

fn source_key(root: usize, instance: usize, local: u32, generation: Option<u32>) -> SourceKey {
    [
        root,
        instance,
        local as usize,
        usize::from(generation.is_some()),
        generation.unwrap_or(0) as usize,
    ]
}

fn locate<T: Ord>(
    keys: &[T],
    key: &T,
    budget: &mut Budget<'_>,
) -> std::result::Result<usize, SourceError> {
    let (mut lo, mut hi) = (0, keys.len());
    while lo < hi {
        budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        if keys[middle] < *key {
            lo = middle + 1;
        } else {
            hi = middle;
        }
    }
    budget.charge_work(1)?;
    if keys.get(lo) == Some(key) {
        Ok(lo)
    } else {
        Err(SourceError::Binding(
            "original allocation descriptor key absent",
        ))
    }
}

fn sort_source(rows: &mut [(SourceKey, usize)], out: &mut Writer<'_, '_>) -> Result<()> {
    fn sift(
        rows: &mut [(SourceKey, usize)],
        mut root: usize,
        end: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        loop {
            out.budget.charge_work(1)?;
            let mut child = root
                .checked_mul(2)
                .and_then(|at| at.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                break;
            }
            out.budget.charge_work(2)?;
            if child + 1 < end && rows[child].0 < rows[child + 1].0 {
                child += 1;
            }
            if rows[root].0 >= rows[child].0 {
                break;
            }
            out.budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
        Ok(())
    }
    let count = rows.len();
    for root in (0..count / 2).rev() {
        out.budget.charge_work(1)?;
        sift(rows, root, count, out)?;
    }
    for end in (1..count).rev() {
        out.budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, out)?;
    }
    for pair in rows.windows(2) {
        out.budget.charge_work(1)?;
        if pair[0].0 >= pair[1].0 {
            return Err(mismatch());
        }
    }
    Ok(())
}

impl<'a, 'source> SourceSlots<'a, 'source> {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        relation: &'a Correspondence<'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let source = plan.source(out)?;
        if !std::ptr::eq(source, relation.source(out.budget)?) {
            return Err(mismatch());
        }
        out.budget.reserve_storage(headers())?;
        let inventory = relation.inventory(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let mut count = 0usize;
        for operation in inventory.operations() {
            out.budget.charge_work(1)?;
            if matches!(
                operation.operation.kind,
                fe2o3_kernel_ir::OperationKind::Alloca { .. }
            ) {
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        let mut operations = vector(count, out)?;
        let mut frames = vector(count, out)?;
        let mut source_order = vector(count, out)?;
        for operation in inventory.operations() {
            out.budget.charge_work(2)?;
            if matches!(
                operation.operation.kind,
                fe2o3_kernel_ir::OperationKind::Alloca { .. }
            ) {
                if operations
                    .last()
                    .is_some_and(|last| *last >= operation.coordinate)
                {
                    return Err(mismatch());
                }
                operations.push(operation.coordinate);
                frames.push(None);
            }
        }
        for root in 0..roots {
            out.budget.charge_work(2)?;
            let physical = plan.root(root, out)?.physical;
            let function = inventory
                .functions()
                .get(physical)
                .ok_or_else(mismatch)?
                .coordinate;
            relation.visit_allocation_frames_v32(root, out.budget, |row, budget| {
                budget.charge_work(8)?;
                if row.root() != root || row.allocation().block.function != function {
                    return Err(SourceError::Binding(
                        "original allocation descriptor root differs",
                    ));
                }
                let at = locate(&operations, &row.allocation(), budget)?;
                if frames[at].is_some() || source_order.len() == count {
                    return Err(SourceError::Binding(
                        "original allocation descriptor duplicate",
                    ));
                }
                source_order.push((
                    source_key(root, row.instance(), row.local(), row.source_generation()),
                    at,
                ));
                frames[at] = Some(row);
                Ok(())
            })?;
        }
        if source_order.len() != count || frames.len() != count || operations.len() != count {
            return Err(mismatch());
        }
        for row in &frames {
            out.budget.charge_work(8)?;
            let row = row.as_ref().ok_or_else(mismatch)?;
            let instance = plan.instance(row.root(), row.instance(), out)?;
            let declaration = semantic
                .functions()
                .get(row.function().index() as usize)
                .ok_or_else(mismatch)?;
            if !instance.active
                || instance.function != row.function()
                || row.local() as usize >= instance.locals.len()
                || declaration
                    .locals()
                    .get(row.local() as usize)
                    .map(|local| local.ty())
                    != Some(row.semantic_type())
            {
                return Err(mismatch());
            }
        }
        sort_source(&mut source_order, out)?;
        let abi = source_abi::SourceAbi::derive(plan, relation, out)?;
        Ok(Self {
            relation,
            operations,
            frames,
            source_order,
            abi,
            required: out.budget.storage(),
        })
    }

    pub(super) fn check_source(
        &self,
        relation: &Correspondence<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        if !std::ptr::eq(self.relation, relation) {
            return Err(mismatch());
        }
        self.relation.source(out.budget)?;
        out.budget.charge_work(2)?;
        if self.operations.len() != self.frames.len()
            || self.frames.len() != self.source_order.len()
        {
            return Err(mismatch());
        }
        if out.budget.storage() < self.required {
            return Err(self
                .relation
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    pub(super) fn correspondence(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<&'a Correspondence<'source>> {
        self.check_source(self.relation, out)?;
        Ok(self.relation)
    }

    pub(super) fn descriptor_slice_bits(
        &self,
        ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<u32>> {
        self.check_source(self.relation, out)?;
        Ok(self.abi.slice(ty, out)?.map(|class| class.metadata_bits))
    }

    pub(super) fn descriptor_parameter(
        &self,
        root: usize,
        argument: usize,
        local: fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1>> {
        self.check_source(self.relation, out)?;
        self.abi.parameter(root, argument, local, out)
    }

    pub(super) fn witness_class(
        &self,
        ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<
        Option<(
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
            u16,
            bool,
            fe2o3_mir_model::semantic_mir_v1::SemanticDisjointIndexSpaceV1,
        )>,
    > {
        self.check_source(self.relation, out)?;
        self.abi.witness(ty, out)
    }

    pub(super) fn frame_by_allocation(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<&Frame> {
        self.check_source(self.relation, out)?;
        let at = locate(&self.operations, &operation, out.budget)?;
        self.frames
            .get(at)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)
    }

    pub(super) fn frame_by_source(
        &self,
        root: usize,
        instance: usize,
        local: u32,
        generation: Option<u32>,
        out: &mut Writer<'_, '_>,
    ) -> Result<&Frame> {
        self.descriptor_by_source(root, instance, local, generation, out)
            .map(|(_, row)| row)
    }

    pub(super) fn descriptor_by_source(
        &self,
        root: usize,
        instance: usize,
        local: u32,
        generation: Option<u32>,
        out: &mut Writer<'_, '_>,
    ) -> Result<(usize, &Frame)> {
        self.check_source(self.relation, out)?;
        let key = source_key(root, instance, local, generation);
        let (mut lo, mut hi) = (0, self.source_order.len());
        while lo < hi {
            out.budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.source_order[middle].0 < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        out.budget.charge_work(1)?;
        let (_, at) = self
            .source_order
            .get(lo)
            .filter(|row| row.0 == key)
            .ok_or_else(mismatch)?;
        Ok((
            *at,
            self.frames
                .get(*at)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?,
        ))
    }

    pub(super) fn instance_descriptors(
        &self,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<std::ops::Range<usize>> {
        self.check_source(self.relation, out)?;
        self.relation
            .source(out.budget)?
            .instance(root, instance, out.budget)?;
        let key = (root, instance);
        let bound = |upper: bool, out: &mut Writer<'_, '_>| -> Result<usize> {
            let (mut lo, mut hi) = (0, self.source_order.len());
            while lo < hi {
                out.budget.charge_work(1)?;
                let middle = lo + (hi - lo) / 2;
                let row = self.source_order[middle].0;
                if (row[0], row[1]) < key || (upper && (row[0], row[1]) == key) {
                    lo = middle + 1;
                } else {
                    hi = middle;
                }
            }
            Ok(lo)
        };
        let start = bound(false, out)?;
        let end = bound(true, out)?;
        Ok(start..end)
    }

    pub(super) fn descriptor_in_source_order(
        &self,
        position: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<(usize, &Frame)> {
        self.check_source(self.relation, out)?;
        out.budget.charge_work(2)?;
        let (_, at) = self.source_order.get(position).ok_or_else(mismatch)?;
        Ok((
            *at,
            self.frames
                .get(*at)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?,
        ))
    }

    pub(super) fn legacy_descriptor_by_source(
        &self,
        root: usize,
        instance: usize,
        local: u32,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(usize, &Frame)>> {
        self.check_source(self.relation, out)?;
        let key = source_key(root, instance, local, None);
        let (mut lo, mut hi) = (0, self.source_order.len());
        while lo < hi {
            out.budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.source_order[middle].0 < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        out.budget.charge_work(3)?;
        let Some((found, at)) = self.source_order.get(lo) else {
            return Ok(None);
        };
        if found[..3] != key[..3] {
            return Ok(None);
        }
        if *found != key
            || self
                .source_order
                .get(lo + 1)
                .is_some_and(|row| row.0[..3] == key[..3])
        {
            return Err(Error::Statement(
                "original object lifetime requires exact statement generation",
            ));
        }
        Ok(Some((
            *at,
            self.frames
                .get(*at)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?,
        )))
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check_source(self.relation, out)?;
        for (ordinal, row) in self.frames.iter().enumerate() {
            out.budget.charge_work(12)?;
            let row = row.as_ref().ok_or_else(mismatch)?;
            let operation = row.allocation();
            write!(out, "open spec fn invocation_source_slot_{ordinal}_v36() -> InvocationSourceSlotV36 {{ InvocationSourceSlotV36 {{ root: {}int, instance: {}int, owner: {}int, local: {}int, semantic_type: {}int, source_generation: ", row.root(), row.instance(), row.function().index(), row.local(), row.semantic_type().index()).map_err(|_| out.error())?;
            emit_option(row.source_generation(), out)?;
            write!(out, ", storage_layout: ").map_err(|_| out.error())?;
            emit_option(row.layout().map(|layout| layout.0), out)?;
            write!(out, ", site: MemorySourceOperationV30 {{ function: {}int, block: {}int, operation: {}int }}, extent: {}int, alignment: {}int }} }}\n", operation.block.function.0, operation.block.block, operation.operation, row.bytes(), row.alignment()).map_err(|_| out.error())?;
        }
        write!(
            out,
            "open spec fn invocation_source_slot_count_v36() -> nat {{ {}nat }}\n",
            self.frames.len()
        )
        .map_err(|_| out.error())?;
        Ok(())
    }
}

impl ByteAllocationResolverV30 for SourceSlots<'_, '_> {
    fn check_owner(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check_source(self.relation, out)?;
        out.budget.charge_work(1)?;
        if !std::ptr::eq(self.relation.inventory(out.budget)?.owner(), owner) {
            return Err(mismatch());
        }
        Ok(())
    }

    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        let frame = self.frame_by_allocation(operation, out)?;
        let source = self.relation.source(out.budget)?;
        let (owner, physical) = source.root(frame.root(), out.budget)?;
        out.budget.charge_work(3)?;
        if operation.block.function.0 as usize != physical || frame.allocation() != operation {
            return Err(mismatch());
        }
        // A hoisted callee allocation executes in the physical root's frame.
        // Its source declaring frame is retained separately in the descriptor.
        Ok(ByteAllocationSiteV30 {
            original: frame.allocation(),
            physical_root_owner: owner.index() as usize,
        })
    }
}

fn emit_option(value: Option<u32>, out: &mut Writer<'_, '_>) -> Result<()> {
    match value {
        Some(value) => write!(out, "Some({value}int)"),
        None => write!(out, "None"),
    }
    .map_err(|_| out.error())
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceSlots<'_, '_>>()
        + h::<Frame>()
        + h::<Vec<Operation>>()
        + h::<Vec<Option<Frame>>>()
        + h::<Vec<(SourceKey, usize)>>()
        + h::<SourceKey>()
        + h::<Operation>()
        + h::<ByteAllocationSiteV30>()
        + h::<std::ops::Range<usize>>()
        + h::<(usize, &Frame)>()
        + h::<(
            fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
            usize,
        )>()
        + h::<&InvocationPlan<'_, '_>>()
        + h::<&Correspondence<'_>>()
        + h::<&super::super::super::Inventory<'_>>()
        + h::<&mut Writer<'_, '_>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 4 * size_of::<std::result::Result<usize, SourceError>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    const LIMIT: usize = 100_000_000;

    fn run_slots(
        work: usize,
        storage: usize,
        examine: impl FnOnce(&mut SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
    ) -> (Result<()>, usize, usize, usize) {
        super::super::super::invocations::tests::run_allocation_variant(
            work,
            storage,
            |plan, out| {
                let source = plan.source(out)?;
                let owner = source.canonical(out.budget)?;
                let (inventory, receipt) =
                    super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
                out.budget.reserve_storage(receipt.retained_storage())?;
                let result = source.with_ranked_correspondence_v18(
                    &inventory,
                    out.budget,
                    |relation, budget| {
                        let mut writer = Writer::new(budget)?;
                        let mut slots = SourceSlots::derive(plan, relation, &mut writer)?;
                        examine(&mut slots, &mut writer)
                    },
                );
                drop(inventory);
                if result.is_ok() {
                    out.budget.release_storage(receipt.retained_storage())?;
                }
                result
            },
        )
    }

    #[test]
    fn original_mir_allocation_index_consumes_genuine_complete_two_root_objects() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            assert_eq!(slots.operations.len(), 2);
            assert_eq!(slots.frames.len(), 2);
            assert_eq!(slots.source_order.len(), 2);
            for root in 0..2 {
                let row = slots
                    .frames
                    .iter()
                    .flatten()
                    .find(|row| row.root() == root)
                    .copied()
                    .unwrap();
                assert_eq!(
                    (row.instance(), row.function().index(), row.local()),
                    (0, root as u32, 4)
                );
                assert_eq!((row.bytes(), row.alignment()), (8, 4));
                assert!(row.layout().is_some());
                assert!(row.source_generation().is_some());
                let by_operation = slots.frame_by_allocation(row.allocation(), out)?;
                assert_eq!(*by_operation, row);
                assert_eq!(
                    *slots.frame_by_source(root, 0, 4, row.source_generation(), out)?,
                    row
                );
            }
            slots.emit(out)?;
            assert_eq!(out.text.matches("-> InvocationSourceSlotV36").count(), 2);
            assert!(
                out.text
                    .contains("invocation_source_slot_count_v36() -> nat { 2nat }")
            );
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_source_abi_cache_never_infers_descriptor_slice_from_tuple_shape() {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticTypeIdV1};
        run_slots(LIMIT, LIMIT, |slots, out| {
            for ty in 0..3 {
                assert_eq!(
                    slots.descriptor_slice_bits(SemanticTypeIdV1::from_index(ty), out)?,
                    None
                );
            }
            for root in 0..2 {
                for argument in 0..2 {
                    assert_eq!(
                        slots.descriptor_parameter(
                            root,
                            argument,
                            SemanticLocalIdV1::from_index(argument as u32 + 1),
                            out,
                        )?,
                        None
                    );
                }
            }
            assert!(
                slots
                    .descriptor_parameter(0, 0, SemanticLocalIdV1::from_index(2), out)
                    .is_err()
            );
            assert!(
                slots
                    .descriptor_parameter(2, 0, SemanticLocalIdV1::from_index(1), out)
                    .is_err()
            );
            assert!(
                slots
                    .descriptor_parameter(0, 2, SemanticLocalIdV1::from_index(3), out)
                    .is_err()
            );
            assert!(
                slots
                    .descriptor_slice_bits(SemanticTypeIdV1::from_index(3), out)
                    .is_err()
            );
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_source_abi_cached_queries_have_constant_work_and_no_retained_allocation() {
        use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1;
        run_slots(LIMIT, LIMIT, |slots, out| {
            let storage = out.budget.storage();
            let before = out.budget.work();
            slots.descriptor_slice_bits(SemanticTypeIdV1::from_index(2), out)?;
            let one = out.budget.work() - before;
            let before = out.budget.work();
            for _ in 0..64 {
                slots.descriptor_slice_bits(SemanticTypeIdV1::from_index(2), out)?;
            }
            assert!(one > 0);
            assert_eq!(out.budget.work() - before, 64 * one);
            assert_eq!(out.budget.storage(), storage);
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_index_refuses_wrong_root_instance_and_implicit_generation() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            let row = *slots
                .frames
                .iter()
                .flatten()
                .find(|row| row.root() == 0)
                .unwrap();
            assert!(
                slots
                    .frame_by_source(2, row.instance(), row.local(), row.source_generation(), out)
                    .is_err()
            );
            assert!(
                slots
                    .frame_by_source(row.root(), 1, row.local(), row.source_generation(), out)
                    .is_err()
            );
            assert!(
                slots
                    .frame_by_source(row.root(), row.instance(), row.local(), None, out)
                    .is_err()
            );
            assert!(matches!(
                slots.legacy_descriptor_by_source(row.root(), row.instance(), row.local(), out),
                Err(Error::Statement(
                    "original object lifetime requires exact statement generation"
                ))
            ));
            assert!(slots.legacy_descriptor_by_source(0, 0, 1, out)?.is_none());
            let other = slots
                .frames
                .iter()
                .flatten()
                .find(|row| row.root() == 1)
                .unwrap();
            assert_ne!(row.allocation(), other.allocation());
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_index_refuses_same_coordinates_from_foreign_retained_source() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            let outer_coordinates = slots.operations.clone();
            run_slots(LIMIT, LIMIT, |foreign, writer| {
                assert_eq!(outer_coordinates, foreign.operations);
                assert!(!std::ptr::eq(slots.relation, foreign.relation));
                let before = (writer.budget.work(), writer.budget.storage());
                assert!(matches!(
                    slots.check_source(foreign.relation, writer),
                    Err(Error::Statement(
                        "original MIR allocation descriptor census differs"
                    ))
                ));
                assert_eq!((writer.budget.work(), writer.budget.storage()), before);
                Ok(())
            })
            .0
            .unwrap();
            slots.check_source(slots.relation, out)
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_index_refuses_missing_complete_census_row() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            let row = slots.frames.pop().unwrap();
            assert!(slots.emit(out).is_err());
            assert!(out.text.is_empty());
            slots.frames.push(row);
            slots.emit(out)
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_byte_resolver_uses_exact_physical_root_owner() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            let relation = slots.correspondence(out)?;
            let owner = relation.inventory(out.budget)?.owner();
            slots.check_owner(owner, out)?;
            for ordinal in 0..slots.operations.len() {
                let operation = slots.operations[ordinal];
                let frame = *slots.frame_by_allocation(operation, out)?;
                let expected = relation
                    .source(out.budget)?
                    .root(frame.root(), out.budget)?;
                let site = slots.site(operation, out)?;
                assert_eq!(site.original, operation);
                assert_eq!(site.physical_root_owner, expected.0.index() as usize);
                assert_eq!(site.original.block.function.0 as usize, expected.1);
                let mut absent = operation;
                absent.operation = u32::MAX;
                assert!(slots.site(absent, out).is_err());
            }
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_byte_resolver_refuses_distinct_same_coordinate_owner() {
        run_slots(LIMIT, LIMIT, |slots, out| {
            let coordinates = slots.operations.clone();
            run_slots(LIMIT, LIMIT, |foreign, writer| {
                assert_eq!(coordinates, foreign.operations);
                let foreign_owner = foreign.relation.inventory(writer.budget)?.owner();
                assert!(matches!(
                    slots.check_owner(foreign_owner, out),
                    Err(Error::Statement(
                        "original MIR allocation descriptor census differs"
                    ))
                ));
                Ok(())
            })
            .0
            .unwrap();
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_byte_resolver_drives_genuine_scalar_memory_function() {
        use super::super::super::super::byte_function_v30::ByteFunctionV30;
        use fe2o3_kernel_ir::{FormalIndexWidth, OperationKind};
        super::super::super::invocations::tests::run_scalar_allocation_variant(
            LIMIT,
            LIMIT,
            |plan, out| {
                let source = plan.source(out)?;
                let owner = source.canonical(out.budget)?;
                let (inventory, receipt) =
                    super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
                out.budget.reserve_storage(receipt.retained_storage())?;
                let result = source.with_ranked_correspondence_v18(
                    &inventory,
                    out.budget,
                    |relation, budget| {
                        let mut writer = Writer::new(budget)?;
                        let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                        assert_eq!(slots.operations.len(), 2);
                        let (physical, storage) =
                            fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                                &inventory,
                                fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                                    max_boundaries: 4096,
                                },
                                writer.budget,
                            )?;
                        writer.budget.reserve_storage(storage.retained_storage())?;
                        for root in 0..2 {
                            let function = plan.root(root, &writer)?.physical;
                            let row = &inventory.functions()[function];
                            let operations = &inventory.operations()[row.operations.clone()];
                            assert!(operations.iter().any(|row| matches!(
                                row.operation.kind,
                                OperationKind::Alloca { .. }
                            )));
                            assert!(operations.iter().any(|row| matches!(
                                row.operation.kind,
                                OperationKind::Load { .. }
                            )));
                            assert!(operations.iter().any(|row| matches!(
                                row.operation.kind,
                                OperationKind::Store { .. }
                            )));
                            let byte = ByteFunctionV30::derive(
                                &inventory,
                                &physical,
                                row.coordinate,
                                FormalIndexWidth::Bits64,
                                &slots,
                                &mut writer,
                            )?;
                            byte.emit(root, &mut writer)?;
                        }
                        assert!(writer.text.contains("MemoryOperationEffectV30::Allocate"));
                        assert!(writer.text.contains("MemoryOperationEffectV30::Read"));
                        assert!(writer.text.contains("MemoryOperationEffectV30::Write"));
                        assert!(writer.text.contains("byte_micro_step_0"));
                        assert!(writer.text.contains("byte_micro_step_1"));
                        assert!(!writer.text.contains("assume("));
                        drop(physical);
                        writer.budget.release_storage(storage.retained_storage())?;
                        Ok(())
                    },
                );
                drop(inventory);
                if result.is_ok() {
                    out.budget.release_storage(receipt.retained_storage())?;
                }
                result
            },
        )
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_allocation_index_has_exact_and_one_short_full_resources() {
        let run = |work, storage| run_slots(work, storage, |slots, out| slots.emit(out));
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        run(measured.1, measured.3).0.unwrap();
        assert!(run(measured.1 - 1, measured.3).0.is_err());
        assert!(run(measured.1, measured.3 - 1).0.is_err());
    }

    #[test]
    fn original_mir_allocation_source_keys_keep_root_instance_and_static_generation() {
        let key = source_key(2, 3, 4, None);
        for other in [
            source_key(1, 3, 4, None),
            source_key(2, 2, 4, None),
            source_key(2, 3, 5, None),
            source_key(2, 3, 4, Some(0)),
            source_key(2, 3, 4, Some(1)),
        ] {
            assert_ne!(key, other);
        }
        assert!(source_key(2, 3, 4, None) < source_key(2, 3, 4, Some(0)));
        assert!(source_key(2, 3, 4, Some(0)) < source_key(2, 3, 4, Some(1)));
    }

    #[test]
    fn original_mir_allocation_lookup_has_independent_four_work_boundary() {
        let keys = [1u32, 3, 5, 7, 9];
        for limit in [4, 3] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 11);
            budget.reserve_storage(11).unwrap();
            let result = locate(&keys, &1, &mut budget);
            // Probes at positions 2, 1 and 0, then one equality check.
            if limit == 4 {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(budget.work(), 3 + 1);
            } else {
                assert!(matches!(
                    result,
                    Err(SourceError::Resource(Resource::Work(_)))
                ));
            }
            assert_eq!(budget.storage(), 11);
        }
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            locate(&keys, &8, &mut budget),
            Err(SourceError::Binding(_))
        ));
        assert_eq!(budget.work(), 4);
    }

    #[test]
    fn original_mir_allocation_source_sort_has_independent_twelve_work_boundary() {
        for limit in [12, 11] {
            let mut rows = [
                (source_key(3, 0, 0, None), 30),
                (source_key(1, 0, 0, None), 10),
                (source_key(2, 0, 0, None), 20),
            ];
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, SOURCE_LIMIT + 17);
            budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
            let mut writer = Writer::new(&mut budget).unwrap();
            let before = writer.budget.storage();
            let result = sort_source(&mut rows, &mut writer);
            // Heap build 1+1+2; extraction at end=2: 1+1+2;
            // extraction at end=1: 1+1; two distinctness comparisons.
            if limit == 12 {
                result.unwrap();
                assert_eq!(writer.budget.work(), 4 + 4 + 2 + 2);
                assert_eq!(rows.map(|row| row.1), [10, 20, 30]);
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            }
            assert_eq!(writer.budget.storage(), before);
            assert!(writer.text.is_empty());
        }
    }

    #[test]
    fn original_mir_allocation_index_headers_have_independent_field_envelope() {
        type Fields<'a, 's> = (
            &'a Correspondence<'s>,
            Vec<Operation>,
            Vec<Option<Frame>>,
            Vec<(SourceKey, usize)>,
            source_abi::SourceAbi,
            usize,
        );
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(
            size_of::<SourceSlots<'_, '_>>(),
            size_of::<Fields<'_, '_>>()
        );
        assert_eq!(
            headers(),
            size_of::<Fields<'_, '_>>()
                + 2 * size_of::<Result<SourceSlots<'_, '_>>>()
                + h::<Frame>()
                + h::<Vec<Operation>>()
                + h::<Vec<Option<Frame>>>()
                + h::<Vec<(SourceKey, usize)>>()
                + h::<SourceKey>()
                + h::<Operation>()
                + h::<ByteAllocationSiteV30>()
                + h::<std::ops::Range<usize>>()
                + h::<(usize, &Frame)>()
                + h::<(
                    fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
                    usize
                )>()
                + 4 * h::<&()>()
                + 32 * size_of::<usize>()
                + 24 * size_of::<&()>()
                + 4 * size_of::<std::result::Result<usize, SourceError>>()
        );
    }
}
