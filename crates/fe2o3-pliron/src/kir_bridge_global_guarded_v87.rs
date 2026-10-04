//! Opt-in preserved checked-store correspondence, never source authorization.
use super::*;
use fe2o3_kernel_ir::MemoryAccess;

fn candidate(operation: &KirOperation) -> bool {
    matches!(operation.kind, OperationKind::GuardedStore { access, .. }
        if matches!(access.address_space, AddressSpace::Global | AddressSpace::Generic)
            && !access.volatile)
}

// Fixed remapping deliberately does not call the generic preserved-operation
// extractor, whose operand Vec and replacement map have different accounting.
fn extract_guarded(
    context: &Context,
    pointer: Ptr<Operation>,
    expected: &OperationKind,
    origins: &KirBridgeOriginsV1,
) -> Result<OperationKind, Failure> {
    let OperationKind::GuardedStore { access, .. } = expected else {
        return Err(Failure::NativeSchema);
    };
    let raw = pointer.deref(context);
    if raw.num_regions() != 0
        || raw.get_num_successors() != 0
        || raw.get_num_operands() != 3
        || raw.get_num_results() != 0
        || raw.attributes.0.len() != 1
    {
        return Err(Failure::NativeSchema);
    }
    let preserved =
        Operation::get_op::<PreservedOperationOp>(pointer, context).ok_or(Failure::NativeSchema)?;
    if preserved.kind(context) != Some(PreservedOperationKindAttr::GuardedStore)
        || origins.preserved_operations.get(&pointer) != Some(expected)
    {
        return Err(Failure::ExactGraph);
    }
    let parent = raw.get_parent_block().ok_or(Failure::ExactGraph)?;
    let function = origins.blocks.get(&parent).ok_or(Failure::ExactGraph)?.0;
    let mut operands = [ValueId(0); 3];
    for (ordinal, id) in operands.iter_mut().enumerate() {
        let value = raw.get_operand(ordinal);
        let block = value
            .get_defining_block(context)
            .ok_or(Failure::ExactGraph)?;
        if origins
            .blocks
            .get(&block)
            .is_none_or(|(owner, _)| *owner != function)
        {
            return Err(Failure::ExactGraph);
        }
        *id = *origins.values.get(&value).ok_or(Failure::ExactGraph)?;
    }
    Ok(OperationKind::GuardedStore {
        pointer: operands[0],
        predicate: operands[1],
        value: operands[2],
        access: *access,
    })
}

