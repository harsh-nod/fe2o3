//! Exact V18 physical occurrences on the retained native graph, not source roles.
use super::*;
use crate::production_analysis::canonical_ranked_checks_v1::{
    CanonicalRankedPolicyFailureV1 as Failure,
    private::PrivateOperationKindV1 as Kind,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18, CheckedCanonicalKirPrivateMemoryV18,
};
use pliron::builtin::op_interfaces::OneRegionInterface;

// Only the graph visitor below constructs this token after a complete
// coordinate/schema census. No V12 graph, owner or certificate is constructed.
pub(crate) struct NativeCanonicalPrivateAdmissionV18<'a> {
    identity: &'a NativeLifecycleIdentityAdmissionV18<'a>,
    coordinates: &'a HashMap<Ptr<Operation>, KirBridgeCoordinateV1>,
    physical: &'a CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
}

impl NativeCanonicalPrivateAdmissionV18<'_> {
    fn inventory(&self) -> &CanonicalKirInventoryV18<'_> {
        self.physical.inventory()
    }

    fn ordinal_for(&self, coordinate: KirBridgeCoordinateV1) -> Option<usize> {
        let KirBridgeCoordinateV1::Operation { function, block, operation } = coordinate else {
            return None;
        };
        if function as usize != self.identity.ordinal { return None; }
        let function_row = self.inventory().functions().get(function as usize)?;
        let block_index = function_row.blocks.start.checked_add(block as usize)?;
        if block_index >= function_row.blocks.end { return None; }
        let block_row = self.inventory().blocks().get(block_index)?;
        if block_row.coordinate.function.0 != function || block_row.coordinate.block != block {
            return None;
        }
        let index = block_row.operations.start.checked_add(operation as usize)?;
        if index >= block_row.operations.end { return None; }
        let row = self.inventory().operations().get(index)?;
        (row.coordinate.block == block_row.coordinate && row.coordinate.operation == operation)
            .then_some(index)
    }

    fn memory_kind(&self, index: usize) -> Option<Kind> {
        use fe2o3_kernel_ir::StorageOperationV1 as Storage;
        if !self.physical.operation(index) { return None; }
        Some(match &self.inventory().operations().get(index)?.operation.kind {
            OperationKind::Alloca { .. } => Kind::Allocate,
            OperationKind::Storage(Storage::ReadValue { .. }) => Kind::Read,
            OperationKind::Storage(Storage::WriteValue { .. }) => Kind::Write,
            _ => return None,
        })
    }

    fn checked_memory_carrier(
        &self,
        pointer: Ptr<Operation>,
        index: usize,
        profile: storage_v18::ProfileV18<'_>,
    ) -> Result<(), Failure> {
        let context = self.identity.context();
        let row = self.inventory().operations().get(index).ok_or(Failure::ExactGraph)?;
        let expected = row.operation;
        let raw = pointer.deref(context);
        let operands = expected.kind.operand_count();
        if raw.num_regions() != 0 || raw.get_num_successors() != 0
            || operands > 2 || raw.get_num_operands() != operands
            || raw.get_num_results() != expected.results.len()
            || expected.results.len() > 1
        { return Err(Failure::NativeSchema); }
        let mut ordinal = 0usize;
        expected.kind.try_visit_operands(|original| {
            if self.identity.origins.values.get(&raw.get_operand(ordinal)) != Some(&original) {
                return Err(Failure::ExactGraph);
            }
            ordinal += 1;
            Ok(())
        })?;
        for (ordinal, original) in expected.results.iter().enumerate() {
            let actual = raw.get_result(ordinal);
            if self.identity.origins.values.get(&actual) != Some(&original.id) {
                return Err(Failure::ExactGraph);
            }
            // The physical checker admits only scalar results and pointers to
            // one scalar/storage layout here. Bound the decoder independently.
            if !matches!(&original.ty, Type::Scalar(_))
                && !matches!(&original.ty, Type::Pointer(p)
                    if matches!(p.pointee.as_ref(), Type::Scalar(_) | Type::StorageObject(_)))
            { return Err(Failure::NativeSchema); }
            let handle = actual.get_type(context);
            let raw_type = handle.deref(context);
            let leaf = match &original.ty {
                Type::Scalar(_) => handle,
                Type::Pointer(_) => raw_type.downcast_ref::<PlironPointerType>()
                    .ok_or(Failure::NativeSchema)?.pointee(),
                _ => return Err(Failure::NativeSchema),
            };
            let leaf_type = leaf.deref(context);
            // Bound the actual decoder input too, not just the expected type.
            if leaf_type.downcast_ref::<PlironPointerType>().is_some()
                || leaf_type.downcast_ref::<PlironSliceType>().is_some()
                || leaf_type.downcast_ref::<PlironFixedVectorTypeV12>().is_some()
            { return Err(Failure::NativeSchema); }
            let actual_type = profile.decode_type(context, handle, 0)
                .map_err(|_| Failure::NativeSchema)?;
            if actual_type != original.ty { return Err(Failure::ExactGraph); }
        }
        let keys = match &expected.kind {
            OperationKind::Storage(_) => &[
                "gpu_storage_kind_v18", "gpu_storage_selector_v18", "gpu_storage_overlap_v18",
                "gpu_storage_read_v18", "gpu_storage_write_v18",
            ][..],
            _ => canonical_ranked_v1::private_profile::keys(context, pointer)
                .ok_or(Failure::NativeSchema)?,
        };
        // The decoder requires each fixed named field; equal cardinality then
        // excludes extras without allocating or rendering attribute names.
        if raw.attributes.0.len() != keys.len() { return Err(Failure::NativeSchema); }
        match &expected.kind {
            OperationKind::Alloca { .. } => {
                let operation = Operation::get_op::<PreservedOperationOp>(pointer, context)
                    .ok_or(Failure::NativeSchema)?;
                if operation.kind(context) != Some(PreservedOperationKindAttr::Alloca)
                    || self.identity.origins.preserved_operations.get(&pointer) != Some(&expected.kind)
                { return Err(Failure::ExactGraph); }
            }
            OperationKind::Storage(_) => {
                // The storage extractor copies a fixed descriptor and at most
                // two operands; avoid the generic preserved Vec/map remapper.
                let extracted = storage_v18::extract(context, pointer,
                    &self.identity.origins.values, self.identity.origins)
                    .map_err(|_| Failure::ExactGraph)?;
                if extracted != expected.kind { return Err(Failure::ExactGraph); }
            }
            _ => return Err(Failure::NativeSchema),
        }
        Ok(())
    }

    fn validate_complete(
        &self,
        profile: storage_v18::ProfileV18<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        let context = self.identity.context();
        let function = self.identity.function();
        if !self.authenticate(context, function) { return Err(Failure::Mutation); }
        let function_row = self.inventory().functions().get(self.identity.ordinal)
            .ok_or(Failure::ExactGraph)?;
        let region = function.get_region(context).deref(context);
        let mut blocks = region.iter(context);
        for source in &self.inventory().blocks()[function_row.blocks.clone()] {
            budget.charge_work(1)?;
            let block = blocks.next().ok_or(Failure::NativeSchema)?;
            if self.identity.origins.blocks.get(&block)
                != Some(&(self.identity.ordinal, source.block.id))
            { return Err(Failure::ExactGraph); }
            let raw_block = block.deref(context);
            let mut operations = raw_block.iter(context);
            for index in source.operations.clone() {
                let row = &self.inventory().operations()[index];
                let pointer = operations.next().ok_or(Failure::NativeSchema)?;
                let expected = KirBridgeCoordinateV1::Operation {
                    function: row.coordinate.block.function.0,
                    block: row.coordinate.block.block, operation: row.coordinate.operation,
                };
                budget.charge_work(self.lookup_bound().and_then(|n| n.checked_mul(32)).ok_or(ResourceError::Arithmetic)?)?;
                if self.coordinates.get(&pointer) != Some(&expected) {
                    return Err(Failure::ExactGraph);
                }
                if self.physical.operation(index) {
                    self.memory_kind(index).ok_or(Failure::NativeSchema)?;
                    // Fixed decoder/operand copies are transient and cannot
                    // outlive this complete carrier check.
                    self.checked_memory_carrier(pointer, index, profile)?;
                } else {
                    self.identity.operation(context, pointer).ok_or(Failure::NativeSchema)?;
                }
            }
            let terminal = operations.next().ok_or(Failure::NativeSchema)?;
            budget.charge_work(self.lookup_bound().and_then(|n| n.checked_mul(32)).ok_or(ResourceError::Arithmetic)?)?;
            if self.coordinates.get(&terminal) != Some(&KirBridgeCoordinateV1::Terminator {
                function: source.coordinate.function.0, block: source.coordinate.block,
            }) || self.identity.operation(context, terminal).is_none()
                || operations.next().is_some()
            { return Err(Failure::NativeSchema); }
        }
        if blocks.next().is_some() { return Err(Failure::NativeSchema); }
        Ok(())
    }

    fn lookup_bound(&self) -> Option<usize> {
        self.coordinates.len().checked_add(self.identity.origins.values.len())?
            .checked_add(self.identity.origins.preserved_operations.len())?
            .checked_add(self.identity.origins.blocks.len())?
            .checked_add(self.identity.origins.functions.len())?
            .checked_add(self.identity.origins.preserved_terminators.len())?
            .checked_add(64)
    }
}

