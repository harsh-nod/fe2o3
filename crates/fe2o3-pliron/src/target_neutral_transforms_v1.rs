//! Conservative target-neutral rewrites for canonical mixed-SSA KIR.
//!
//! Both passes operate on registered typed operations. They never infer
//! floating-point identities, reorder effects, or treat a pass-reported change
//! bit as preservation evidence. The owning optimizer re-exports canonical V13
//! and independently replays affected analyses at every pass boundary.

use std::collections::HashMap;

use dialect_gpu::optimization_v1::{
    BinaryKindAttr, BinaryOp, CastKindAttr, CastOp, CompareOp, ConstantOp, SelectOp, SliceDataOp,
    SliceLengthOp, UnaryKindAttr, UnaryOp,
};
use pliron::{
    attribute::AttributeDict,
    basic_block::BasicBlock,
    common_traits::Verify,
    context::{Context, Ptr},
    graph::dominance::{DomTree, compute_dominator_tree},
    irbuild::{
        listener::DummyListener,
        match_rewrite::{MatchRewrite, MatchRewriter, PassWrapper},
        rewriter::{IRRewriter, Rewriter},
    },
    linked_list::ContainsLinkedList,
    op::op_cast,
    operation::Operation,
    opts::dce::SideEffects,
    pass::{AnalysisManager, Pass, PassResult},
    region::Region,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::{DefiningEntity, Value},
};

/// Canonicalizes only identities that are total under every admitted target
/// and numerical policy.
#[derive(Default)]
pub struct GeneralTargetIndependentCanonicalizationPassV1;

impl Pass for GeneralTargetIndependentCanonicalizationPassV1 {
    fn run(
        &mut self,
        root: Ptr<Operation>,
        context: &mut Context,
        analyses: &mut AnalysisManager,
    ) -> Result<PassResult> {
        PassWrapper::new(
            "gpu-general-target-independent-canonicalization-v1",
            ExactTotalIdentityPatternV1,
        )
        .run(root, context, analyses)
    }

    fn name(&self) -> &str {
        "gpu-general-target-independent-canonicalization-v1"
    }
}

#[derive(Default)]
struct ExactTotalIdentityPatternV1;

impl MatchRewrite for ExactTotalIdentityPatternV1 {
    fn r#match(&mut self, context: &Context, operation: Ptr<Operation>) -> bool {
        exact_total_identity_replacement(context, operation).is_some()
    }

    fn rewrite(
        &mut self,
        context: &mut Context,
        rewriter: &mut MatchRewriter,
        operation: Ptr<Operation>,
    ) -> Result<()> {
        if let Some(replacement) = exact_total_identity_replacement(context, operation) {
            rewriter.replace_operation_with_values(context, operation, vec![replacement]);
        }
        Ok(())
    }
}

fn exact_total_identity_replacement(context: &Context, operation: Ptr<Operation>) -> Option<Value> {
    if let Some(binary) = Operation::get_op::<BinaryOp>(operation, context)
        && binary.verify(context).is_ok()
        && matches!(
            binary.kind(context),
            Some(BinaryKindAttr::BitAnd | BinaryKindAttr::BitOr)
        )
        && binary.get_operand_lhs(context) == binary.get_operand_rhs(context)
    {
        return Some(binary.get_operand_lhs(context));
    }

    if let Some(cast) = Operation::get_op::<CastOp>(operation, context)
        && cast.verify(context).is_ok()
        && cast.kind(context) == Some(CastKindAttr::Bitcast)
        && cast.get_operand_value(context).get_type(context)
            == cast.result(context).get_type(context)
    {
        return Some(cast.get_operand_value(context));
    }

    let outer = Operation::get_op::<UnaryOp>(operation, context)?;
    if outer.verify(context).is_err() || outer.kind(context) != Some(UnaryKindAttr::Not) {
        return None;
    }
    let DefiningEntity::Op(inner_operation) = outer.get_operand_operand(context).defining_entity()
    else {
        return None;
    };
    let inner = Operation::get_op::<UnaryOp>(inner_operation, context)?;
    (inner.verify(context).is_ok() && inner.kind(context) == Some(UnaryKindAttr::Not))
        .then(|| inner.get_operand_operand(context))
}

/// Eliminates an exact total expression when an identical expression already
/// dominates it. The scoped environment follows the dominator tree, so lookup
/// is linear in graph size rather than scanning all prior operations.
#[derive(Default)]
pub struct DominanceScopedGlobalValueNumberingPassV1;

