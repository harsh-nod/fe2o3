//! Conservative same-block common-subexpression elimination for `gpu.*` operations.
//!
//! Eligibility is a closed whitelist. A candidate must also pass its concrete
//! verifier and explicitly implement the Pliron `SideEffects` interface as
//! effect-free. Expressions are reused only from earlier in the same block, so
//! the replacement value trivially dominates the operation being removed.

use std::collections::HashMap;

use pliron::{
    attribute::AttributeDict,
    basic_block::BasicBlock,
    common_traits::Verify,
    context::{Context, Ptr},
    irbuild::{
        IRStatus,
        listener::DummyListener,
        rewriter::{IRRewriter, Rewriter},
    },
    linked_list::ContainsLinkedList,
    op::op_cast,
    operation::Operation,
    opts::dce::SideEffects,
    pass::{AnalysisManager, Pass, PassResult},
    result::Result,
    r#type::TypeHandle,
    value::Value,
};

use crate::optimization_v1::{
    BinaryOp, CastOp, CompareOp, ConstantOp, SelectOp, SliceDataOp, SliceLengthOp, UnaryOp,
};

/// A deterministic local CSE pass for the closed set of total GPU expressions.
#[derive(Default)]
pub struct LocalPureCsePassV1;

impl Pass for LocalPureCsePassV1 {
    fn run(
        &mut self,
        root: Ptr<Operation>,
        context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> Result<PassResult> {
        let mut result = PassResult::default();
        result.ir_changed = local_pure_cse_v1(root, context);
        Ok(result)
    }

    fn name(&self) -> &str {
        "gpu-local-pure-cse-v1"
    }
}

/// Eliminates exact duplicate total expressions within individual blocks.
pub fn local_pure_cse_v1(root: Ptr<Operation>, context: &mut Context) -> IRStatus {
    let mut pending_containers = vec![root];
    let mut rewriter = IRRewriter::<DummyListener>::default();
    rewriter.get_config_mut().set_name_on_value_replacement = false;

    while let Some(container) = pending_containers.pop() {
        let regions = container.deref(context).regions().collect::<Vec<_>>();
        let mut nested_containers = Vec::new();
        for region in regions {
            let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
            for block in blocks {
                let operations = block.deref(context).iter(context).collect::<Vec<_>>();
                nested_containers.extend(
                    operations
                        .iter()
                        .copied()
                        .filter(|operation| operation.deref(context).num_regions() != 0),
                );
                eliminate_in_block(block, operations, context, &mut rewriter);
            }
        }
        pending_containers.extend(nested_containers.into_iter().rev());
    }

    rewriter.is_modified().into()
}

fn eliminate_in_block(
    _block: Ptr<BasicBlock>,
    operations: Vec<Ptr<Operation>>,
    context: &mut Context,
    rewriter: &mut IRRewriter<DummyListener>,
) {
    let mut available = HashMap::<LocalCseKey, Ptr<Operation>>::new();
    for operation in operations {
        let Some(key) = LocalCseKey::from_operation(operation, context) else {
            continue;
        };
        if let Some(earlier) = available.get(&key).copied() {
            let replacement_values = earlier.deref(context).results().collect();
            rewriter.replace_operation_with_values(context, operation, replacement_values);
        } else {
            available.insert(key, operation);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum LocalCseOperationKind {
    Constant,
    Unary,
    Binary,
    Compare,
    Cast,
    Select,
    SliceLength,
    SliceData,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct LocalCseKey {
    kind: LocalCseOperationKind,
    attributes: AttributeDict,
    result_types: Vec<TypeHandle>,
    operands: Vec<Value>,
}

impl LocalCseKey {
    fn from_operation(operation: Ptr<Operation>, context: &Context) -> Option<Self> {
        let dynamic = Operation::get_op_dyn(operation, context);
        let effects = op_cast::<dyn SideEffects>(&*dynamic)?;
        if effects.has_side_effects(context) {
            return None;
        }
        let kind = verified_kind(operation, context)?;
        let operation = operation.deref(context);
        Some(Self {
            kind,
            attributes: operation.attributes.clone(),
            result_types: operation.result_types().collect(),
            operands: operation.operands().collect(),
        })
    }
}

fn verified_kind(operation: Ptr<Operation>, context: &Context) -> Option<LocalCseOperationKind> {
    macro_rules! verified {
        ($op:ty, $kind:ident) => {
            if let Some(concrete) = Operation::get_op::<$op>(operation, context) {
                return concrete
                    .verify(context)
                    .is_ok()
                    .then_some(LocalCseOperationKind::$kind);
            }
        };
    }

    verified!(ConstantOp, Constant);
    verified!(UnaryOp, Unary);
    verified!(BinaryOp, Binary);
    verified!(CompareOp, Compare);
    verified!(CastOp, Cast);
    verified!(SelectOp, Select);
    verified!(SliceLengthOp, SliceLength);
    verified!(SliceDataOp, SliceData);
    None
}
