//! Typed construction and verification smoke coverage for the LLVM dialect.

use pliron::{
    builtin::{
        op_interfaces::{OneResultInterface, SingleBlockRegionInterface},
        ops::ModuleOp,
        types::{IntegerType, Signedness},
    },
    context::Context,
    op::{Op, verify_op},
};
use pliron_llvm::{
    ops::{FreezeOp, FuncOp, ReturnOp, ZeroOp},
    types::FuncType,
};

fn module_with_return(has_value: bool) -> (Context, ModuleOp) {
    let mut context = Context::new();
    let i32_type = IntegerType::get(&context, 32, Signedness::Signless).into();
    let function_type = FuncType::get(&context, i32_type, vec![], false);
    let module = ModuleOp::new(
        &mut context,
        "pliron_llvm_smoke".try_into().expect("valid symbol"),
    );
    let function = FuncOp::new(
        &mut context,
        "typed_identity".try_into().expect("valid symbol"),
        function_type,
    );
    module.append_operation(&mut context, function.get_operation(), 0);

    let entry = function.get_or_create_entry_block(&mut context);
    let zero = ZeroOp::new(&mut context, i32_type);
    zero.get_operation().insert_at_back(entry, &context);
    let zero_value = zero.get_result(&context);
    let frozen = FreezeOp::new(&mut context, zero_value);
    frozen.get_operation().insert_at_back(entry, &context);
    let returned = has_value.then(|| frozen.get_result(&context));
    ReturnOp::new(&mut context, returned)
        .get_operation()
        .insert_at_back(entry, &context);

    (context, module)
}

#[test]
fn typed_llvm_dialect_operations_construct_and_verify_without_llvm_sys() {
    let (context, module) = module_with_return(true);
    verify_op(&module, &context).expect("typed LLVM dialect module verifies");
}

#[test]
fn typed_llvm_dialect_verifier_rejects_a_missing_return_value() {
    let (context, module) = module_with_return(false);
    assert!(verify_op(&module, &context).is_err());
}