impl super::super::native_private_seal::Sealed for NativeCanonicalPrivateAdmissionV18<'_> {}
impl NativePrivateInputV1 for NativeCanonicalPrivateAdmissionV18<'_> {
    fn context(&self) -> &Context { self.identity.context() }
    fn function(&self) -> &FuncOp { self.identity.function() }
    fn ordinal(&self) -> usize { self.identity.ordinal }
    fn epoch(&self) -> u64 { self.identity.epoch }
    fn operation_count(&self) -> usize { self.identity.operations }
    fn authenticate(&self, context: &Context, function: &FuncOp) -> bool {
        self.identity.authenticate(context, function)
            && self.physical.is_for(self.inventory())
    }
    fn operation(&self, context: &Context, pointer: Ptr<Operation>) -> Option<Kind> {
        if !self.authenticate(context, self.function()) { return None; }
        // Query the exact imported pointer before dereferencing it. A foreign
        // pointer is not made safe by equal ordinal/type/bit content.
        let coordinate = *self.coordinates.get(&pointer)?;
        if let Some(index) = self.ordinal_for(coordinate) {
            if self.physical.operation(index) { return self.memory_kind(index); }
            self.identity.operation(context, pointer)?;
            return Some(if matches!(self.inventory().operations()[index].operation.kind,
                OperationKind::Execution(_)) { Kind::LifecycleV18 } else { Kind::Scalar });
        }
        if !matches!(coordinate, KirBridgeCoordinateV1::Terminator { function, .. }
            if function as usize == self.ordinal()) { return None; }
        self.identity.operation(context, pointer)?;
        Some(if self.identity.unreachable(context, pointer) { Kind::UnreachableV18 } else { Kind::Scalar })
    }
    fn attribute(&self, context: &Context, pointer: Ptr<Operation>, key: &str, dialect: &str, name: &str) -> bool {
        let Some(kind) = self.operation(context, pointer) else { return false; };
        if matches!(kind, Kind::LifecycleV18 | Kind::UnreachableV18) {
            return self.identity.attribute(context, pointer, key, dialect, name);
        }
        if dialect != "gpu" { return false; }
        matches!((kind, key, name),
            (Kind::Allocate, "gpu_preserved_operation_kind", "preserved_operation_kind")
            | (Kind::Read, "gpu_load_alignment", "memory_alignment")
            | (Kind::Read, "gpu_load_volatile", "volatile")
            | (Kind::Write, "gpu_store_alignment", "memory_alignment")
            | (Kind::Write, "gpu_store_volatile", "volatile"))
            || (matches!(kind, Kind::Read | Kind::Write) && matches!((key, name),
                ("gpu_storage_kind_v18", "storage_kind_v18")
                | ("gpu_storage_selector_v18", "storage_ordinal_v18")
                | ("gpu_storage_overlap_v18", "storage_overlap_v18")
                | ("gpu_storage_read_v18" | "gpu_storage_write_v18", "storage_access_v18")))
    }
    fn identity_lookup_work(&self) -> Option<usize> {
        self.operation_count().checked_add(1)?.checked_mul(self.lookup_bound()?)?.checked_mul(512)
    }
    fn stage_lookup_work(&self) -> Option<usize> {
        self.operation_count().checked_add(1)?.checked_mul(self.lookup_bound()?)?.checked_mul(32)
    }
    fn run_fixed(&self, limits: crate::ProductionAnalysisResourceLimitsV1,
        receipt: Option<&mut crate::invocation_receipt_v1::InvocationReceiptV1>)
        -> Result<crate::canonical_private_v1::CanonicalPrivatePipelineOutcomeV1,
            crate::PipelineErrorV1>
    {
        crate::canonical_private_v1::run_v18(self, limits, receipt)
    }
}

