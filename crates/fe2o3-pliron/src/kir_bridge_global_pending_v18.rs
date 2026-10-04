//! Closed carrier correspondence only. This does not admit global memory safety.
use super::*;
use crate::production_analysis::CanonicalRankedPolicyFailureV1 as Failure;
use fe2o3_kernel_ir::{AddressSpace, CanonicalKirOperationCoordinateV1};
use pliron::builtin::op_interfaces::OneRegionInterface;

#[path = "kir_bridge_global_guarded_v87.rs"]
mod guarded_v87;

// These are exact correspondence candidates, not a scalar-memory grammar:
// nonvolatile Global/Generic loads/stores, scalar-Global pointer producers, and slice
// lengths (including non-Global slices). The source join separately admits its
// exact scalar Global allocation and Generic transport recipe; observing a
// candidate never establishes a pointer's origin or memory safety.
fn closed(operation: &KirOperation) -> bool {
    match &operation.kind {
        OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
            matches!(
                access.address_space,
                AddressSpace::Global | AddressSpace::Generic
            ) && !access.volatile
        }
        OperationKind::SliceLength { .. } => true,
        OperationKind::SliceData { .. } | OperationKind::GetElementPointer { .. } => {
            matches!(operation.results.as_slice(), [result]
                if matches!(&result.ty, Type::Pointer(pointer)
                    if pointer.address_space == AddressSpace::Global
                    && matches!(pointer.pointee.as_ref(), Type::Scalar(_))))
        }
        _ => false,
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use fe2o3_kernel_ir::{Function, FunctionBody, PointerType, ValueDef};
    use std::{cell::Ref, mem::size_of, ops::Range};
    #[test]
    fn pending_global_generic_carriers_are_inert_and_do_not_admit_other_spaces() {
        use fe2o3_kernel_ir::{MemoryAccess, ScalarType};
        for space in [
            AddressSpace::Private,
            AddressSpace::Workgroup,
            AddressSpace::Global,
            AddressSpace::Constant,
            AddressSpace::Generic,
        ] {
            for volatile in [false, true] {
                let mut access = MemoryAccess::new(space, 4);
                access.volatile = volatile;
                let expected =
                    matches!(space, AddressSpace::Global | AddressSpace::Generic) && !volatile;
                let load = KirOperation::new(
                    vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32))],
                    OperationKind::Load {
                        pointer: ValueId(0),
                        access,
                    },
                );
                let store = KirOperation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(0),
                        value: ValueId(1),
                        access,
                    },
                );
                assert_eq!(closed(&load), expected);
                assert_eq!(closed(&store), expected);
            }
        }
    }
    #[test]
    fn pending_global_native_scan_frames_have_an_independent_equation() {
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T, Failure>>()
        }
        type Roster<'a> = (
            &'a Context,
            Ptr<Operation>,
            Option<&'a Ptr<Operation>>,
            Ptr<pliron::region::Region>,
            Ref<'a, Operation>,
            Ref<'a, pliron::region::Region>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Ref<'a, BasicBlock>,
            pliron::linked_list::Iter<'a, Operation>,
            std::iter::Enumerate<std::slice::Iter<'a, Function>>,
            Option<(usize, &'a Function)>,
            &'a Function,
            &'a FunctionBody,
            Ptr<Operation>,
            Option<Ptr<Operation>>,
            Option<&'a usize>,
            FuncOp,
            Option<FuncOp>,
            Ptr<pliron::region::Region>,
            Ref<'a, pliron::region::Region>,
            pliron::linked_list::Iter<'a, BasicBlock>,
            std::iter::Enumerate<std::slice::Iter<'a, fe2o3_kernel_ir::BasicBlock>>,
            Option<(usize, &'a fe2o3_kernel_ir::BasicBlock)>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            Ref<'a, BasicBlock>,
            pliron::linked_list::Iter<'a, Operation>,
            Range<usize>,
            Option<usize>,
            [Ptr<Operation>; 2],
            [Option<Ptr<Operation>>; 3],
            Option<&'a KirBridgeCoordinateV1>,
            KirBridgeCoordinateV1,
            [usize; 6],
            [u32; 3],
            Result<u32, std::num::TryFromIntError>,
        );
        type Carrier<'a> = (
            &'a Context,
            std::collections::hash_map::Iter<'a, Ptr<Operation>, KirBridgeCoordinateV1>,
            Option<(&'a Ptr<Operation>, &'a KirBridgeCoordinateV1)>,
            &'a Ptr<Operation>,
            &'a KirBridgeCoordinateV1,
            KirBridgeCoordinateV1,
            Option<&'a Function>,
            Option<&'a FunctionBody>,
            Option<&'a fe2o3_kernel_ir::BasicBlock>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Option<&'a KirOperation>,
            &'a KirOperation,
            Ref<'a, Operation>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            (
                &'a HashMap<Value, ValueId>,
                &'a Ref<'a, Operation>,
                &'a mut usize,
            ),
            ValueId,
            Value,
            Option<&'a ValueId>,
            std::iter::Enumerate<std::slice::Iter<'a, ValueDef>>,
            Option<(usize, &'a ValueDef)>,
            &'a ValueDef,
            &'a Type,
            &'a PointerType,
            [TypeHandle; 2],
            [Ref<'a, dyn pliron::r#type::Type>; 2],
            Option<&'a PlironPointerType>,
            &'a PlironPointerType,
            Option<&'a PlironSliceType>,
            Option<&'a PlironFixedVectorTypeV12>,
            OperationKind,
            Type,
            Box<Type>,
            Type,
            Result<Type, KirBridgeErrorV1>,
            Result<OperationKind, KirBridgeErrorV1>,
            [usize; 6],
            [u32; 3],
        );
        type Extract<'a> = (
            &'a Context,
            Ptr<Operation>,
            &'a OperationKind,
            &'a HashMap<Value, ValueId>,
            &'a KirBridgeOriginsV1,
            KirBridgeTypeProfileV12<'a>,
            [Ref<'a, Operation>; 2],
            LoadOp,
            Option<LoadOp>,
            StoreOp,
            Option<StoreOp>,
            Value,
            [ValueId; 2],
            Option<&'a ValueId>,
            Result<ValueId, KirBridgeErrorV1>,
            fe2o3_kernel_ir::MemoryAccess,
            AddressSpace,
            Option<AddressSpaceAttr>,
            Option<u32>,
            Option<bool>,
            OperationKind,
            Ref<'a, AddressSpaceAttr>,
            Option<Ref<'a, AddressSpaceAttr>>,
            Ref<'a, dialect_gpu::optimization_v1::MemoryAlignmentAttr>,
            Option<Ref<'a, dialect_gpu::optimization_v1::MemoryAlignmentAttr>>,
            Ref<'a, dialect_gpu::optimization_v1::VolatileAttr>,
            Option<Ref<'a, dialect_gpu::optimization_v1::VolatileAttr>>,
        );
        let expected = h::<Roster<'_>>()
            + h::<Carrier<'_>>()
            + h::<Extract<'_>>()
            + h::<(
                &KirPlironGraphV18<'_>,
                &VerifiedCanonicalKernelIrModuleV18,
                u64,
                &mut Budget<'_>,
            )>()
            + h::<(&KirPlironGraphV18<'_>, &mut Budget<'_>)>();
        assert_eq!(
            KirPlironGraphV18::pending_global_scan_headers_v18().unwrap(),
            expected
        );
        for short in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
            let mut budget = Budget::new(&mut work, expected - usize::from(short));
            let result = budget
                .reserve_storage(KirPlironGraphV18::pending_global_scan_headers_v18().unwrap());
            if short {
                assert!(matches!(result, Err(ResourceError::Storage(error))
                    if error.actual() == expected && error.limit() == expected - 1));
                assert_eq!(budget.storage(), 0);
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), expected);
            }
        }
    }
}

