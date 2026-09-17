use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, DIALECT_NAME, DYNAMIC_EXTENT,
    IndexConstantOp, IndexUnknownOp, MemorySpaceAttr, PublicationAtomicAccessAttr,
    PublicationReadGuardOp, RankedAccessOp, RankedViewOp, RankedViewType, register_dialect,
};
use pliron::{
    context::Context,
    dialect::DialectName,
    op::{Op, verify_op},
    operation::Operation,
    value::Value,
};

fn context() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn view(
    context: &mut Context,
    extent: Value,
    width: u32,
    writable: bool,
    space: MemorySpaceAttr,
) -> Value {
    let ty = RankedViewType::new(context, width, writable, vec![DYNAMIC_EXTENT]).unwrap();
    RankedViewOp::new_in_space(context, ty, vec![extent], space)
        .unwrap()
        .result(context)
}

#[test]
fn publication_read_retains_real_acquire_and_exact_extent() {
    let context = &mut context();
    let extent = IndexConstantOp::new(context, 128).result(context);
    let index = IndexConstantOp::new(context, 7).result(context);
    let flags = view(context, extent, 32, true, MemorySpaceAttr::Global);
    let payload = view(context, extent, 32, true, MemorySpaceAttr::Global);
    for (value, kind) in [
        (1, PublicationAtomicAccessAttr::ReleaseRequestU32),
        (2, PublicationAtomicAccessAttr::ReleaseReadyU32),
    ] {
        let store =
            RankedAccessOp::new_publication_store_u32(context, flags, index, value).unwrap();
        verify_op(&store, context).unwrap();
        assert_eq!(store.publication_atomic_kind(context), Some(kind));
        assert_eq!(kind.stored_value(), Some(value));
        assert_eq!(store.publication_read_result(context), None);
    }
    let load = RankedAccessOp::new_publication_load_u32(context, flags, index).unwrap();
    verify_op(&load, context).unwrap();
    let acquired = load.publication_read_result(context).unwrap();
    let guard = PublicationReadGuardOp::new(context, index, extent, acquired).unwrap();
    verify_op(&guard, context).unwrap();
    let read = RankedAccessOp::new_predicated(
        context,
        AccessKindAttr::Read,
        payload,
        guard.result(context),
        guard.success(context),
    )
    .unwrap();
    verify_op(&read, context).unwrap();
    assert_eq!(read.checked_success(context), Some(guard.success(context)));
    assert_eq!(read.indices(context), [guard.result(context)]);
    let other_extent = IndexConstantOp::new(context, 128).result(context);
    let other_payload = view(context, other_extent, 32, true, MemorySpaceAttr::Global);
    assert!(
        RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Read,
            other_payload,
            guard.result(context),
            guard.success(context)
        )
        .is_err()
    );
    assert!(
        RankedAccessOp::new_predicated(
            context,
            AccessKindAttr::Write,
            payload,
            guard.result(context),
            guard.success(context)
        )
        .is_err()
    );
}

#[test]
fn publication_guard_rejects_symbols_wrong_cells_and_stale_order() {
    let context = &mut context();
    let extent = IndexConstantOp::new(context, 128).result(context);
    let index = IndexConstantOp::new(context, 0).result(context);
    let other_index = IndexConstantOp::new(context, 0).result(context);
    let flags = view(context, extent, 32, true, MemorySpaceAttr::Global);
    let unknown = IndexUnknownOp::new(context).result(context);
    assert!(PublicationReadGuardOp::new(context, index, extent, unknown).is_err());
    let load = RankedAccessOp::new_publication_load_u32(context, flags, index).unwrap();
    let acquired = load.publication_read_result(context).unwrap();
    assert!(PublicationReadGuardOp::new(context, other_index, extent, acquired).is_err());
    let guard = PublicationReadGuardOp::new(context, index, extent, acquired).unwrap();
    load.set_attr_kernel_atomic_ordering(context, AtomicOrderingAttr::Relaxed);
    assert!(verify_op(&load, context).is_err());
    assert!(verify_op(&guard, context).is_err());
    load.set_attr_kernel_atomic_ordering(context, AtomicOrderingAttr::Acquire);
    load.set_attr_kernel_atomic_scope(context, AtomicScopeAttr::Workgroup);
    assert!(verify_op(&load, context).is_err());
    assert!(verify_op(&guard, context).is_err());
    load.set_attr_kernel_atomic_scope(context, AtomicScopeAttr::System);
    Operation::remove_operand(load.get_operation(), context, 1);
    assert!(verify_op(&load, context).is_err());
    assert!(verify_op(&guard, context).is_err());
}

#[test]
fn publication_atomic_shapes_and_values_are_closed() {
    let context = &mut context();
    let extent = IndexConstantOp::new(context, 128).result(context);
    let index = IndexConstantOp::new(context, 0).result(context);
    let flags = view(context, extent, 32, true, MemorySpaceAttr::Global);
    for value in [0, 3, u32::MAX] {
        assert!(RankedAccessOp::new_publication_store_u32(context, flags, index, value).is_err());
    }
    for (width, writable, space) in [
        (16, true, MemorySpaceAttr::Global),
        (64, true, MemorySpaceAttr::Global),
        (32, false, MemorySpaceAttr::Global),
        (32, true, MemorySpaceAttr::Workgroup),
    ] {
        let invalid = view(context, extent, width, writable, space);
        assert!(RankedAccessOp::new_publication_load_u32(context, invalid, index).is_err());
        assert!(RankedAccessOp::new_publication_store_u32(context, invalid, index, 1).is_err());
    }
    let store = RankedAccessOp::new_publication_store_u32(context, flags, index, 1).unwrap();
    store.set_attr_kernel_publication_atomic(context, PublicationAtomicAccessAttr::AcquireU32);
    assert!(verify_op(&store, context).is_err());
    let load = RankedAccessOp::new_publication_load_u32(context, flags, index).unwrap();
    load.set_attr_kernel_publication_atomic(context, PublicationAtomicAccessAttr::ReleaseReadyU32);
    assert!(verify_op(&load, context).is_err());
}

#[test]
fn legacy_atomic_access_has_no_publication_result_or_guard() {
    let context = &mut context();
    let extent = IndexConstantOp::new(context, 128).result(context);
    let index = IndexConstantOp::new(context, 0).result(context);
    let flags = view(context, extent, 32, true, MemorySpaceAttr::Global);
    let legacy = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicRead,
        AtomicOrderingAttr::Acquire,
        AtomicScopeAttr::System,
        flags,
        vec![index],
    )
    .unwrap();
    verify_op(&legacy, context).unwrap();
    assert_eq!(legacy.publication_read_result(context), None);
    assert_eq!(legacy.publication_atomic_kind(context), None);
    legacy.set_attr_kernel_publication_atomic(context, PublicationAtomicAccessAttr::AcquireU32);
    assert!(verify_op(&legacy, context).is_err());
}