impl KirPlironGraphV18<'_> {
    pub(crate) fn visit_private_policy_functions_v18(
        &self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        epoch: u64,
        budget: &mut Budget<'_>,
        mut consume: impl FnMut(usize, &NativeCanonicalPrivateAdmissionV18<'_>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.validate_custody(budget)?;
        self.check_ranked_policy_epoch_v18(epoch)?;
        if !std::ptr::eq(physical.inventory().owner(), self.profile.owner()) {
            return Err(Failure::ExactGraph);
        }
        budget.reserve_storage(native_private_headers_v18(
            std::mem::size_of_val(&consume), std::mem::align_of_val(&consume),
        )?)?;
        budget.charge_work(self.session.operations.len().checked_add(1).ok_or(ResourceError::Arithmetic)?)?;
        let context = &self.session.context;
        let root = self.session.operations[&self.root.identity];
        let region = root.deref(context).get_region(0);
        let block = region.deref(context).iter(context).next().ok_or(Failure::NativeSchema)?;
        let live_block = block.deref(context);
        let mut live_functions = live_block.iter(context);
        for (ordinal, source) in self.profile.owner().module().functions.iter().enumerate() {
            budget.charge_work(1)?;
            let Some(source_body) = source.body.as_ref() else { continue; };
            let pointer = live_functions.next().ok_or(Failure::NativeSchema)?;
            budget.charge_work(self.origins.functions.len().checked_add(1).ok_or(ResourceError::Arithmetic)?)?;
            if self.origins.functions.get(&pointer) != Some(&ordinal) {
                return Err(Failure::ExactGraph);
            }
            let function = Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            self.check_ranked_policy_epoch_v18(epoch)?;
            budget.charge_work(source_body.blocks.len())?;
            let operations = source_body.blocks.iter().try_fold(0usize, |count, block| {
                count.checked_add(block.operations.len())?.checked_add(1)
            }).ok_or(ResourceError::Arithmetic)?;
            let identity = NativeLifecycleIdentityAdmissionV18 {
                context, function: &function, origins: &self.origins, ordinal, epoch, operations,
            };
            let input = NativeCanonicalPrivateAdmissionV18 {
                identity: &identity, coordinates: &self.coordinates, physical,
            };
            input.validate_complete(self.profile, budget)?;
            consume(ordinal, &input)?;
            self.check_ranked_policy_epoch_v18(epoch)?;
        }
        budget.charge_work(1)?;
        if live_functions.next().is_some() { return Err(Failure::NativeSchema); }
        self.validate_custody(budget)?;
        self.check_ranked_policy_epoch_v18(epoch)
    }
}

