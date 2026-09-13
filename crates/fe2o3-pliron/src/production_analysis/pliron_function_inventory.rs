//! One bounded top-level operation inventory for an immutable PLIRON function.

use std::ops::Range;

use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::MAX_PLIRON_IDENTITY_OPERATIONS_V1;

/// Maximum top-level blocks retained by the shared function inventory.
///
/// This matches the widest existing standalone analysis so sharing the
/// inventory does not silently tighten an individual pass's accepted input.
pub(crate) const MAX_PLIRON_FUNCTION_INVENTORY_BLOCKS_V1: usize = 65_536;
/// Maximum top-level operations retained by the shared function inventory.
pub(crate) const MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1: usize =
    MAX_PLIRON_IDENTITY_OPERATIONS_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundedPlironFunctionInventoryFailureV1 {
    BlockLimit { actual: usize, limit: usize },
    OperationLimit { actual: usize, limit: usize },
}

impl BoundedPlironFunctionInventoryFailureV1 {
    pub(crate) const fn resource(self) -> &'static str {
        match self {
            Self::BlockLimit { .. } => "CFG block",
            Self::OperationLimit { .. } => "operation",
        }
    }

    pub(crate) const fn actual(self) -> usize {
        match self {
            Self::BlockLimit { actual, .. } | Self::OperationLimit { actual, .. } => actual,
        }
    }

    pub(crate) const fn limit(self) -> usize {
        match self {
            Self::BlockLimit { limit, .. } | Self::OperationLimit { limit, .. } => limit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlironOperationSiteV1 {
    block: usize,
    operation: usize,
    pointer: Ptr<Operation>,
}

impl PlironOperationSiteV1 {
    pub(crate) const fn block(self) -> usize {
        self.block
    }

    pub(crate) const fn operation(self) -> usize {
        self.operation
    }

    pub(crate) const fn pointer(self) -> Ptr<Operation> {
        self.pointer
    }
}

/// Stable pointers and locations for one immutable function verification run.
pub(crate) struct BoundedPlironFunctionInventoryV1 {
    blocks: Vec<Ptr<BasicBlock>>,
    operations: Vec<PlironOperationSiteV1>,
    block_operation_ranges: Vec<Range<usize>>,
}

impl BoundedPlironFunctionInventoryV1 {
    pub(crate) fn collect(
        context: &Context,
        function: &FuncOp,
    ) -> Result<Self, BoundedPlironFunctionInventoryFailureV1> {
        Self::collect_with_limits(
            context,
            function,
            MAX_PLIRON_FUNCTION_INVENTORY_BLOCKS_V1,
            MAX_PLIRON_FUNCTION_INVENTORY_OPERATIONS_V1,
        )
    }

    fn collect_with_limits(
        context: &Context,
        function: &FuncOp,
        block_limit: usize,
        operation_limit: usize,
    ) -> Result<Self, BoundedPlironFunctionInventoryFailureV1> {
        let mut blocks = Vec::new();
        let mut operations = Vec::new();
        let mut block_operation_ranges = Vec::new();
        for block in function.get_region(context).deref(context).iter(context) {
            if blocks.len() == block_limit {
                return Err(BoundedPlironFunctionInventoryFailureV1::BlockLimit {
                    actual: blocks.len().saturating_add(1),
                    limit: block_limit,
                });
            }
            let block_index = blocks.len();
            let start = operations.len();
            blocks.push(block);
            for (operation_index, pointer) in block.deref(context).iter(context).enumerate() {
                if operations.len() == operation_limit {
                    return Err(BoundedPlironFunctionInventoryFailureV1::OperationLimit {
                        actual: operations.len().saturating_add(1),
                        limit: operation_limit,
                    });
                }
                operations.push(PlironOperationSiteV1 {
                    block: block_index,
                    operation: operation_index,
                    pointer,
                });
            }
            block_operation_ranges.push(start..operations.len());
        }
        Ok(Self {
            blocks,
            operations,
            block_operation_ranges,
        })
    }

    pub(crate) fn blocks(&self) -> &[Ptr<BasicBlock>] {
        &self.blocks
    }

    pub(crate) fn operations(&self) -> &[PlironOperationSiteV1] {
        &self.operations
    }

    pub(crate) fn block_operations(&self, block: usize) -> &[PlironOperationSiteV1] {
        let range = self
            .block_operation_ranges
            .get(block)
            .expect("inventory block index must identify a retained block");
        &self.operations[range.clone()]
    }
}

#[cfg(test)]
mod tests {
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };

    use super::*;

    fn empty_function(context: &mut Context) -> FuncOp {
        FuncOp::new(
            context,
            "inventory_test".try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        )
    }

    #[test]
    fn inventories_blocks_once_with_stable_ranges() {
        let mut context = Context::new();
        let function = empty_function(&mut context);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        assert_eq!(inventory.blocks(), &[function.get_entry_block(&context)]);
        assert!(inventory.operations().is_empty());
        assert!(inventory.block_operations(0).is_empty());
    }

    #[test]
    fn rejects_the_first_block_beyond_the_bound() {
        let mut context = Context::new();
        let function = empty_function(&mut context);
        assert!(matches!(
            BoundedPlironFunctionInventoryV1::collect_with_limits(
                &context,
                &function,
                0,
                usize::MAX,
            ),
            Err(BoundedPlironFunctionInventoryFailureV1::BlockLimit {
                actual: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn retains_operation_locations_and_enforces_the_operation_bound() {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let function = empty_function(&mut context);
        let operation = dialect_kernel::ReturnOp::new(&mut context);
        let pointer = operation.get_operation();
        pointer.insert_at_back(function.get_entry_block(&context), &context);

        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        assert_eq!(inventory.operations().len(), 1);
        assert_eq!(inventory.operations()[0].block(), 0);
        assert_eq!(inventory.operations()[0].operation(), 0);
        assert_eq!(inventory.operations()[0].pointer(), pointer);
        assert_eq!(inventory.block_operations(0), inventory.operations());
        assert!(
            BoundedPlironFunctionInventoryV1::collect_with_limits(&context, &function, 1, 1,)
                .is_ok()
        );
        assert!(matches!(
            BoundedPlironFunctionInventoryV1::collect_with_limits(
                &context,
                &function,
                usize::MAX,
                0,
            ),
            Err(BoundedPlironFunctionInventoryFailureV1::OperationLimit {
                actual: 1,
                limit: 0,
            })
        ));
    }
}
