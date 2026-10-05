//! Finite-width values obtained only through the dialect's immutable folder.
use super::native_input_v1::{NativeTraceInputV1, NativeTraceRefusalV1, failure, native_resource};
use super::native_resources_v1::{fold_scratch, reserve_map, reserve_rows};
use super::*;
use crate::kir_bridge_v1::canonical_trace_v1::NativeSubjectV1;
use dialect_gpu::optimization_v1::{BinaryOp, CastOp, CompareOp, ConstantOp, SelectOp, UnaryOp};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, OperationKind, ScalarType, Type,
};
use pliron::{
    attribute::AttrObj,
    builtin::{attributes::IntegerAttr, types::IntegerType},
    opts::constants::ConstFoldInterface,
    r#type::{TypeHandle, Typed, TypedHandle},
    utils::apint::{APInt, bw},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeScalarV1 {
    pub bits: u128,
    pub width: u16,
    pub signed: bool,
}

impl NativeScalarV1 {
    pub(crate) fn signed_offset(self) -> Option<i128> {
        if self.signed {
            if self.width == 128 {
                Some(self.bits as i128)
            } else {
                Some(((self.bits << (128 - self.width)) as i128) >> (128 - self.width))
            }
        } else {
            i128::try_from(self.bits).ok()
        }
    }
}

pub(crate) struct NativeTraceStateV1<'a, 'g, 'b, 'w> {
    pub(crate) input: &'a NativeTraceInputV1<'a, 'g>,
    pub(crate) budget: &'b mut Budget<'w>,
    pub(crate) environment: HashMap<Value, NativeScalarV1>,
}

pub(super) fn failure_from_resource(
    error: crate::production_analysis::CanonicalRankedPolicyFailureV1,
) -> PlironTraceFailureV1 {
    match error {
        crate::production_analysis::CanonicalRankedPolicyFailureV1::Resource(error) => {
            native_resource(error)
        }
        _ => failure(0, 0, NativeTraceRefusalV1::Correspondence),
    }
}

impl<'a, 'g, 'b, 'w> NativeTraceStateV1<'a, 'g, 'b, 'w> {
    pub(crate) fn new(
        input: &'a NativeTraceInputV1<'a, 'g>,
        budget: &'b mut Budget<'w>,
        max_arguments: usize,
    ) -> Result<Self, PlironTraceFailureV1> {
        let environment = reserve_map(max_arguments, budget).map_err(failure_from_resource)?;
        Ok(Self {
            input,
            budget,
            environment,
        })
    }

    pub(crate) fn evaluate(
        &mut self,
        context: &Context,
        value: Value,
        total: &mut usize,
    ) -> Result<Option<NativeScalarV1>, PlironTraceFailureV1> {
        self.budget.charge_work(1).map_err(native_resource)?;
        let floor = self.budget.storage();
        let mut cache = reserve_map(MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1 * 2, self.budget)
            .map_err(failure_from_resource)?;
        let mut active = reserve_rows(MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1, self.budget)
            .map_err(failure_from_resource)?;
        let mut visits = 0;
        let result =
            self.evaluate_inner(context, value, total, &mut visits, &mut cache, &mut active);
        drop(cache);
        drop(active);
        self.budget
            .release_storage(self.budget.storage() - floor)
            .map_err(native_resource)?;
        result
    }

    fn evaluate_inner(
        &mut self,
        context: &Context,
        value: Value,
        total: &mut usize,
        visits: &mut usize,
        cache: &mut HashMap<Value, Option<NativeScalarV1>>,
        active: &mut Vec<Value>,
    ) -> Result<Option<NativeScalarV1>, PlironTraceFailureV1> {
        charge_trace_work_v1(total, 1)?;
        self.budget.charge_work(1).map_err(native_resource)?;
        *visits += 1;
        if *visits > MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1 {
            return Err(PlironTraceFailureV1::ResourceLimit);
        }
        if let Some(value) = self.environment.get(&value) {
            return Ok(Some(*value));
        }
        if let Some(value) = cache.get(&value) {
            return Ok(*value);
        }
        self.budget
            .charge_work(active.len())
            .map_err(native_resource)?;
        if active.contains(&value) {
            return Ok(None);
        }
        let Some((_, Type::Scalar(scalar))) = self.input.function.values.get(&value) else {
            return Ok(None);
        };
        if !scalar.is_integer() && *scalar != ScalarType::Bool {
            return Ok(None);
        }
        let Some(width) = scalar.bit_width() else {
            return Ok(None);
        };
        if width > 128 {
            return Ok(None);
        }
        let Some(pointer) = value.defining_op() else {
            return Ok(None);
        };
        let input = self.input;
        let Some(index) = input.function.occurrence_index.get(&pointer) else {
            return Err(failure(0, 0, NativeTraceRefusalV1::Correspondence));
        };
        let occurrence = &input.function.occurrences[*index];
        let NativeSubjectV1::Operation(operation) = occurrence.subject else {
            return Ok(None);
        };
        if !matches!(
            operation.kind,
            OperationKind::Constant(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
        ) {
            return Ok(None);
        }
        let raw = pointer.deref(context);
        if raw.get_num_operands() > 3 || raw.get_num_results() > 2 {
            return Ok(None);
        }
        // Every admitted operand/result has a finite integer width. In
        // particular gpu.index is never materialized as an IntegerAttr.
        let mut scratch_width = usize::from(width);
        for operand in raw.operands() {
            self.budget.charge_work(1).map_err(native_resource)?;
            let Some((_, Type::Scalar(scalar))) = self.input.function.values.get(&operand) else {
                return Ok(None);
            };
            if (!scalar.is_integer() && *scalar != ScalarType::Bool) || scalar.bit_width().is_none()
            {
                return Ok(None);
            }
            scratch_width = scratch_width.max(usize::from(scalar.bit_width().unwrap()));
        }
        let mut inputs = [None; 3];
        active.push(value);
        for (slot, operand) in raw.operands().enumerate() {
            inputs[slot] = self.evaluate_inner(context, operand, total, visits, cache, active)?;
        }
        active.pop();
        let floor = self.budget.storage();
        self.budget
            .charge_work(raw.get_num_operands() + raw.get_num_results() + 1)
            .map_err(native_resource)?;
        self.budget
            .reserve_storage(
                fold_scratch(scratch_width, raw.get_num_operands(), raw.get_num_results())
                    .map_err(native_resource)?,
            )
            .map_err(native_resource)?;
        let mut operands: Vec<Option<AttrObj>> = Vec::with_capacity(raw.get_num_operands());
        for (slot, native) in raw.operands().enumerate() {
            let attr = if let Some(value) = inputs[slot] {
                let ty = native.get_type(context);
                let Some(integer) = ty.deref(context).downcast_ref::<IntegerType>().map(|_| ty)
                else {
                    return Ok(None);
                };
                let integer =
                    TypedHandle::<IntegerType>::from_handle(integer, context).map_err(|_| {
                        failure(
                            occurrence.block,
                            occurrence.operation,
                            NativeTraceRefusalV1::UnsupportedValue,
                        )
                    })?;
                Some(Box::new(IntegerAttr::new(
                    integer,
                    APInt::from_u128(value.bits, bw(usize::from(value.width))),
                )) as AttrObj)
            } else {
                None
            };
            operands.push(attr);
        }
        let op = Operation::get_op_dyn(pointer, context);
        let results = if let Some(op) = op.downcast_ref::<ConstantOp>() {
            op.check_fold(context, &operands)
        } else if let Some(op) = op.downcast_ref::<UnaryOp>() {
            op.check_fold(context, &operands)
        } else if let Some(op) = op.downcast_ref::<BinaryOp>() {
            op.check_fold(context, &operands)
        } else if let Some(op) = op.downcast_ref::<CompareOp>() {
            op.check_fold(context, &operands)
        } else if let Some(op) = op.downcast_ref::<CastOp>() {
            op.check_fold(context, &operands)
        } else if let Some(op) = op.downcast_ref::<SelectOp>() {
            op.check_fold(context, &operands)
        } else {
            return Err(failure(
                occurrence.block,
                occurrence.operation,
                NativeTraceRefusalV1::Correspondence,
            ));
        };
        if results.len() != raw.get_num_results() {
            return Err(failure(
                occurrence.block,
                occurrence.operation,
                NativeTraceRefusalV1::Correspondence,
            ));
        }
        for (slot, result) in results.iter().enumerate() {
            let expected_type = raw.get_type(slot);
            let scalar = result
                .as_ref()
                .and_then(|attr| attr.downcast_ref::<IntegerAttr>())
                .and_then(|attr| {
                    let ty = attr.get_type();
                    let attr_value = attr.value();
                    (TypeHandle::from(ty) == expected_type && attr_value.bw() <= 128).then(|| {
                        NativeScalarV1 {
                            bits: attr_value.to_u128(),
                            width: attr_value.bw() as u16,
                            signed: ty.deref(context).is_signed(),
                        }
                    })
                });
            cache.insert(raw.get_result(slot), scalar);
        }
        drop(results);
        drop(operands);
        self.budget
            .release_storage(self.budget.storage() - floor)
            .map_err(native_resource)?;
        Ok(cache.get(&value).copied().flatten())
    }
}