fn native_private_headers_v18(capture: usize, alignment: usize) -> Result<usize, Failure> {
    use fe2o3_kernel_analysis::{CanonicalKirFunctionRefV1, CanonicalKirBlockRefV1, CanonicalKirOperationRefV1};
    use fe2o3_kernel_ir::{Function, FunctionBody, ValueDef, PointerType};
    use std::{cell::Ref, mem::size_of, ops::Range};
    fn h<T>() -> Result<usize, Failure> {
        size_of::<T>().checked_add(size_of::<Result<T, Failure>>().checked_mul(2)
            .ok_or(ResourceError::Arithmetic)?).ok_or_else(|| ResourceError::Arithmetic.into())
    }
    // Reusable fixed slots for each walk/census/carrier frame. Type decoding is
    // independently closed above at depth two: at most one pointer Box is live.
    type Walk<'a> = (
        &'a Context, Ptr<Operation>, Ptr<pliron::region::Region>, Ptr<BasicBlock>,
        Ref<'a, Operation>, Ref<'a, pliron::region::Region>, Ref<'a, BasicBlock>,
        pliron::linked_list::Iter<'a, BasicBlock>, pliron::linked_list::Iter<'a, Operation>,
        std::iter::Enumerate<std::slice::Iter<'a, Function>>,
        Option<(usize, &'a Function)>, &'a Function, &'a FunctionBody,
        Option<Ptr<Operation>>, Option<&'a usize>, FuncOp, Option<FuncOp>,
        std::slice::Iter<'a, fe2o3_kernel_ir::BasicBlock>,
        Option<&'a fe2o3_kernel_ir::BasicBlock>, Option<usize>,
    );
    type Census<'a> = (
        &'a Context, &'a FuncOp, &'a CanonicalKirFunctionRefV1<'a>,
        Option<&'a CanonicalKirFunctionRefV1<'a>>, Range<usize>,
        &'a CanonicalKirBlockRefV1<'a>, std::slice::Iter<'a, CanonicalKirBlockRefV1<'a>>,
        Option<&'a CanonicalKirBlockRefV1<'a>>, Ref<'a, pliron::region::Region>,
        Ref<'a, BasicBlock>, pliron::linked_list::Iter<'a, BasicBlock>,
        pliron::linked_list::Iter<'a, Operation>, Option<Ptr<BasicBlock>>, Ptr<BasicBlock>,
        Option<Ptr<Operation>>, Ptr<Operation>, Option<&'a (usize, BlockId)>,
        &'a CanonicalKirOperationRefV1<'a>, Option<&'a CanonicalKirOperationRefV1<'a>>,
        Option<&'a KirBridgeCoordinateV1>, KirBridgeCoordinateV1, Range<usize>,
        Option<usize>, Option<Kind>, Option<()>,
    );
    type Carrier<'a> = (
        &'a Context, &'a CanonicalKirOperationRefV1<'a>,
        Option<&'a CanonicalKirOperationRefV1<'a>>, &'a KirOperation,
        Ref<'a, Operation>, usize, usize, Value, Option<&'a ValueId>,
        std::iter::Enumerate<std::slice::Iter<'a, ValueDef>>, Option<(usize, &'a ValueDef)>,
        &'a ValueDef, &'a Type, &'a PointerType, TypeHandle, TypeHandle,
        Ref<'a, dyn pliron::r#type::Type>, Ref<'a, dyn pliron::r#type::Type>,
        Option<&'a PlironPointerType>, &'a PlironPointerType,
        Option<&'a PlironSliceType>, Option<&'a PlironFixedVectorTypeV12>,
        &'a [&'a str], Option<&'a [&'a str]>, OperationKind, Type, Box<Type>, Type,
        PreservedOperationOp, Option<PreservedOperationOp>, Option<PreservedOperationKindAttr>,
        Option<&'a OperationKind>, &'a OperationKind,
        dialect_gpu::storage_operations_v18::StorageOpV18,
        Option<dialect_gpu::storage_operations_v18::StorageOpV18>,
        dialect_gpu::storage_operations_v18::StorageDescriptorV18,
        Option<dialect_gpu::storage_operations_v18::StorageDescriptorV18>,
        Result<dialect_gpu::storage_operations_v18::StorageDescriptorV18, KirBridgeErrorV1>,
        Result<Type, KirBridgeErrorV1>, Result<OperationKind, KirBridgeErrorV1>,
        (&'a NativeCanonicalPrivateAdmissionV18<'a>, &'a Ref<'a, Operation>, &'a mut usize),
    );
    type Extract<'a> = (
        &'a Context, Ptr<Operation>, &'a HashMap<Value, ValueId>, &'a KirBridgeOriginsV1,
        Ref<'a, Operation>, &'a fe2o3_kernel_ir::StorageOperationV1,
        fe2o3_kernel_ir::StorageOperationV1, fe2o3_kernel_ir::MemoryAccess,
        Value, ValueId, Option<&'a ValueId>, Result<ValueId, KirBridgeErrorV1>,
        (&'a HashMap<Value, ValueId>, &'a Ref<'a, Operation>),
        Ref<'a, dialect_gpu::storage_operations_v18::StorageKindAttrV18>,
        Option<Ref<'a, dialect_gpu::storage_operations_v18::StorageKindAttrV18>>,
        Ref<'a, dialect_gpu::storage_types_v18::StorageOrdinalAttrV18>,
        Option<Ref<'a, dialect_gpu::storage_types_v18::StorageOrdinalAttrV18>>,
        Ref<'a, dialect_gpu::storage_operations_v18::StorageOverlapAttrV18>,
        Option<Ref<'a, dialect_gpu::storage_operations_v18::StorageOverlapAttrV18>>,
        Ref<'a, dialect_gpu::storage_operations_v18::StorageAccessAttrV18>,
        Option<Ref<'a, dialect_gpu::storage_operations_v18::StorageAccessAttrV18>>,
        Ref<'a, dialect_gpu::storage_operations_v18::StorageAccessAttrV18>,
        Option<Ref<'a, dialect_gpu::storage_operations_v18::StorageAccessAttrV18>>,
    );
    let mut bytes = capture.checked_add(alignment.checked_mul(2).ok_or(ResourceError::Arithmetic)?)
        .ok_or(ResourceError::Arithmetic)?;
    for amount in [
        h::<Walk<'_>>()?, h::<Census<'_>>()?, h::<Carrier<'_>>()?, h::<Extract<'_>>()?,
        h::<NativeLifecycleIdentityAdmissionV18<'_>>()?, h::<NativeCanonicalPrivateAdmissionV18<'_>>()?,
        h::<(&KirPlironGraphV18<'_>, &CheckedCanonicalKirPrivateMemoryV18<'_, '_>, &mut Budget<'_>)>()?,
        h::<(usize, &NativeCanonicalPrivateAdmissionV18<'_>)>()?,
        h::<storage_v18::ProfileV18<'_>>()?, h::<KirBridgeTypeProfileV12<'_>>()?,
        h::<&HashMap<Ptr<Operation>, KirBridgeCoordinateV1>>()?,
        h::<&CanonicalKirInventoryV18<'_>>()?, h::<&VerifiedCanonicalKernelIrModuleV18>()?,
        h::<()>()?, h::<bool>()?, h::<usize>()?,
        h::<Result<(), KirBridgeErrorV1>>()?,
    ] { bytes = bytes.checked_add(amount).ok_or(ResourceError::Arithmetic)?; }
    Ok(bytes)
}