impl KirPlironGraphV18<'_> {
    pub(crate) fn pending_global_scan_headers_v18() -> Result<usize, Failure> {
        use fe2o3_kernel_ir::{Function, FunctionBody, PointerType, ValueDef};
        use std::{cell::Ref, mem::size_of, ops::Range};
        fn h<T>() -> Result<usize, Failure> {
            Ok(resources::add(
                size_of::<T>(),
                resources::mul(2, size_of::<Result<T, Failure>>())?,
            )?)
        }
        // Concurrent call frames have separate slots; Result carriers are not
        // used as spare storage for native iterators, borrows or decoded payloads.
        type Roster<'a> = (
            &'a Context,
            Ptr<Operation>,
            Option<&'a Ptr<Operation>>,
            Ptr<pliron::region::Region>,
            Ref<'a, Operation>,
            Ref<'a, pliron::region::Region>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Ref<'a, BasicBlock>,
            pliron::linked_list::Iter<'a, Operation>,
            std::iter::Enumerate<std::slice::Iter<'a, Function>>,
            Option<(usize, &'a Function)>,
            &'a Function,
            &'a FunctionBody,
            Ptr<Operation>,
            Option<Ptr<Operation>>,
            Option<&'a usize>,
            FuncOp,
            Option<FuncOp>,
            Ptr<pliron::region::Region>,
            Ref<'a, pliron::region::Region>,
            pliron::linked_list::Iter<'a, BasicBlock>,
            std::iter::Enumerate<std::slice::Iter<'a, fe2o3_kernel_ir::BasicBlock>>,
            Option<(usize, &'a fe2o3_kernel_ir::BasicBlock)>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            Ref<'a, BasicBlock>,
            pliron::linked_list::Iter<'a, Operation>,
            Range<usize>,
            Option<usize>,
            [Ptr<Operation>; 2],
            [Option<Ptr<Operation>>; 3],
            Option<&'a KirBridgeCoordinateV1>,
            KirBridgeCoordinateV1,
            [usize; 6],
            [u32; 3],
            Result<u32, std::num::TryFromIntError>,
        );
        type Carrier<'a> = (
            &'a Context,
            std::collections::hash_map::Iter<'a, Ptr<Operation>, KirBridgeCoordinateV1>,
            Option<(&'a Ptr<Operation>, &'a KirBridgeCoordinateV1)>,
            &'a Ptr<Operation>,
            &'a KirBridgeCoordinateV1,
            KirBridgeCoordinateV1,
            Option<&'a Function>,
            Option<&'a FunctionBody>,
            Option<&'a fe2o3_kernel_ir::BasicBlock>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Option<&'a KirOperation>,
            &'a KirOperation,
            Ref<'a, Operation>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            (
                &'a HashMap<Value, ValueId>,
                &'a Ref<'a, Operation>,
                &'a mut usize,
            ),
            ValueId,
            Value,
            Option<&'a ValueId>,
            std::iter::Enumerate<std::slice::Iter<'a, ValueDef>>,
            Option<(usize, &'a ValueDef)>,
            &'a ValueDef,
            &'a Type,
            &'a PointerType,
            [TypeHandle; 2],
            [Ref<'a, dyn pliron::r#type::Type>; 2],
            Option<&'a PlironPointerType>,
            &'a PlironPointerType,
            Option<&'a PlironSliceType>,
            Option<&'a PlironFixedVectorTypeV12>,
            OperationKind,
            Type,
            Box<Type>,
            Type,
            Result<Type, KirBridgeErrorV1>,
            Result<OperationKind, KirBridgeErrorV1>,
            [usize; 6],
            [u32; 3],
        );
        type Extract<'a> = (
            &'a Context,
            Ptr<Operation>,
            &'a OperationKind,
            &'a HashMap<Value, ValueId>,
            &'a KirBridgeOriginsV1,
            KirBridgeTypeProfileV12<'a>,
            [Ref<'a, Operation>; 2],
            LoadOp,
            Option<LoadOp>,
            StoreOp,
            Option<StoreOp>,
            Value,
            [ValueId; 2],
            Option<&'a ValueId>,
            Result<ValueId, KirBridgeErrorV1>,
            fe2o3_kernel_ir::MemoryAccess,
            AddressSpace,
            Option<AddressSpaceAttr>,
            Option<u32>,
            Option<bool>,
            OperationKind,
            Ref<'a, AddressSpaceAttr>,
            Option<Ref<'a, AddressSpaceAttr>>,
            Ref<'a, dialect_gpu::optimization_v1::MemoryAlignmentAttr>,
            Option<Ref<'a, dialect_gpu::optimization_v1::MemoryAlignmentAttr>>,
            Ref<'a, dialect_gpu::optimization_v1::VolatileAttr>,
            Option<Ref<'a, dialect_gpu::optimization_v1::VolatileAttr>>,
        );
        let mut total = 0;
        for amount in [
            h::<Roster<'_>>()?,
            h::<Carrier<'_>>()?,
            h::<Extract<'_>>()?,
            h::<(
                &KirPlironGraphV18<'_>,
                &VerifiedCanonicalKernelIrModuleV18,
                u64,
                &mut Budget<'_>,
            )>()?,
            h::<(&KirPlironGraphV18<'_>, &mut Budget<'_>)>()?,
        ] {
            total = resources::add(total, amount)?;
        }
        Ok(total)
    }

    pub(crate) fn pending_global_operation_v18(
        &self,
        coordinate: CanonicalKirOperationCoordinateV1,
    ) -> Option<&KirOperation> {
        let operation = self
            .profile
            .owner()
            .module()
            .functions
            .get(coordinate.block.function.0 as usize)?
            .body
            .as_ref()?
            .blocks
            .get(coordinate.block.block as usize)?
            .operations
            .get(coordinate.operation as usize)?;
        closed(operation).then_some(operation)
    }

    // The enclosing pending-source scope has already performed the exact live
    // O0 round trip, including ordered origins, operands and successor payloads.
    // This additional census closes the actual carrier schema once. Queries
    // subsequently use dense immutable coordinates and the same native epoch.
    pub(crate) fn check_pending_global_carriers_v18(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        epoch: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        if !std::ptr::eq(owner, self.profile.owner()) {
            return Err(Failure::ExactGraph);
        }
        self.check_ranked_policy_epoch_v18(epoch)?;
        budget.charge_work(resources::add(
            resources::add(
                self.session.operations.len(),
                self.session.owned_tree_work.len(),
            )?,
            resources::add(self.session.operation_graph_epochs.len(), 8)?,
        )?)?;
        self.validate_custody(budget)?;
        self.check_pending_global_roster_v18(budget)?;
        let context = &self.session.context;
        let lookup = resources::add(self.origins.values.len(), self.origins.blocks.len())?;
        let per_row = resources::mul(resources::add(lookup, 32)?, 8)?;
        for (pointer, coordinate) in &self.coordinates {
            budget.charge_work(1)?;
            let KirBridgeCoordinateV1::Operation {
                function,
                block,
                operation,
            } = *coordinate
            else {
                continue;
            };
            let source_block = owner
                .module()
                .functions
                .get(function as usize)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(block as usize))
                .ok_or(Failure::ExactGraph)?;
            let expected = source_block
                .operations
                .get(operation as usize)
                .ok_or(Failure::ExactGraph)?;
            if !closed(expected) {
                continue;
            }
            budget.charge_work(per_row)?;
            let raw = pointer.deref(context);
            if raw.num_regions() != 0
                || raw.get_num_successors() != 0
                || raw.get_num_operands() != expected.kind.operand_count()
                || raw.get_num_operands() > 2
                || raw.get_num_results() != expected.results.len()
                || raw.get_num_results() > 1
            {
                return Err(Failure::NativeSchema);
            }
            let parent = raw.get_parent_block().ok_or(Failure::NativeSchema)?;
            if self.origins.blocks.get(&parent) != Some(&(function as usize, source_block.id)) {
                return Err(Failure::ExactGraph);
            }
            let mut ordinal = 0;
            expected.kind.try_visit_operands(|id| {
                if self.origins.values.get(&raw.get_operand(ordinal)) != Some(&id) {
                    return Err(Failure::ExactGraph);
                }
                ordinal += 1;
                Ok(())
            })?;
            for (ordinal, result) in expected.results.iter().enumerate() {
                let actual = raw.get_result(ordinal);
                if self.origins.values.get(&actual) != Some(&result.id) {
                    return Err(Failure::ExactGraph);
                }
                let handle = actual.get_type(context);
                let actual_type = handle.deref(context);
                let leaf = if let Some(pointer) = actual_type.downcast_ref::<PlironPointerType>() {
                    pointer.pointee()
                } else {
                    handle
                };
                let leaf_type = leaf.deref(context);
                if leaf_type.downcast_ref::<PlironPointerType>().is_some()
                    || leaf_type.downcast_ref::<PlironSliceType>().is_some()
                    || leaf_type
                        .downcast_ref::<PlironFixedVectorTypeV12>()
                        .is_some()
                    || !matches!(&result.ty, Type::Scalar(_) | Type::Pointer(_))
                {
                    return Err(Failure::NativeSchema);
                }
                if self
                    .profile
                    .decode_type(context, handle, 0)
                    .map_err(|_| Failure::NativeSchema)?
                    != result.ty
                {
                    return Err(Failure::ExactGraph);
                }
            }
            let attributes = match expected.kind {
                OperationKind::Load { .. } | OperationKind::Store { .. } => 3,
                _ => 0,
            };
            if raw.attributes.0.len() != attributes {
                return Err(Failure::NativeSchema);
            }
            // Each admitted extractor has a fixed shape, no vectors or owned
            // strings. Its required named getters plus cardinality reject extras.
            let actual = extract_operation(
                context,
                *pointer,
                &expected.kind,
                &self.origins.values,
                &self.origins,
                KirBridgeTypeProfileV12::V18(self.profile),
            )
            .map_err(|_| Failure::NativeSchema)?;
            if actual != expected.kind {
                return Err(Failure::ExactGraph);
            }
        }
        self.check_ranked_policy_epoch_v18(epoch)
    }

    fn check_pending_global_roster_v18(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        let context = &self.session.context;
        budget.charge_work(resources::add(self.session.operations.len(), 4)?)?;
        let root = *self
            .session
            .operations
            .get(&self.root.identity)
            .ok_or(Failure::ExactGraph)?;
        let region = root.deref(context).get_region(0);
        let block = region
            .deref(context)
            .iter(context)
            .next()
            .ok_or(Failure::NativeSchema)?;
        let raw = block.deref(context);
        let mut functions = raw.iter(context);
        let mut count = 0usize;
        for (function, source) in self.profile.owner().module().functions.iter().enumerate() {
            budget.charge_work(resources::add(self.origins.functions.len(), 4)?)?;
            let Some(body) = source.body.as_ref() else {
                continue;
            };
            let pointer = functions.next().ok_or(Failure::ExactGraph)?;
            if self.origins.functions.get(&pointer) != Some(&function) {
                return Err(Failure::ExactGraph);
            }
            let native =
                Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            let native_region = native.get_region(context).deref(context);
            let mut blocks = native_region.iter(context);
            for (block, source) in body.blocks.iter().enumerate() {
                budget.charge_work(resources::add(self.origins.blocks.len(), 4)?)?;
                let pointer = blocks.next().ok_or(Failure::ExactGraph)?;
                if self.origins.blocks.get(&pointer) != Some(&(function, source.id)) {
                    return Err(Failure::ExactGraph);
                }
                let raw = pointer.deref(context);
                let mut operations = raw.iter(context);
                for operation in 0..source.operations.len() {
                    budget.charge_work(resources::add(self.coordinates.len(), 4)?)?;
                    let pointer = operations.next().ok_or(Failure::ExactGraph)?;
                    if self.coordinates.get(&pointer)
                        != Some(&KirBridgeCoordinateV1::Operation {
                            function: u32::try_from(function)
                                .map_err(|_| ResourceError::Arithmetic)?,
                            block: u32::try_from(block).map_err(|_| ResourceError::Arithmetic)?,
                            operation: u32::try_from(operation)
                                .map_err(|_| ResourceError::Arithmetic)?,
                        })
                    {
                        return Err(Failure::ExactGraph);
                    }
                    count = resources::add(count, 1)?;
                }
                let terminal = operations.next().ok_or(Failure::ExactGraph)?;
                budget.charge_work(resources::add(self.coordinates.len(), 4)?)?;
                if self.coordinates.get(&terminal)
                    != Some(&KirBridgeCoordinateV1::Terminator {
                        function: u32::try_from(function).map_err(|_| ResourceError::Arithmetic)?,
                        block: u32::try_from(block).map_err(|_| ResourceError::Arithmetic)?,
                    })
                    || operations.next().is_some()
                {
                    return Err(Failure::ExactGraph);
                }
                count = resources::add(count, 1)?;
            }
            if blocks.next().is_some() {
                return Err(Failure::ExactGraph);
            }
        }
        if functions.next().is_some() || count != self.coordinates.len() {
            return Err(Failure::ExactGraph);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn test_pending_global_roster_counts_v18(&self) -> [usize; 5] {
        [
            self.session.operations.len(),
            self.profile.owner().module().functions.len(),
            self.origins.functions.len(),
            self.origins.blocks.len(),
            self.coordinates.len(),
        ]
    }

    #[cfg(test)]
    pub(crate) fn test_pending_global_roster_work_v18(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        self.check_pending_global_roster_v18(budget)
    }

    #[cfg(test)]
    pub(crate) fn test_pending_global_operand_v18(&self, fault: usize) {
        let context = &self.session.context;
        let operation = |index| {
            *self
                .coordinates
                .iter()
                .find(|(_, coordinate)| {
                    **coordinate
                        == KirBridgeCoordinateV1::Operation {
                            function: 1,
                            block: 0,
                            operation: index,
                        }
                })
                .unwrap()
                .0
        };
        let (target, operand, replacement) = match fault {
            0 => (operation(4), 1, operation(3).deref(context).get_result(0)),
            1 => (operation(2), 0, operation(2).deref(context).get_result(0)),
            2 => (operation(2), 1, operation(1).deref(context).get_result(0)),
            3 => (
                operation(0),
                0,
                *self
                    .origins
                    .values
                    .iter()
                    .find(|(_, id)| **id == ValueId(20))
                    .unwrap()
                    .0,
            ),
            4 => {
                let terminal = *self
                    .coordinates
                    .iter()
                    .find(|(_, coordinate)| {
                        **coordinate
                            == KirBridgeCoordinateV1::Terminator {
                                function: 1,
                                block: 0,
                            }
                    })
                    .unwrap()
                    .0;
                (
                    terminal,
                    0,
                    *self
                        .origins
                        .values
                        .iter()
                        .find(|(_, id)| **id == ValueId(21))
                        .unwrap()
                        .0,
                )
            }
            _ => panic!("closed Global native mutation"),
        };
        Operation::replace_operand(target, context, operand, replacement);
    }
}