impl KirPlironGraphV18<'_> {
    pub(crate) fn pending_guarded_scan_headers_v87() -> Result<usize, Failure> {
        use std::{cell::Ref, mem::size_of};
        type Scan<'a> = (
            &'a KirPlironGraphV18<'a>,
            &'a VerifiedCanonicalKernelIrModuleV18,
            &'a mut Budget<'a>,
            &'a Context,
            std::collections::hash_map::Iter<'a, Ptr<Operation>, KirBridgeCoordinateV1>,
            Option<(&'a Ptr<Operation>, &'a KirBridgeCoordinateV1)>,
            &'a Ptr<Operation>,
            &'a KirBridgeCoordinateV1,
            KirBridgeCoordinateV1,
            Option<&'a fe2o3_kernel_ir::Function>,
            Option<&'a fe2o3_kernel_ir::FunctionBody>,
            Option<&'a fe2o3_kernel_ir::BasicBlock>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Option<&'a KirOperation>,
            &'a KirOperation,
            Ref<'a, Operation>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            OperationKind,
            Result<OperationKind, Failure>,
            [usize; 6],
            [u32; 3],
            u64,
        );
        type Extract<'a> = (
            &'a Context,
            Ptr<Operation>,
            &'a OperationKind,
            &'a KirBridgeOriginsV1,
            &'a MemoryAccess,
            Ref<'a, Operation>,
            PreservedOperationOp,
            Option<PreservedOperationOp>,
            Option<PreservedOperationKindAttr>,
            Option<&'a OperationKind>,
            [ValueId; 3],
            std::iter::Enumerate<std::slice::IterMut<'a, ValueId>>,
            Option<(usize, &'a mut ValueId)>,
            &'a mut ValueId,
            Value,
            [Ptr<BasicBlock>; 2],
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            [usize; 2],
            Option<&'a ValueId>,
            OperationKind,
            Result<OperationKind, Failure>,
        );
        let frames = resources::add(size_of::<Scan<'_>>(), size_of::<Extract<'_>>())?;
        let results = resources::add(
            size_of::<Result<Scan<'_>, Failure>>(),
            size_of::<Result<Extract<'_>, Failure>>(),
        )?;
        Ok(resources::add(frames, resources::mul(2, results)?)?)
    }

    pub(crate) fn pending_guarded_global_operation_v87(
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
        candidate(operation).then_some(operation)
    }

    // Called only after the unchanged complete V18 roster/custody census. The
    // additional scan validates every candidate, including unqueried effects.
    pub(crate) fn check_pending_guarded_carriers_v87(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        epoch: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        if !std::ptr::eq(owner, self.profile.owner()) {
            return Err(Failure::ExactGraph);
        }
        self.check_ranked_policy_epoch_v18(epoch)?;
        let probes = resources::add(self.origins.values.len(), self.origins.blocks.len())?;
        let probes = resources::add(probes, self.origins.preserved_operations.len())?;
        let per_row = resources::mul(resources::add(probes, 32)?, 8)?;
        let context = &self.session.context;
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
            let source = owner
                .module()
                .functions
                .get(function as usize)
                .and_then(|f| f.body.as_ref())
                .and_then(|b| b.blocks.get(block as usize))
                .ok_or(Failure::ExactGraph)?;
            let expected = source
                .operations
                .get(operation as usize)
                .ok_or(Failure::ExactGraph)?;
            if !candidate(expected) {
                continue;
            }
            budget.charge_work(per_row)?;
            if !expected.results.is_empty() {
                return Err(Failure::NativeSchema);
            }
            let raw = pointer.deref(context);
            let parent = raw.get_parent_block().ok_or(Failure::NativeSchema)?;
            if self.origins.blocks.get(&parent) != Some(&(function as usize, source.id)) {
                return Err(Failure::ExactGraph);
            }
            let actual = extract_guarded(context, *pointer, &expected.kind, &self.origins)?;
            if actual != expected.kind {
                return Err(Failure::ExactGraph);
            }
        }
        self.check_ranked_policy_epoch_v18(epoch)
    }

    #[cfg(test)]
    pub(crate) fn test_guarded_extraction_v87(&self, fault: usize, budget: &mut Budget<'_>) {
        let context = &self.session.context;
        let pointer = *self
            .coordinates
            .iter()
            .find(|(_, coordinate)| {
                **coordinate
                    == KirBridgeCoordinateV1::Operation {
                        function: 2,
                        block: 0,
                        operation: 4,
                    }
            })
            .unwrap()
            .0;
        let expected = &self.profile.owner().module().functions[2]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations[4]
            .kind;
        let mut donor = self.origins.clone();
        match fault {
            0 => {}
            1..=3 => {
                let (operand, replacement) = match fault {
                    1 => (1, ValueId(21)),
                    2 => (2, ValueId(7)),
                    _ => (0, ValueId(4)),
                };
                let replacement = donor
                    .values
                    .iter()
                    .find(|(native, id)| {
                        **id == replacement
                            && native.get_defining_block(context).is_some_and(|block| {
                                self.origins
                                    .blocks
                                    .get(&block)
                                    .is_some_and(|(function, _)| *function == 2)
                            })
                    })
                    .map(|(native, _)| *native)
                    .unwrap();
                Operation::replace_operand(pointer, context, operand, replacement);
            }
            4 => {
                Operation::pop_operand(pointer, context);
            }
            5 => {
                let ty = pointer.deref(context).get_operand(2).get_type(context);
                Operation::push_result(pointer, context, ty);
            }
            6 => {
                pointer.deref_mut(context).attributes.0.clear();
            }
            7 => {
                let OperationKind::GuardedStore { access, .. } =
                    donor.preserved_operations.get_mut(&pointer).unwrap()
                else {
                    unreachable!()
                };
                access.alignment *= 2;
            }
            8 => {
                donor.values.remove(&pointer.deref(context).get_operand(1));
            }
            9 => {
                Operation::get_op::<PreservedOperationOp>(pointer, context)
                    .unwrap()
                    .set_attr_gpu_preserved_operation_kind(
                        context,
                        PreservedOperationKindAttr::GuardedLoad,
                    );
            }
            10 => {
                let replacement = donor
                    .values
                    .iter()
                    .find(|(native, id)| {
                        **id == ValueId(3)
                            && native.get_defining_block(context).is_some_and(|block| {
                                self.origins
                                    .blocks
                                    .get(&block)
                                    .is_some_and(|(function, _)| *function == 1)
                            })
                    })
                    .map(|(native, _)| *native)
                    .unwrap();
                Operation::replace_operand(pointer, context, 1, replacement);
            }
            11 => pointer.unlink(context),
            _ => unreachable!(),
        }
        let extracted = extract_guarded(context, pointer, expected, &donor);
        if fault == 0 {
            assert_eq!(extracted.unwrap(), *expected);
        } else {
            assert!(match &extracted {
                Err(_) => true,
                Ok(actual) => actual != expected,
            });
        }
        if fault < 7 || fault >= 9 {
            // A fresh epoch alone must not launder a malformed or different effect.
            let epoch = self.ranked_policy_epoch_v18().unwrap();
            let result =
                self.check_pending_guarded_carriers_v87(self.profile.owner(), epoch, budget);
            assert_eq!(result.is_ok(), fault == 0);
        }
        if fault == 11 {
            assert!(self.check_pending_global_roster_v18(budget).is_err());
            let terminal = *self
                .coordinates
                .iter()
                .find(|(_, coordinate)| {
                    **coordinate
                        == KirBridgeCoordinateV1::Terminator {
                            function: 2,
                            block: 0,
                        }
                })
                .unwrap()
                .0;
            pointer.insert_before(context, terminal);
            self.check_pending_global_roster_v18(budget).unwrap();
        }
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use std::{cell::Ref, mem::size_of};

    #[test]
    fn pending_guarded_v87_classifier_is_opt_in_global_generic_and_nonvolatile_only() {
        for space in [
            AddressSpace::Private,
            AddressSpace::Workgroup,
            AddressSpace::Global,
            AddressSpace::Generic,
            AddressSpace::Constant,
        ] {
            for volatile in [false, true] {
                let mut access = MemoryAccess::new(space, 4);
                access.volatile = volatile;
                let row = KirOperation::new(
                    vec![],
                    OperationKind::GuardedStore {
                        pointer: ValueId(0),
                        predicate: ValueId(1),
                        value: ValueId(2),
                        access,
                    },
                );
                assert_eq!(
                    candidate(&row),
                    !volatile && matches!(space, AddressSpace::Global | AddressSpace::Generic)
                );
                assert!(!super::super::closed(&row));
            }
        }
    }

    #[test]
    fn pending_guarded_v87_fixed_frames_have_independent_exact_and_one_short_storage() {
        type Census<'a> = (
            &'a KirPlironGraphV18<'a>,
            &'a VerifiedCanonicalKernelIrModuleV18,
            &'a mut Budget<'a>,
            &'a Context,
            std::collections::hash_map::Iter<'a, Ptr<Operation>, KirBridgeCoordinateV1>,
            Option<(&'a Ptr<Operation>, &'a KirBridgeCoordinateV1)>,
            &'a Ptr<Operation>,
            &'a KirBridgeCoordinateV1,
            KirBridgeCoordinateV1,
            Option<&'a fe2o3_kernel_ir::Function>,
            Option<&'a fe2o3_kernel_ir::FunctionBody>,
            Option<&'a fe2o3_kernel_ir::BasicBlock>,
            &'a fe2o3_kernel_ir::BasicBlock,
            Option<&'a KirOperation>,
            &'a KirOperation,
            Ref<'a, Operation>,
            Ptr<BasicBlock>,
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            OperationKind,
            Result<OperationKind, Failure>,
            [usize; 6],
            [u32; 3],
            u64,
        );
        type Remap<'a> = (
            &'a Context,
            Ptr<Operation>,
            &'a OperationKind,
            &'a KirBridgeOriginsV1,
            &'a MemoryAccess,
            Ref<'a, Operation>,
            PreservedOperationOp,
            Option<PreservedOperationOp>,
            Option<PreservedOperationKindAttr>,
            Option<&'a OperationKind>,
            [ValueId; 3],
            std::iter::Enumerate<std::slice::IterMut<'a, ValueId>>,
            Option<(usize, &'a mut ValueId)>,
            &'a mut ValueId,
            Value,
            [Ptr<BasicBlock>; 2],
            Option<Ptr<BasicBlock>>,
            Option<&'a (usize, BlockId)>,
            [usize; 2],
            Option<&'a ValueId>,
            OperationKind,
            Result<OperationKind, Failure>,
        );
        let expected = size_of::<Census<'_>>()
            + size_of::<Remap<'_>>()
            + 2 * size_of::<Result<Census<'_>, Failure>>()
            + 2 * size_of::<Result<Remap<'_>, Failure>>();
        assert_eq!(
            KirPlironGraphV18::pending_guarded_scan_headers_v87().unwrap(),
            expected
        );
        for short in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
            let mut budget = Budget::new(&mut work, expected - usize::from(short));
            let result = budget.reserve_storage(expected);
            assert_eq!(result.is_ok(), !short);
            assert_eq!(budget.storage(), if short { 0 } else { expected });
            assert_eq!(budget.work(), 0);
            if short {
                assert!(matches!(result, Err(ResourceError::Storage(error))
                    if error.actual() == expected && error.limit() == expected - 1));
                for _ in 0..2 {
                    assert!(
                        matches!(budget.check_prior_denials_v1(), Err(ResourceError::Storage(error))
                        if error.actual() == expected && error.limit() == expected - 1)
                    );
                }
            }
        }
    }
}
