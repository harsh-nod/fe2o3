//! Complete retained context-entry branch, not the enclosing bindings owner.
//! Counts original owned allocations, not the semantic module they reference.
use super::{
    CallBoundaryV29, CompletedContextEntryV29, RetainedContextEntriesV29, RetainedContextEntryV29,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};
use fe2o3_mir_model::semantic_mir_v1::SemanticOperandV1;

fn fixed<T: Copy>(_: &T) {}
fn fixed_boundary(value: &CallBoundaryV29) {
    let CallBoundaryV29 {
        block,
        statements,
        destination,
        destination_type,
        target,
        unwind,
    } = value;
    fixed(block);
    fixed(statements);
    fixed(destination);
    fixed(destination_type);
    fixed(target);
    fixed(unwind);
}

impl RetainedContextEntriesV29 {
    /// The caller counts this inline header exactly once (normally as part of
    /// AuthenticatedProductionBindings). This heap-only walk counts a root
    /// visit, the entries Vec capacity, every initialized entry and argument
    /// allocation, every operand/owned Box, and optional scope vectors.
    ///
    /// Entry/CompletedContextEntry/CallBoundary headers are embedded in the
    /// entries allocation. Operand headers are embedded in argument slots.
    /// The inline Option owns its scope header; it is not a boxed allocation.
    /// Fixed IDs/digests are not followed into separate MIR/compiler owners.
    ///
    /// Same original bounded counter throughout; no pre-scan, allocation,
    /// fresh ledger, rollback or refund. The first error is returned and any
    /// partial enclosing observation must be discarded. Observation grants no
    /// source authentication, lifecycle, compiler, proof or execution authority.
    pub(crate) fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        counter.charge(0, 1)?;
        let Self {
            entries,
            semantic_sha256,
            scopes,
        } = self;
        fixed(semantic_sha256);
        counter.vector(entries)?;
        for entry in entries {
            counter.charge(0, 1)?;
            let RetainedContextEntryV29 { source, context } = entry;
            fixed(context);
            let CompletedContextEntryV29 {
                function,
                helper,
                issuer,
                issuance,
                helper_call,
                helper_argument,
                arguments,
                root_identity,
                helper_identity,
                issuer_identity,
                context_identity,
                _source_commitment,
            } = source;
            fixed(function);
            fixed(helper);
            fixed(issuer);
            fixed_boundary(issuance);
            fixed_boundary(helper_call);
            fixed(helper_argument);
            fixed(root_identity);
            fixed(helper_identity);
            fixed(issuer_identity);
            fixed(context_identity);
            fixed(_source_commitment);
            charge_arguments(arguments, counter)?;
        }
        if let Some(scopes) = scopes {
            scopes.charge_retained_heap_storage_v1(counter)?;
        }
        Ok(())
    }
}

/// Borrowed actual Vec, not a synthetic authenticated context-entry constructor.
fn charge_arguments(
    arguments: &Vec<SemanticOperandV1>,
    counter: &mut LogicalStorageCounterV1,
) -> Result<(), LogicalStorageErrorV1> {
    counter.vector(arguments)?;
    for argument in arguments {
        argument.visit_retained_heap_storage_v1(|count, width| {
            let bytes = count
                .checked_mul(width)
                .ok_or(LogicalStorageErrorV1::Arithmetic)?;
            counter.charge(bytes, 1)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::LogicalStorageLimitsV1;
    use fe2o3_mir_model::semantic_mir_v1::*;
    use std::mem::size_of;

    fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
        LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
            max_bytes: bytes,
            max_items: items,
        })
    }
    fn arguments() -> Vec<SemanticOperandV1> {
        let ty = SemanticTypeIdV1::from_index(0);
        let mut args = Vec::with_capacity(13);
        args.push(SemanticOperandV1::Move(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(0),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty).unwrap()],
                ty,
            )
            .unwrap(),
        ));
        args.push(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![1, 2, 3]).unwrap()),
        )));
        args
    }
    #[test]
    fn nested_arguments_count_actual_spare_slots_and_box_payloads() {
        let args = arguments();
        let before = args.clone();
        let expected = args.capacity() * size_of::<SemanticOperandV1>()
            + size_of::<SemanticProjectionV1>()
            + 3;
        let mut c = counter(Some(expected), 5);
        charge_arguments(&args, &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (expected, 5));
        assert_eq!(args, before);
    }
    #[test]
    fn argument_walk_refuses_first_or_late_shared_item_limit() {
        let args = arguments();
        let mut first = counter(None, 0);
        assert_eq!(
            charge_arguments(&args, &mut first),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!((first.bytes(), first.items()), (0, 0));
        let mut late = counter(None, 4);
        assert_eq!(
            charge_arguments(&args, &mut late),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!(
            (late.bytes(), late.items()),
            (
                args.capacity() * size_of::<SemanticOperandV1>()
                    + size_of::<SemanticProjectionV1>(),
                4
            )
        );
    }
    #[test]
    fn one_short_byte_limit_stops_at_final_box_with_prefix_retained() {
        let args = arguments();
        let prefix =
            args.capacity() * size_of::<SemanticOperandV1>() + size_of::<SemanticProjectionV1>();
        let mut c = counter(Some(prefix + 2), 5);
        assert_eq!(
            charge_arguments(&args, &mut c),
            Err(LogicalStorageErrorV1::ByteLimit)
        );
        assert_eq!((c.bytes(), c.items()), (prefix, 4));
    }
    #[test]
    fn existing_arithmetic_prefix_refuses_without_resetting() {
        let args = arguments();
        let mut c = counter(None, usize::MAX);
        c.charge(usize::MAX, 7).unwrap();
        assert_eq!(
            charge_arguments(&args, &mut c),
            Err(LogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((c.bytes(), c.items()), (usize::MAX, 7));
    }
}
