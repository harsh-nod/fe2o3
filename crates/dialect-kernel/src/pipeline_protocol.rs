use std::{error::Error, fmt};

use pliron::{
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NRegionsInterface, NResultsInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op, pliron_type},
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    r#type::{Type, Typed, TypedHandle},
    value::Value,
    verify_err, verify_err_noloc,
};

use crate::{MemorySpaceAttr, RankedViewOp, RankedViewType, is_index_type, ranked_view_type};

/// Largest ring admitted by the first target-neutral pipeline protocol.
pub const MAX_PIPELINE_BUFFERS_V1: u32 = 8;

/// Construction or local structural verification failure for pipeline IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineProtocolError {
    BufferCountOutOfBounds(u32),
    PrefetchDistanceOutOfBounds {
        buffers: u32,
        prefetch_distance: u32,
    },
    ForeignViewType,
    ReadOnlyView,
    NonWorkgroupView,
    StorageBufferDimensionMismatch {
        expected: u32,
        actual: u64,
    },
    ForeignPipelineType,
    ForeignIndexType {
        operand: usize,
    },
    MalformedPayload(&'static str),
}

impl fmt::Display for PipelineProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferCountOutOfBounds(buffers) => write!(
                formatter,
                "pipeline buffer count {buffers} is outside 2..={MAX_PIPELINE_BUFFERS_V1}"
            ),
            Self::PrefetchDistanceOutOfBounds {
                buffers,
                prefetch_distance,
            } => write!(
                formatter,
                "pipeline prefetch distance {prefetch_distance} must be in 1..{buffers}"
            ),
            Self::ForeignViewType => {
                formatter.write_str("pipeline storage is not a kernel ranked view")
            }
            Self::ReadOnlyView => {
                formatter.write_str("pipeline storage must be a writable ranked view")
            }
            Self::NonWorkgroupView => {
                formatter.write_str("pipeline storage must be in workgroup memory")
            }
            Self::StorageBufferDimensionMismatch { expected, actual } => write!(
                formatter,
                "pipeline storage leading extent {actual} does not match buffer count {expected}"
            ),
            Self::ForeignPipelineType => {
                formatter.write_str("pipeline event operand is not a kernel pipeline")
            }
            Self::ForeignIndexType { operand } => {
                write!(
                    formatter,
                    "pipeline event operand {operand} is not a kernel index"
                )
            }
            Self::MalformedPayload(message) => formatter.write_str(message),
        }
    }
}

impl Error for PipelineProtocolError {}

fn check_configuration(buffers: u32, prefetch_distance: u32) -> Result<(), PipelineProtocolError> {
    if !(2..=MAX_PIPELINE_BUFFERS_V1).contains(&buffers) {
        return Err(PipelineProtocolError::BufferCountOutOfBounds(buffers));
    }
    if prefetch_distance == 0 || prefetch_distance >= buffers {
        return Err(PipelineProtocolError::PrefetchDistanceOutOfBounds {
            buffers,
            prefetch_distance,
        });
    }
    Ok(())
}

/// Target-neutral ring-buffer protocol configuration.
///
/// The type carries no workload, target, instruction, or element-layout
/// semantics. Whole-function analysis proves the epoch lifecycle.
#[pliron_type(
    name = "kernel.pipeline",
    format = "`<` $buffers `,` $prefetch_distance `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PipelineType {
    buffers: u32,
    prefetch_distance: u32,
}

impl PipelineType {
    pub fn new(
        context: &Context,
        buffers: u32,
        prefetch_distance: u32,
    ) -> Result<TypedHandle<Self>, PipelineProtocolError> {
        check_configuration(buffers, prefetch_distance)?;
        Ok(Self::instantiate(
            Self {
                buffers,
                prefetch_distance,
            },
            context,
        ))
    }

    pub const fn buffers(&self) -> u32 {
        self.buffers
    }

    pub const fn prefetch_distance(&self) -> u32 {
        self.prefetch_distance
    }
}

impl Verify for PipelineType {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if let Err(error) = check_configuration(self.buffers, self.prefetch_distance) {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// One transition in the generic staged-storage lifecycle.
#[pliron_attr(name = "kernel.pipeline_event_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PipelineEventKindAttr {
    Stage,
    Commit,
    Wait,
    Consume,
    Discard,
    Release,
}

/// Binds one ring-buffer protocol to its physical workgroup allocation.
#[pliron_op(
    name = "kernel.pipeline_create",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>]
)]
pub struct PipelineCreateOp;

impl PipelineCreateOp {
    pub fn new(
        context: &mut Context,
        view: Value,
        buffers: u32,
        prefetch_distance: u32,
    ) -> Result<Self, PipelineProtocolError> {
        let view_type = validate_workgroup_view(view, context)?;
        validate_storage_shape(view_type, context, buffers)?;
        let pipeline = PipelineType::new(context, buffers, prefetch_distance)?;
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![pipeline.into()],
            vec![view],
            vec![],
            0,
        );
        Ok(Self::from_operation(operation))
    }

    pub fn view(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn pipeline(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }

    pub fn pipeline_type(&self, context: &Context) -> Option<TypedHandle<PipelineType>> {
        pipeline_type(self.pipeline(context), context)
    }
}

