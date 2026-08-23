use dialect_kernel::{
    CheckedTiledIndex2DOp, DIALECT_NAME, DimensionOp, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, InvocationIndexOp, RankedViewOp, RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    SparseIndexFactV1, SparseIndexFailureV1, analyze_pliron_sparse_indices_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
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

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

#[test]
fn sparse_affine_chain_propagates_only_to_its_consumers() {
    let context = &mut setup();
    let function = function(context, "affine_chain");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 16);
    let stride = IndexConstantOp::new(context, 4);
    let offset = IndexConstantOp::new(context, 3);
    let scaled = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        stride.result(context),
    );
    let address = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        scaled.result(context),
        offset.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        stride.get_operation(),
        offset.get_operation(),
        scaled.get_operation(),
        address.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let SparseIndexFactV1::Affine(fact) = analysis.fact(address.result(context)) else {
        panic!("expected affine fact");
    };
    assert_eq!(fact.constant_term(), 3);
    assert_eq!(fact.coefficients()[0], 4);
    assert_eq!(fact.evaluate(&[7]), Some(31));
    assert_eq!(fact.maximum(analysis.launch_extents()), Some(63));
}

#[test]
fn nonlinear_products_and_arithmetic_overflow_become_unknown() {
    let context = &mut setup();
    let function = function(context, "nonlinear");
    let entry = function.get_entry_block(context);
    let x = InvocationIndexOp::new(context, 0, 4);
    let y = InvocationIndexOp::new(context, 1, 4);
    let product = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        x.result(context),
        y.result(context),
    );
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let one = IndexConstantOp::new(context, 1);
    let overflow = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        one.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        x.get_operation(),
        y.get_operation(),
        product.get_operation(),
        maximum.get_operation(),
        one.get_operation(),
        overflow.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(product.result(context)),
        SparseIndexFactV1::Unknown
    );
    assert_eq!(
        analysis.fact(overflow.result(context)),
        SparseIndexFactV1::Unknown
    );
}

#[test]
fn nonzero_remainder_is_exact_and_zero_remainder_is_unknown() {
    let context = &mut setup();
    let function = function(context, "remainder");
    let entry = function.get_entry_block(context);
    let x = InvocationIndexOp::new(context, 0, 64);
    let eight = IndexConstantOp::new(context, 8);
    let zero = IndexConstantOp::new(context, 0);
    let wrapped = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        x.result(context),
        eight.result(context),
    );
    let invalid = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        x.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        x.get_operation(),
        eight.get_operation(),
        zero.get_operation(),
        wrapped.get_operation(),
        invalid.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(wrapped.result(context)).evaluate(&[19]),
        Some(3)
    );
    assert_eq!(
        analysis.fact(wrapped.result(context)).maximum(&[64]),
        Some(7)
    );
    assert_eq!(
        analysis.fact(invalid.result(context)),
        SparseIndexFactV1::Unknown
    );
}

#[test]
fn checked_tiled_fact_preserves_a_dynamic_component_value() {
    let context = &mut setup();
    let function = function(context, "dynamic_tiled_component");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let four = IndexConstantOp::new(context, 4);
    let sixteen = IndexConstantOp::new(context, 16);
    let component = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation.result(context),
        four.result(context),
    );
    let tiled = CheckedTiledIndex2DOp::new(
        context,
        invocation.result(context),
        component.result(context),
        sixteen.result(context),
        sixteen.result(context),
        sixteen.result(context),
        [64, 16, 16, 4],
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        four.get_operation(),
        sixteen.get_operation(),
        component.get_operation(),
        tiled.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let fact = analysis
        .fact(tiled.result(context))
        .checked_tiled_2d()
        .cloned()
        .expect("checked tiled fact");
    assert_eq!(fact.component(), component.result(context));
    assert_eq!(fact.runtime_layout(), [sixteen.result(context); 3]);
    assert_eq!(fact.geometry(), [64, 16, 16, 4]);
}

#[test]
fn static_ranked_dimension_propagates_as_a_constant() {
    let context = &mut setup();
    let function = function(context, "static_dimension");
    let entry = function.get_entry_block(context);
    let ty = RankedViewType::new(context, 32, false, vec![17]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![]).unwrap();
    let dimension = DimensionOp::new(context, view.result(context), 0).unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &dimension);
    append(context, entry, &ret);
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(dimension.result(context)).constant_value(),
        Some(17)
    );
}

#[test]
fn inconsistent_launch_contracts_fail_before_any_consumer_pass() {
    let context = &mut setup();
    let function = function(context, "inconsistent_launch");
    let entry = function.get_entry_block(context);
    let first = InvocationIndexOp::new(context, 0, 32);
    let second = InvocationIndexOp::new(context, 0, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &ret);
    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension: 0,
            first: 32,
            second: 64,
        }
    );
}

#[test]
fn dynamic_and_static_launch_contracts_cannot_share_one_dimension() {
    for (first, second) in [(0, 8), (8, 0)] {
        let context = &mut setup();
        let function = function(context, "mixed_launch_contract");
        let entry = function.get_entry_block(context);
        let first = InvocationIndexOp::new(context, 0, first);
        let second = InvocationIndexOp::new(context, 0, second);
        let ret = ReturnOp::new(context);
        append(context, entry, &first);
        append(context, entry, &second);
        append(context, entry, &ret);
        assert!(matches!(
            analyze_pliron_sparse_indices_v1(context, &function),
            Err(SparseIndexFailureV1::InconsistentLaunchExtent { dimension: 0, .. })
        ));
    }
}
