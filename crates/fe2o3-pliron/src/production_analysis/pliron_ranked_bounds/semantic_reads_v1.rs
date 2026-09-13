/// Requires a structurally verified immutable graph and its immediately
/// preceding operation in this same block. Establishes correspondence to one
/// existing access, not bounds, memory stability, or equality of read values.
pub(super) fn paired_read_access_v1(
    context: &Context,
    read: &SemanticTypedReadOp,
    previous: Option<Ptr<Operation>>,
) -> Option<Ptr<Operation>> {
    let previous = previous?;
    let operation = Operation::get_op_dyn(previous, context);
    let access = operation.downcast_ref::<RankedAccessOp>()?;
    let raw_access = previous.deref(context);
    let raw_read = read.get_operation().deref(context);
    (access.kind(context) == Some(AccessKindAttr::Read)
        && access.atomic_ordering(context).is_none()
        && access.atomic_scope(context).is_none()
        && access.checked_success(context).is_none()
        && read.guarded(context).is_none()
        && raw_access.get_num_operands() == raw_read.get_num_operands()
        && raw_access.operands().eq(raw_read.operands()))
    .then_some(previous)
}