impl Verify for PipelineCreateOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let raw = self.get_operation();
        let raw = raw.deref(context);
        if raw.get_num_operands() != 1
            || raw.get_num_results() != 1
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || payload_attribute_count(&raw) != 0
        {
            return verify_err!(
                self.loc(context),
                PipelineProtocolError::MalformedPayload(
                    "kernel.pipeline_create requires one view, one pipeline result, and no attributes",
                )
            );
        }
        let Some(pipeline_type) = self.pipeline_type(context) else {
            return verify_err!(
                self.loc(context),
                PipelineProtocolError::ForeignPipelineType
            );
        };
        let storage = validate_workgroup_view(self.view(context), context).and_then(|view_type| {
            validate_storage_shape(view_type, context, pipeline_type.deref(context).buffers())
        });
        if let Err(error) = storage {
            return verify_err!(self.loc(context), error);
        }
        Ok(())
    }
}

/// Records an epoch-aware transition for one pipeline ring slot.
///
/// Epoch and slot are SSA indices so the same operation schema represents
/// static schedules and runtime-bounded loops.
#[pliron_op(
    name = "kernel.pipeline_event",
    format,
    interfaces = [NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (kernel_pipeline_event_kind: PipelineEventKindAttr)
)]
pub struct PipelineEventOp;

impl PipelineEventOp {
    pub fn new(
        context: &mut Context,
        pipeline: Value,
        epoch: Value,
        slot: Value,
        kind: PipelineEventKindAttr,
    ) -> Result<Self, PipelineProtocolError> {
        require_pipeline(pipeline, context)?;
        require_index(epoch, context, 1)?;
        require_index(slot, context, 2)?;
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![pipeline, epoch, slot],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_pipeline_event_kind(context, kind);
        Ok(op)
    }

    pub fn pipeline(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn epoch(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }

    pub fn slot(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(2)
    }

    pub fn kind(&self, context: &Context) -> Option<PipelineEventKindAttr> {
        self.get_attr_kernel_pipeline_event_kind(context)
            .map(|kind| *kind)
    }
}

impl Verify for PipelineEventOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let raw = self.get_operation();
        let raw = raw.deref(context);
        if raw.get_num_operands() != 3
            || raw.get_num_results() != 0
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || payload_attribute_count(&raw) != 1
            || self.kind(context).is_none()
        {
            return verify_err!(
                self.loc(context),
                PipelineProtocolError::MalformedPayload(
                    "kernel.pipeline_event requires pipeline, epoch, slot, and one event kind",
                )
            );
        }
        if let Err(error) = require_pipeline(self.pipeline(context), context)
            .and_then(|()| require_index(self.epoch(context), context, 1))
            .and_then(|()| require_index(self.slot(context), context, 2))
        {
            return verify_err!(self.loc(context), error);
        }
        Ok(())
    }
}

pub fn pipeline_type(value: Value, context: &Context) -> Option<TypedHandle<PipelineType>> {
    TypedHandle::from_handle(value.get_type(context), context).ok()
}

fn require_pipeline(value: Value, context: &Context) -> Result<(), PipelineProtocolError> {
    pipeline_type(value, context)
        .map(|_| ())
        .ok_or(PipelineProtocolError::ForeignPipelineType)
}

fn require_index(
    value: Value,
    context: &Context,
    operand: usize,
) -> Result<(), PipelineProtocolError> {
    if is_index_type(value, context) {
        Ok(())
    } else {
        Err(PipelineProtocolError::ForeignIndexType { operand })
    }
}

fn validate_workgroup_view(
    value: Value,
    context: &Context,
) -> Result<TypedHandle<RankedViewType>, PipelineProtocolError> {
    let view_type =
        ranked_view_type(value, context).ok_or(PipelineProtocolError::ForeignViewType)?;
    if !view_type.deref(context).writable() {
        return Err(PipelineProtocolError::ReadOnlyView);
    }
    let definition = value
        .defining_op()
        .ok_or(PipelineProtocolError::ForeignViewType)?;
    let definition = Operation::get_op_dyn(definition, context);
    let memory_space = definition
        .downcast_ref::<RankedViewOp>()
        .and_then(|view| view.memory_space(context));
    if memory_space != Some(MemorySpaceAttr::Workgroup) {
        return Err(PipelineProtocolError::NonWorkgroupView);
    }
    Ok(view_type)
}

fn validate_storage_shape(
    view_type: TypedHandle<RankedViewType>,
    context: &Context,
    buffers: u32,
) -> Result<(), PipelineProtocolError> {
    let actual = view_type.deref(context).shape()[0];
    if actual == u64::from(buffers) {
        Ok(())
    } else {
        Err(PipelineProtocolError::StorageBufferDimensionMismatch {
            expected: buffers,
            actual,
        })
    }
}

fn payload_attribute_count(operation: &Operation) -> usize {
    operation
        .attributes
        .0
        .keys()
        .filter(|key| *key != &*ATTR_KEY_DEBUG_INFO)
        .count()
}
