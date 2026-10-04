use dialect_kernel::{
    DIALECT_NAME, IndexConstantOp, MAX_PIPELINE_BUFFERS_V1, MemorySpaceAttr, PipelineCreateOp,
    PipelineEventKindAttr, PipelineEventOp, PipelineProtocolError, PipelineType, RankedViewOp,
    RankedViewType, register_dialect,
};
use pliron::{
    context::Context,
    dialect::DialectName,
    op::{Op, verify_op},
    operation::Operation,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid dialect name"),
    )
    .expect("kernel dialect registration");
    context
}

fn workgroup_view(context: &mut Context, writable: bool, buffers: u64) -> RankedViewOp {
    let ty = RankedViewType::new(context, 16, writable, vec![buffers, 64])
        .expect("valid staged view type");
    RankedViewOp::new_in_space(context, ty, vec![], MemorySpaceAttr::Workgroup)
        .expect("valid workgroup view")
}

#[test]
fn configuration_is_bounded_and_requires_a_spare_slot() {
    let context = setup();
    assert_eq!(
        PipelineType::new(&context, 1, 1).unwrap_err(),
        PipelineProtocolError::BufferCountOutOfBounds(1),
    );
    assert_eq!(
        PipelineType::new(&context, MAX_PIPELINE_BUFFERS_V1 + 1, 1).unwrap_err(),
        PipelineProtocolError::BufferCountOutOfBounds(MAX_PIPELINE_BUFFERS_V1 + 1),
    );
    assert_eq!(
        PipelineType::new(&context, 3, 3).unwrap_err(),
        PipelineProtocolError::PrefetchDistanceOutOfBounds {
            buffers: 3,
            prefetch_distance: 3,
        },
    );
    assert!(PipelineType::new(&context, 3, 2).is_ok());
}

#[test]
fn create_binds_exactly_one_ring_dimension_to_writable_workgroup_storage() {
    let context = &mut setup();
    let matching = workgroup_view(context, true, 2);
    let create = PipelineCreateOp::new(context, matching.result(context), 2, 1)
        .expect("matching workgroup ring");
    assert!(verify_op(&create, context).is_ok());

    let mismatched = workgroup_view(context, true, 3);
    assert_eq!(
        PipelineCreateOp::new(context, mismatched.result(context), 2, 1)
            .err()
            .expect("mismatched ring is rejected"),
        PipelineProtocolError::StorageBufferDimensionMismatch {
            expected: 2,
            actual: 3,
        },
    );

    let read_only = workgroup_view(context, false, 2);
    assert_eq!(
        PipelineCreateOp::new(context, read_only.result(context), 2, 1)
            .err()
            .expect("read-only ring is rejected"),
        PipelineProtocolError::ReadOnlyView,
    );

    let global_ty = RankedViewType::new(context, 16, true, vec![2, 64]).unwrap();
    let global = RankedViewOp::new(context, global_ty, vec![]).unwrap();
    assert_eq!(
        PipelineCreateOp::new(context, global.result(context), 2, 1)
            .err()
            .expect("global ring is rejected"),
        PipelineProtocolError::NonWorkgroupView,
    );
}

#[test]
fn events_require_pipeline_and_index_operands() {
    let context = &mut setup();
    let view = workgroup_view(context, true, 2);
    let create = PipelineCreateOp::new(context, view.result(context), 2, 1).unwrap();
    let zero = IndexConstantOp::new(context, 0);
    let event = PipelineEventOp::new(
        context,
        create.pipeline(context),
        zero.result(context),
        zero.result(context),
        PipelineEventKindAttr::Stage,
    )
    .expect("well-typed event");
    assert!(verify_op(&event, context).is_ok());

    Operation::remove_operand(event.get_operation(), context, 2);
    assert!(verify_op(&event, context).is_err());
}