#[cfg(test)]
impl KirPlironGraphV18<'_> {
    pub(crate) fn test_private_terminator_coordinate_fault_v18(
        &mut self, physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        epoch: u64, fault: usize, budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        let expected = KirBridgeCoordinateV1::Terminator { function: 0, block: 0 };
        let pointer = *self.coordinates.iter().find(|(_, row)| **row == expected).unwrap().0;
        match fault {
            0 => { self.coordinates.remove(&pointer); }
            1 => { self.coordinates.insert(pointer, KirBridgeCoordinateV1::Terminator { function: 1, block: 0 }); }
            2 => { self.coordinates.insert(pointer, KirBridgeCoordinateV1::Terminator { function: 0, block: 1 }); }
            3 => { self.coordinates.insert(pointer, KirBridgeCoordinateV1::Operation { function: 0, block: 0, operation: 0 }); }
            _ => panic!("closed terminator coordinate fault"),
        }
        let floor = budget.storage();
        let checked = self.visit_private_policy_functions_v18(physical, epoch, budget,
            |_, _| panic!("foreign or missing terminator coordinate published a token"));
        self.coordinates.insert(pointer, expected);
        budget.release_storage(budget.storage().checked_sub(floor).ok_or(ResourceError::Accounting)?)?;
        checked
    }

    pub(crate) fn test_private_memory_fault_v18(
        &mut self, physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        epoch: u64, fault: usize, budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        let coordinate = KirBridgeCoordinateV1::Operation { function: 0, block: 0, operation: 0 };
        let pointer = *self.coordinates.iter().find(|(_, row)| **row == coordinate).unwrap().0;
        let saved_coordinate = self.coordinates[&pointer];
        let result = self.origins.values.iter().find(|(value, _)| {
            let op = pointer.deref(&self.session.context);
            **value == op.get_result(0)
        }).map(|(value, original)| (*value, *original)).unwrap();
        let old_kind = self.origins.preserved_operations[&pointer].clone();
        match fault {
            0 => { self.coordinates.remove(&pointer); }
            1 => { self.coordinates.insert(pointer, KirBridgeCoordinateV1::Operation { function: 0, block: 0, operation: 1 }); }
            2 => { self.coordinates.insert(pointer, KirBridgeCoordinateV1::Operation { function: 1, block: 0, operation: 0 }); }
            3 => { self.origins.values.insert(result.0, ValueId(20)); }
            4 => {
                let mut kind = old_kind.clone();
                let OperationKind::Alloca { alignment, .. } = &mut kind else { panic!("allocation"); };
                *alignment *= 2;
                self.origins.preserved_operations.insert(pointer, kind);
            }
            5 => self.test_ranked_mutate_and_restore_v18(),
            _ => panic!("closed native fault"),
        }
        let floor = budget.storage();
        let checked = self.visit_private_policy_functions_v18(physical, epoch, budget,
            |_, _| panic!("mutated import published a native token"));
        self.coordinates.insert(pointer, saved_coordinate);
        self.origins.values.insert(result.0, result.1);
        self.origins.preserved_operations.insert(pointer, old_kind);
        budget.release_storage(budget.storage().checked_sub(floor).ok_or(ResourceError::Accounting)?)?;
        checked
    }
}

