use dialect_kernel::{
    AccessKindAttr, DIALECT_NAME, IndexConstantOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp,
    RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    PlironAliasDecisionV1, PlironProvenanceFailureV1, analyze_pliron_provenance_alias_v1,
};
use pliron::{
    builtin::{ops::FuncOp, types::FunctionType},
    context::Context,
    dialect::DialectName,
    op::Op,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    )
}

fn view(context: &mut Context, origin: u64, class: u64) -> RankedViewOp {
    let ty = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        origin,
        class,
    )
    .unwrap()
}

fn access(
    context: &mut Context,
    kind: AccessKindAttr,
    view: &RankedViewOp,
    index: &IndexConstantOp,
) -> RankedAccessOp {
    RankedAccessOp::new(
        context,
        kind,
        view.result(context),
        vec![index.result(context)],
    )
    .unwrap()
}

#[test]
fn distinct_classes_are_derived_as_disjoint() {
    let context = &mut setup();
    let function = function(context, "disjoint");
    let entry = function.get_entry_block(context);
    let first = view(context, 11, 101);
    let second = view(context, 12, 102);
    let zero = IndexConstantOp::new(context, 0);
    let first_write = access(context, AccessKindAttr::Write, &first, &zero);
    let second_write = access(context, AccessKindAttr::Write, &second, &zero);
    for operation in [
        first.get_operation(),
        second.get_operation(),
        zero.get_operation(),
        first_write.get_operation(),
        second_write.get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_provenance_alias_v1(context, &function).unwrap();
    assert_eq!(
        analysis.alias(first.result(context), second.result(context)),
        PlironAliasDecisionV1::Disjoint
    );
}

#[test]
fn one_origin_cannot_mutate_to_a_second_alias_class() {
    let context = &mut setup();
    let function = function(context, "inconsistent_origin");
    let entry = function.get_entry_block(context);
    let first = view(context, 17, 201);
    let second = view(context, 17, 202);
    let zero = IndexConstantOp::new(context, 0);
    let first_read = access(context, AccessKindAttr::Read, &first, &zero);
    let second_read = access(context, AccessKindAttr::Read, &second, &zero);
    for operation in [
        first.get_operation(),
        second.get_operation(),
        zero.get_operation(),
        first_read.get_operation(),
        second_read.get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    assert!(matches!(
        analyze_pliron_provenance_alias_v1(context, &function),
        Err(PlironProvenanceFailureV1::InconsistentClassForOrigin {
            origin: 17,
            first: 201,
            second: 202,
        })
    ));
}

#[test]
fn unknown_writable_views_are_explicitly_incomplete() {
    let context = &mut setup();
    let function = function(context, "unknown_writable");
    let entry = function.get_entry_block(context);
    let first = view(context, 0, 0);
    let second = view(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let first_read = access(context, AccessKindAttr::Read, &first, &zero);
    let second_write = access(context, AccessKindAttr::Write, &second, &zero);
    for operation in [
        first.get_operation(),
        second.get_operation(),
        zero.get_operation(),
        first_read.get_operation(),
        second_write.get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    assert_eq!(
        analyze_pliron_provenance_alias_v1(context, &function).unwrap_err(),
        PlironProvenanceFailureV1::UnknownWritableAlias {
            memory_space: MemorySpaceAttr::Global,
        }
    );
}

#[test]
fn read_only_unknown_views_remain_query_level_incomplete() {
    let context = &mut setup();
    let function = function(context, "unknown_read_only");
    let entry = function.get_entry_block(context);
    let first = view(context, 0, 0);
    let second = view(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let first_read = access(context, AccessKindAttr::Read, &first, &zero);
    let second_read = access(context, AccessKindAttr::Read, &second, &zero);
    for operation in [
        first.get_operation(),
        second.get_operation(),
        zero.get_operation(),
        first_read.get_operation(),
        second_read.get_operation(),
        ReturnOp::new(context).get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_provenance_alias_v1(context, &function).unwrap();
    assert_eq!(
        analysis.alias(first.result(context), first.result(context)),
        PlironAliasDecisionV1::SameAllocation
    );
    assert_eq!(
        analysis.alias(first.result(context), second.result(context)),
        PlironAliasDecisionV1::Incomplete
    );
}
