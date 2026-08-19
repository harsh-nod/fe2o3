#![cfg(feature = "pliron")]
#![forbid(unsafe_code)]

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_mir::{
    MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_TYPES, MirBlockId, MirTypeId,
    pliron::{
        MirBlockHandleError, MirBlockIdAttr, MirBlockOp, MirDialectBuildError, MirDialectLimitKind,
        MirDialectLimits, MirFunctionOp, MirIdentityAttr, MirLimitsAttr, MirModuleOp, MirReturnOp,
        MirTypeRef, mir_dialect_registration, register_mir_dialect,
    },
};
use fe2o3_pliron::{
    CONTEXT_IDENTITY_MARKER_KEY, ContextIdentityError, PLIRON_REVISION, PlironSession, ShellLimits,
};
use pliron::{
    context::Context,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    result::ExpectOk,
    r#type::{Type, TypeHandle, verify_type},
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

#[test]
fn explicit_registration_is_duplicate_safe_and_session_scoped() {
    let registration = mir_dialect_registration().expect("fixed MIR dialect name");
    let mut session = PlironSession::new(ShellLimits::default(), [registration])
        .expect("explicit MIR registration");
    assert_eq!(session.manifest().pliron_revision(), PLIRON_REVISION);
    assert_eq!(session.manifest().registration_order(), &["mir".to_owned()]);

    session
        .with_context_mut(|context| {
            register_mir_dialect(context);
            register_mir_dialect(context);

            assert_eq!(MirModuleOp::get_opid_static().to_string(), "mir.module");
            assert_eq!(MirFunctionOp::get_opid_static().to_string(), "mir.func");
            assert_eq!(MirBlockOp::get_opid_static().to_string(), "mir.block");
            assert_eq!(MirTypeRef::get_type_id_static().to_string(), "mir.type_ref");
        })
        .expect("healthy session");
}

#[test]
fn typed_module_function_and_blocks_round_trip_through_pliron() {
    let mut context = Context::new();
    register_mir_dialect(&mut context);

    let limits = MirDialectLimits::new(4, 4, 128).unwrap();
    let module = MirModuleOp::try_new(&mut context, "crate::kernels", limits).unwrap();
    let function = module
        .append_function(
            &mut context,
            "crate::kernels::vecadd",
            &[MirTypeId(0), MirTypeId(1)],
        )
        .unwrap();
    let second_block = function.append_block(&mut context).unwrap();

    assert_eq!(module.function_count(&context), 1);
    assert_eq!(function.block_count(&context), 2);
    assert_eq!(second_block.block_id(&context), Ok(MirBlockId(1)));
    assert_eq!(second_block.verify(&context), Ok(()));
    assert!(!second_block.grants_authority());
    assert_eq!(
        MirBlockOp::from_operation(
            function
                .entry_block(&context)
                .deref(&context)
                .get_head()
                .unwrap()
        )
        .block_id(&context),
        Some(MirBlockId(0))
    );
    verify_operation(module.get_operation(), &context).unwrap();

    let printed = module
        .get_operation()
        .deref(&context)
        .disp(&context)
        .to_string();
    assert!(printed.contains("mir.module"));
    assert!(printed.contains("mir.func"));
    assert!(printed.contains("mir.block"));
    assert!(printed.contains("mir.type_ref"));

    let mut parsed_context = Context::new();
    register_mir_dialect(&mut parsed_context);
    let parsed = parse_from_str(Operation::top_level_parser(), &mut parsed_context, &printed)
        .expect_ok(&parsed_context);
    verify_operation(parsed, &parsed_context).unwrap();
}

#[test]
fn bounded_construction_rejects_before_mutating_ir() {
    assert_eq!(
        MirDialectLimits::new(0, 1, 1),
        Err(MirDialectBuildError::InvalidLimit {
            kind: MirDialectLimitKind::Functions,
            value: 0,
            hard_limit: dialect_mir::MAX_EXECUTABLE_FUNCTIONS,
        })
    );

    let mut empty_context = Context::new();
    let limits = MirDialectLimits::new(1, 1, 8).unwrap();
    let error = match MirModuleOp::try_new(&mut empty_context, "identity-too-long", limits) {
        Ok(_) => panic!("oversized identity was accepted"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        MirDialectBuildError::IdentityTooLong {
            bytes: "identity-too-long".len(),
            limit: 8,
        }
    );
    assert!(empty_context.is_ir_empty());

    let mut context = Context::new();
    let module = MirModuleOp::try_new(&mut context, "module", limits).unwrap();
    let function = module
        .append_function(&mut context, "first", &[MirTypeId(0)])
        .unwrap();
    assert_eq!(
        module.append_function(&mut context, "second", &[]).err(),
        Some(MirDialectBuildError::FunctionLimitExceeded { limit: 1 })
    );
    assert_eq!(module.function_count(&context), 1);
    assert_eq!(
        function.append_block(&mut context),
        Err(MirDialectBuildError::BlockLimitExceeded { limit: 1 })
    );
    assert_eq!(function.block_count(&context), 1);
    verify_operation(module.get_operation(), &context).unwrap();

    let wider_limits = MirDialectLimits::new(2, 1, 8).unwrap();
    let wider = MirModuleOp::try_new(&mut context, "wider", wider_limits).unwrap();
    assert_eq!(
        wider
            .append_function(
                &mut context,
                "badtype",
                &[MirTypeId(MAX_EXECUTABLE_TYPES as u32)],
            )
            .err(),
        Some(MirDialectBuildError::TypeIdOutOfRange(MirTypeId(
            MAX_EXECUTABLE_TYPES as u32
        )))
    );
    assert_eq!(wider.function_count(&context), 0);
}

#[test]
fn verifier_rejects_duplicate_identities_and_hostile_attributes() {
    let mut context = Context::new();
    let limits = MirDialectLimits::new(3, 2, 64).unwrap();
    let duplicate = MirModuleOp::try_new(&mut context, "module", limits).unwrap();
    duplicate
        .append_function(&mut context, "same", &[])
        .unwrap();
    duplicate
        .append_function(&mut context, "same", &[])
        .unwrap();
    assert!(verify_operation(duplicate.get_operation(), &context).is_err());

    let oversized = MirModuleOp::try_new(&mut context, "module2", limits).unwrap();
    oversized.set_attr_module_identity(&context, MirIdentityAttr::new("x".repeat(65)));
    assert!(verify_operation(oversized.get_operation(), &context).is_err());

    let inconsistent = MirModuleOp::try_new(&mut context, "module_limits", limits).unwrap();
    let inconsistent_function = inconsistent
        .append_function(&mut context, "function", &[])
        .unwrap();
    inconsistent_function.set_attr_function_limits(
        &context,
        MirLimitsAttr::new(MirDialectLimits::new(3, 1, 64).unwrap()),
    );
    assert!(verify_operation(inconsistent.get_operation(), &context).is_err());

    let invalid_block = MirModuleOp::try_new(&mut context, "module3", limits).unwrap();
    let function = invalid_block
        .append_function(&mut context, "function", &[])
        .unwrap();
    let marker = MirBlockOp::from_operation(
        function
            .entry_block(&context)
            .deref(&context)
            .get_head()
            .unwrap(),
    );
    marker.set_attr_block_id(
        &context,
        MirBlockIdAttr::new(MirBlockId(MAX_EXECUTABLE_BLOCKS as u32)),
    );
    assert!(verify_operation(invalid_block.get_operation(), &context).is_err());
}

#[test]
fn verifier_rejects_malformed_structure_and_nesting() {
    let mut context = Context::new();
    register_mir_dialect(&mut context);

    let regionless = Operation::new(
        &mut context,
        MirModuleOp::get_concrete_op_info(),
        vec![],
        vec![],
        vec![],
        0,
    );
    assert!(verify_operation(regionless, &context).is_err());

    let limits = MirDialectLimits::new(2, 2, 64).unwrap();
    let misplaced = MirModuleOp::try_new(&mut context, "misplaced", limits).unwrap();
    MirReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(misplaced.body(&context), &context);
    assert!(verify_operation(misplaced.get_operation(), &context).is_err());

    let missing_marker = MirModuleOp::try_new(&mut context, "missing", limits).unwrap();
    let function = missing_marker
        .append_function(&mut context, "function", &[])
        .unwrap();
    let marker = function
        .entry_block(&context)
        .deref(&context)
        .get_head()
        .unwrap();
    Operation::erase(marker, &mut context);
    assert!(verify_operation(missing_marker.get_operation(), &context).is_err());

    let detached = MirModuleOp::try_new(&mut context, "detached", limits).unwrap();
    let function = detached
        .append_function(&mut context, "function", &[])
        .unwrap();
    function.get_operation().unlink(&context);
    assert!(verify_operation(function.get_operation(), &context).is_err());
}

#[test]
fn verifier_rejects_untrusted_type_references() {
    let mut context = Context::new();
    register_mir_dialect(&mut context);
    let text = format!("mir.type_ref {}", MAX_EXECUTABLE_TYPES);
    let ty = parse_from_str(TypeHandle::parser(()), &mut context, &text).expect_ok(&context);
    assert!(verify_type(&*ty.deref(&context), &context).is_err());
}

#[test]
fn owner_bound_block_rejects_an_equal_slot_foreign_context_without_unwinding() {
    let limits = MirDialectLimits::new(1, 2, 64).unwrap();
    let mut owner = Context::new();
    let owner_module = MirModuleOp::try_new(&mut owner, "owner", limits).unwrap();
    let owner_function = owner_module
        .append_function(&mut owner, "owner::entry", &[])
        .unwrap();
    let owner_block = owner_function.append_block(&mut owner).unwrap();

    let mut foreign = Context::new();
    let foreign_module = MirModuleOp::try_new(&mut foreign, "foreign", limits).unwrap();
    let foreign_function = foreign_module
        .append_function(&mut foreign, "foreign::entry", &[])
        .unwrap();
    foreign_function.append_block(&mut foreign).unwrap();

    let rejection = catch_unwind(AssertUnwindSafe(|| owner_block.verify(&foreign)))
        .expect("foreign handle verification must not unwind");
    assert_eq!(rejection, Err(MirBlockHandleError::ForeignContext));
    assert_eq!(owner_block.verify(&owner), Ok(()));
}

#[test]
fn erased_owner_bound_block_returns_a_stale_error_without_unwinding() {
    let limits = MirDialectLimits::new(1, 2, 64).unwrap();
    let mut context = Context::new();
    let module = MirModuleOp::try_new(&mut context, "owner", limits).unwrap();
    let function = module
        .append_function(&mut context, "owner::entry", &[])
        .unwrap();
    let block = function.append_block(&mut context).unwrap();
    let stale = block.clone();

    block.erase(&mut context).unwrap();
    let rejection = catch_unwind(AssertUnwindSafe(|| stale.block_id(&context)))
        .expect("stale handle observation must not unwind");
    assert_eq!(rejection, Err(MirBlockHandleError::StaleHandle));
    assert_eq!(function.block_count(&context), 1);
    verify_operation(module.get_operation(), &context).unwrap();
}

#[test]
fn transplanted_context_marker_cannot_transfer_block_ownership() {
    let limits = MirDialectLimits::new(1, 2, 64).unwrap();
    let mut owner = Context::new();
    let owner_module = MirModuleOp::try_new(&mut owner, "owner", limits).unwrap();
    let owner_function = owner_module
        .append_function(&mut owner, "owner::entry", &[])
        .unwrap();
    owner_function.append_block(&mut owner).unwrap();

    let mut foreign = Context::new();
    let foreign_module = MirModuleOp::try_new(&mut foreign, "foreign", limits).unwrap();
    let foreign_function = foreign_module
        .append_function(&mut foreign, "foreign::entry", &[])
        .unwrap();
    let foreign_block = foreign_function.append_block(&mut foreign).unwrap();

    transplant_marker(&mut owner, &mut foreign, CONTEXT_IDENTITY_MARKER_KEY);
    let rejection = catch_unwind(AssertUnwindSafe(|| foreign_block.verify(&foreign)))
        .expect("transplanted-marker verification must not unwind");
    assert_eq!(
        rejection,
        Err(MirBlockHandleError::ContextIdentity(
            ContextIdentityError::CorruptMarker
        ))
    );
}

#[test]
fn owner_bound_block_contains_a_pointer_borrow_panic_path() {
    let limits = MirDialectLimits::new(1, 2, 64).unwrap();
    let mut context = Context::new();
    let module = MirModuleOp::try_new(&mut context, "owner", limits).unwrap();
    let function = module
        .append_function(&mut context, "owner::entry", &[])
        .unwrap();
    let block = function.append_block(&mut context).unwrap();
    let raw_block = function
        .get_operation()
        .deref(&context)
        .get_region(0)
        .deref(&context)
        .get_tail()
        .unwrap();
    let _exclusive_borrow = raw_block.deref_mut(&context);

    let rejection = catch_unwind(AssertUnwindSafe(|| block.verify(&context)))
        .expect("borrow-conflict verification must not unwind");
    assert_eq!(rejection, Err(MirBlockHandleError::StaleHandle));
}