#[cfg(test)]
impl KirPlironGraphV18<'_> {
    pub(crate) fn test_private_memory_census_v18(
        &self, physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        epoch: u64, budget: &mut Budget<'_>,
    ) -> Result<(usize, usize), Failure> {
        let functions = &self.profile.owner().module().functions;
        let definitions = functions.iter().filter(|f| f.body.is_some()).count();
        let blocks: usize = functions.iter().filter_map(|f| f.body.as_ref()).map(|b| b.blocks.len()).sum();
        let operations: usize = functions.iter().filter_map(|f| f.body.as_ref())
            .flat_map(|b| &b.blocks).map(|b| b.operations.len() + 1).sum();
        let lookup = self.coordinates.len() + self.origins.values.len()
            + self.origins.preserved_operations.len() + self.origins.blocks.len()
            + self.origins.functions.len() + self.origins.preserved_terminators.len() + 64;
        let expected_work = self.session.operations.len() + 1 + functions.len()
            + definitions * (self.origins.functions.len() + 1) + 2 * blocks
            + 32 * operations * lookup + 1;
        let headers = native_private_headers_v18(0, 1)?;
        let floor = budget.storage();
        let before = budget.work();
        self.visit_private_policy_functions_v18(physical, epoch, budget, |_, _| Ok(()))?;
        assert_eq!(budget.work() - before, expected_work);
        assert_eq!(budget.storage() - floor, headers);
        budget.release_storage(headers)?;
        Ok((expected_work, headers))
    }
}