impl Pass for DominanceScopedGlobalValueNumberingPassV1 {
    fn run(
        &mut self,
        root: Ptr<Operation>,
        context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> Result<PassResult> {
        let mut rewriter = IRRewriter::<DummyListener>::default();
        rewriter.get_config_mut().set_name_on_value_replacement = false;
        value_number_container(root, context, &mut rewriter);
        let mut result = PassResult::default();
        result.ir_changed = rewriter.is_modified().into();
        Ok(result)
    }

    fn name(&self) -> &str {
        "gpu-dominance-scoped-global-value-numbering-v1"
    }
}

fn value_number_container(
    container: Ptr<Operation>,
    context: &mut Context,
    rewriter: &mut IRRewriter<DummyListener>,
) {
    let regions = container.deref(context).regions().collect::<Vec<_>>();
    for region in regions {
        value_number_region(region, context, rewriter);

        let nested = region
            .deref(context)
            .iter(context)
            .flat_map(|block| block.deref(context).iter(context))
            .filter(|operation| operation.deref(context).num_regions() != 0)
            .collect::<Vec<_>>();
        for operation in nested {
            value_number_container(operation, context, rewriter);
        }
    }
}

fn value_number_region(
    region: Ptr<Region>,
    context: &mut Context,
    rewriter: &mut IRRewriter<DummyListener>,
) {
    let dominators = compute_dominator_tree(context, &region);
    let Some(root) = dominators.root() else {
        return;
    };
    let mut available = HashMap::<GlobalValueKeyV1, Vec<Ptr<Operation>>>::new();
    value_number_dominator_subtree(root, &dominators, context, rewriter, &mut available);
}

fn value_number_dominator_subtree(
    block: Ptr<BasicBlock>,
    dominators: &DomTree<Ptr<Region>, Context>,
    context: &mut Context,
    rewriter: &mut IRRewriter<DummyListener>,
    available: &mut HashMap<GlobalValueKeyV1, Vec<Ptr<Operation>>>,
) {
    let operations = block.deref(context).iter(context).collect::<Vec<_>>();
    let mut introduced = Vec::new();
    for operation in operations {
        let Some(key) = GlobalValueKeyV1::from_operation(operation, context) else {
            continue;
        };
        if let Some(dominating) = available
            .get(&key)
            .and_then(|values| values.last())
            .copied()
        {
            let replacements = dominating.deref(context).results().collect();
            rewriter.replace_operation_with_values(context, operation, replacements);
        } else {
            available.entry(key.clone()).or_default().push(operation);
            introduced.push(key);
        }
    }

    let children = dominators.children(&block).collect::<Vec<_>>();
    for child in children {
        value_number_dominator_subtree(child, dominators, context, rewriter, available);
    }

    for key in introduced.into_iter().rev() {
        let values = available
            .get_mut(&key)
            .expect("a scoped GVN key remains live until its dominator subtree exits");
        values.pop();
        if values.is_empty() {
            available.remove(&key);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum GlobalValueOperationKindV1 {
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
struct GlobalValueKeyV1 {
    kind: GlobalValueOperationKindV1,
    attributes: AttributeDict,
    result_types: Vec<TypeHandle>,
    operands: Vec<Value>,
}

impl GlobalValueKeyV1 {
    fn from_operation(operation: Ptr<Operation>, context: &Context) -> Option<Self> {
        let dynamic = Operation::get_op_dyn(operation, context);
        let effects = op_cast::<dyn SideEffects>(&*dynamic)?;
        if effects.has_side_effects(context) {
            return None;
        }
        let kind = verified_total_operation_kind(operation, context)?;
        let operation = operation.deref(context);
        Some(Self {
            kind,
            attributes: operation.attributes.clone(),
            result_types: operation.result_types().collect(),
            operands: operation.operands().collect(),
        })
    }
}

fn verified_total_operation_kind(
    operation: Ptr<Operation>,
    context: &Context,
) -> Option<GlobalValueOperationKindV1> {
    macro_rules! verified {
        ($operation:ty, $kind:ident) => {
            if let Some(concrete) = Operation::get_op::<$operation>(operation, context) {
                return concrete
                    .verify(context)
                    .is_ok()
                    .then_some(GlobalValueOperationKindV1::$kind);
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

#[cfg(test)]
mod tests {
    use dialect_gpu::optimization_v1::{BranchOp, ReturnOp};
    use pliron::{
        attribute::AttrObj,
        basic_block::BasicBlock,
        builtin::{
            attributes::IntegerAttr,
            op_interfaces::{OneRegionInterface, SingleBlockRegionInterface},
            ops::{FuncOp, ModuleOp},
            types::{FunctionType, IntegerType, Signedness},
        },
        context::Context,
        identifier::Identifier,
        irbuild::IRStatus,
        op::Op,
        operation::verify_operation,
        pass::{AnalysisManager, Pass},
        utils::apint::{APInt, bw},
    };

    use super::*;

    fn integer_attr(context: &Context, value: u32) -> AttrObj {
        Box::new(IntegerAttr::new(
            IntegerType::get(context, 32, Signedness::Unsigned),
            APInt::from_u32(value, bw(32)),
        ))
    }

    fn module_and_function(context: &mut Context, name: &str) -> (ModuleOp, FuncOp) {
        let module = ModuleOp::new(context, Identifier::try_from(name).unwrap());
        let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
        let signature = FunctionType::get(context, vec![], vec![ty]);
        let function = FuncOp::new(
            context,
            Identifier::try_from(format!("{name}_function")).unwrap(),
            signature,
        );
        module.append_operation(context, function.get_operation(), 0);
        (module, function)
    }

    #[test]
    fn total_integer_identity_is_canonicalized_without_float_algebra() {
        let context = &mut Context::new();
        let (module, function) = module_and_function(context, "canonicalize");
        let entry = function.get_entry_block(context);
        let source = ConstantOp::new(context, integer_attr(context, 7));
        let identity = BinaryOp::new(
            context,
            BinaryKindAttr::BitAnd,
            source.result(context),
            source.result(context),
        );
        let identity_pointer = identity.get_operation();
        let returned = ReturnOp::new(context, vec![identity.result(context)]);
        for operation in [
            source.get_operation(),
            identity_pointer,
            returned.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        verify_operation(module.get_operation(), context).unwrap();

        let report = GeneralTargetIndependentCanonicalizationPassV1
            .run(
                module.get_operation(),
                context,
                &mut AnalysisManager::default(),
            )
            .unwrap();

        assert_eq!(report.ir_changed, IRStatus::Changed);
        assert_eq!(returned.values(context), vec![source.result(context)]);
        assert!(identity_pointer.try_deref(context).is_err());
        verify_operation(module.get_operation(), context).unwrap();
    }

    #[test]
    fn exact_expression_is_reused_across_a_dominated_block() {
        let context = &mut Context::new();
        let (module, function) = module_and_function(context, "global_value_numbering");
        let entry = function.get_entry_block(context);
        let body = function.get_region(context);
        let successor = BasicBlock::new(context, None, vec![]);
        successor.insert_at_back(body, context);

        let lhs = ConstantOp::new(context, integer_attr(context, 6));
        let rhs = ConstantOp::new(context, integer_attr(context, 3));
        let dominating = BinaryOp::new(
            context,
            BinaryKindAttr::BitOr,
            lhs.result(context),
            rhs.result(context),
        );
        let branch = BranchOp::new(context, successor, vec![]);
        for operation in [
            lhs.get_operation(),
            rhs.get_operation(),
            dominating.get_operation(),
            branch.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }

        let duplicate = BinaryOp::new(
            context,
            BinaryKindAttr::BitOr,
            lhs.result(context),
            rhs.result(context),
        );
        let duplicate_pointer = duplicate.get_operation();
        let returned = ReturnOp::new(context, vec![duplicate.result(context)]);
        duplicate_pointer.insert_at_back(successor, context);
        returned.get_operation().insert_at_back(successor, context);
        verify_operation(module.get_operation(), context).unwrap();

        let report = DominanceScopedGlobalValueNumberingPassV1
            .run(
                module.get_operation(),
                context,
                &mut AnalysisManager::default(),
            )
            .unwrap();

        assert_eq!(report.ir_changed, IRStatus::Changed);
        assert_eq!(returned.values(context), vec![dominating.result(context)]);
        assert!(duplicate_pointer.try_deref(context).is_err());
        verify_operation(module.get_operation(), context).unwrap();
    }

    #[test]
    fn potentially_trapping_arithmetic_is_not_globally_numbered() {
        let context = &mut Context::new();
        let (module, function) = module_and_function(context, "trapping_not_gvn");
        let entry = function.get_entry_block(context);
        let lhs = ConstantOp::new(context, integer_attr(context, 6));
        let rhs = ConstantOp::new(context, integer_attr(context, 3));
        let first = BinaryOp::new(
            context,
            BinaryKindAttr::Add,
            lhs.result(context),
            rhs.result(context),
        );
        let second = BinaryOp::new(
            context,
            BinaryKindAttr::Add,
            lhs.result(context),
            rhs.result(context),
        );
        let second_pointer = second.get_operation();
        let returned = ReturnOp::new(context, vec![second.result(context)]);
        for operation in [
            lhs.get_operation(),
            rhs.get_operation(),
            first.get_operation(),
            second_pointer,
            returned.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        verify_operation(module.get_operation(), context).unwrap();

        let report = DominanceScopedGlobalValueNumberingPassV1
            .run(
                module.get_operation(),
                context,
                &mut AnalysisManager::default(),
            )
            .unwrap();

        assert_eq!(report.ir_changed, IRStatus::Unchanged);
        assert!(second_pointer.try_deref(context).is_ok());
    }
}
