#![cfg(all(test, feature = "pliron"))]
#![forbid(unsafe_code)]

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_mir::{
    MirBlockId,
    pliron::{
        MirBlockHandleError, MirDialectLimits, MirFunctionOp, MirModuleOp, register_mir_dialect,
    },
};
use fe2o3_pliron::{CONTEXT_IDENTITY_MARKER_KEY, ContextIdentityError};
use pliron::{
    basic_block::BasicBlock,
    context::{Context, Ptr},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
};

fn marker_key(value: &str) -> Identifier {
    value.try_into().expect("fixed marker key is valid")
}

fn take_marker(context: &mut Context, key: &str) -> Box<dyn Any> {
    let index = context
        .aux_data_map
        .remove(&marker_key(key))
        .expect("marker is indexed");
    context.aux_data.remove(index).expect("marker is present")
}

fn install_marker(context: &mut Context, key: &str, marker: Box<dyn Any>) {
    let index = context.aux_data.insert(marker);
    context.aux_data_map.insert(marker_key(key), index);
}

fn transplant_marker(owner: &mut Context, foreign: &mut Context, key: &str) {
    let owner_marker = take_marker(owner, key);
    drop(take_marker(foreign, key));
    install_marker(foreign, key, owner_marker);
}

fn module_and_function(context: &mut Context, identity: &str) -> (MirModuleOp, MirFunctionOp) {
    register_mir_dialect(context);
    let limits = MirDialectLimits::new(2, 3, 64).unwrap();
    let module = MirModuleOp::try_new(context, identity, limits).unwrap();
    let function = module
        .append_function(context, format!("{identity}::entry"), &[])
        .unwrap();
    (module, function)
}

#[cfg(test)]
fn raw_module_body_for_handle_ownership_test(
    module: &MirModuleOp,
    context: &Context,
) -> Ptr<BasicBlock> {
    module
        .get_operation()
        .deref(context)
        .get_region(0)
        .deref(context)
        .get_head()
        .expect("test module body")
}

#[cfg(test)]
fn raw_function_entry_for_handle_ownership_test(
    function: &MirFunctionOp,
    context: &Context,
) -> Ptr<BasicBlock> {
    function
        .get_operation()
        .deref(context)
        .get_region(0)
        .deref(context)
        .get_head()
        .expect("test function entry")
}

#[test]
fn module_body_and_entry_accessors_return_owner_bound_handles() {
    let mut context = Context::new();
    let (module, function) = module_and_function(&mut context, "owner");

    let body = module.body(&context).unwrap();
    let entry = function.entry_block(&context).unwrap();

    assert_eq!(body.function_count(&context), Ok(1));
    assert_eq!(body.verify(&context), Ok(()));
    assert!(!body.grants_authority());
    assert_eq!(entry.block_id(&context), Ok(MirBlockId(0)));
    assert_eq!(entry.verify(&context), Ok(()));
    assert!(!entry.grants_authority());
}

#[test]
fn body_and_entry_handles_reject_equal_slot_foreign_contexts_without_unwinding() {
    let mut owner = Context::new();
    let (owner_module, owner_function) = module_and_function(&mut owner, "owner");
    let owner_body = owner_module.body(&owner).unwrap();
    let owner_entry = owner_function.entry_block(&owner).unwrap();

    let mut foreign = Context::new();
    module_and_function(&mut foreign, "foreign");

    let body_rejection = catch_unwind(AssertUnwindSafe(|| owner_body.verify(&foreign)))
        .expect("foreign body verification must not unwind");
    let entry_rejection = catch_unwind(AssertUnwindSafe(|| owner_entry.verify(&foreign)))
        .expect("foreign entry verification must not unwind");
    assert_eq!(body_rejection, Err(MirBlockHandleError::ForeignContext));
    assert_eq!(entry_rejection, Err(MirBlockHandleError::ForeignContext));
}

#[test]
fn erased_body_and_entry_handles_report_stale_without_unwinding() {
    let mut context = Context::new();
    let (module, function) = module_and_function(&mut context, "owner");
    let body = module.body(&context).unwrap();
    let stale_body = body.clone();
    let stale_entry = function.entry_block(&context).unwrap();

    body.erase(&mut context).unwrap();

    let body_rejection = catch_unwind(AssertUnwindSafe(|| stale_body.function_count(&context)))
        .expect("stale body observation must not unwind");
    let entry_rejection = catch_unwind(AssertUnwindSafe(|| stale_entry.block_id(&context)))
        .expect("stale entry observation must not unwind");
    assert_eq!(body_rejection, Err(MirBlockHandleError::StaleHandle));
    assert_eq!(entry_rejection, Err(MirBlockHandleError::StaleHandle));
}

#[test]
fn transplanted_context_marker_cannot_transfer_body_or_entry_ownership() {
    let mut owner = Context::new();
    module_and_function(&mut owner, "owner");

    let mut foreign = Context::new();
    let (foreign_module, foreign_function) = module_and_function(&mut foreign, "foreign");
    let foreign_body = foreign_module.body(&foreign).unwrap();
    let foreign_entry = foreign_function.entry_block(&foreign).unwrap();

    transplant_marker(&mut owner, &mut foreign, CONTEXT_IDENTITY_MARKER_KEY);

    let expected = Err(MirBlockHandleError::ContextIdentity(
        ContextIdentityError::CorruptMarker,
    ));
    assert_eq!(foreign_body.verify(&foreign), expected);
    assert_eq!(foreign_entry.verify(&foreign), expected);
}

#[test]
fn same_context_parent_transplants_are_rejected_deterministically() {
    let mut context = Context::new();
    let (first_module, first_function) = module_and_function(&mut context, "first");
    let (second_module, second_function) = module_and_function(&mut context, "second");
    let first_body = first_module.body(&context).unwrap();
    let first_entry = first_function.entry_block(&context).unwrap();

    let raw_body = raw_module_body_for_handle_ownership_test(&first_module, &context);
    raw_body.unlink(&context);
    raw_body.insert_at_back(
        second_module.get_operation().deref(&context).get_region(0),
        &context,
    );

    let raw_entry = raw_function_entry_for_handle_ownership_test(&first_function, &context);
    raw_entry.unlink(&context);
    raw_entry.insert_at_back(
        second_function
            .get_operation()
            .deref(&context)
            .get_region(0),
        &context,
    );

    assert_eq!(
        first_body.verify(&context),
        Err(MirBlockHandleError::TransplantedHandle)
    );
    assert_eq!(
        first_entry.verify(&context),
        Err(MirBlockHandleError::TransplantedHandle)
    );
}

#[test]
fn wrong_operation_kinds_and_entry_positions_are_rejected_without_unwinding() {
    let mut context = Context::new();
    let (module, function) = module_and_function(&mut context, "owner");
    let entry = function.entry_block(&context).unwrap();

    let wrong_body = catch_unwind(AssertUnwindSafe(|| {
        MirModuleOp::from_operation(function.get_operation()).body(&context)
    }))
    .expect("wrong-kind module body access must not unwind");
    let wrong_entry = catch_unwind(AssertUnwindSafe(|| {
        MirFunctionOp::from_operation(module.get_operation()).entry_block(&context)
    }))
    .expect("wrong-kind function entry access must not unwind");
    assert_eq!(wrong_body.unwrap_err(), MirBlockHandleError::WrongKind);
    assert_eq!(wrong_entry.unwrap_err(), MirBlockHandleError::WrongKind);

    let second = function.append_block(&mut context).unwrap();
    let raw_second = function
        .get_operation()
        .deref(&context)
        .get_region(0)
        .deref(&context)
        .get_tail()
        .unwrap();
    raw_second.unlink(&context);
    raw_second.insert_at_front(
        function.get_operation().deref(&context).get_region(0),
        &context,
    );
    assert_eq!(
        entry.block_id(&context),
        Err(MirBlockHandleError::WrongKind)
    );
    assert_eq!(second.block_id(&context), Ok(MirBlockId(1)));
}

#[test]
fn borrow_conflicts_and_malformed_accessors_are_contained() {
    let mut context = Context::new();
    let (module, function) = module_and_function(&mut context, "owner");
    let body = module.body(&context).unwrap();
    let entry = function.entry_block(&context).unwrap();

    let raw_body = raw_module_body_for_handle_ownership_test(&module, &context);
    let exclusive_borrow = raw_body.deref_mut(&context);
    let body_rejection = catch_unwind(AssertUnwindSafe(|| body.verify(&context)))
        .expect("body borrow-conflict verification must not unwind");
    assert_eq!(body_rejection, Err(MirBlockHandleError::StaleHandle));
    drop(exclusive_borrow);

    let raw_entry = raw_function_entry_for_handle_ownership_test(&function, &context);
    let exclusive_borrow = raw_entry.deref_mut(&context);

    let borrow_rejection = catch_unwind(AssertUnwindSafe(|| entry.verify(&context)))
        .expect("borrow-conflict verification must not unwind");
    assert_eq!(borrow_rejection, Err(MirBlockHandleError::StaleHandle));
    drop(exclusive_borrow);

    let regionless = Operation::new(
        &mut context,
        MirFunctionOp::get_concrete_op_info(),
        vec![],
        vec![],
        vec![],
        0,
    );
    let malformed = MirFunctionOp::from_operation(regionless);
    let malformed_rejection = catch_unwind(AssertUnwindSafe(|| malformed.entry_block(&context)))
        .expect("malformed entry access must not unwind");
    assert_eq!(
        malformed_rejection.unwrap_err(),
        MirBlockHandleError::UpstreamPanicked
    );
}
