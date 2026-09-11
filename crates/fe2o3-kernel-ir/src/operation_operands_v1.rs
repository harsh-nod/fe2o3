use crate::{
    AssemblyOperandKind, MemoryIntrinsicOperation, OperationKind, ValueId, WaveOperation,
    WaveOperationKind,
};

impl OperationKind {
    pub fn operands(&self) -> Vec<ValueId> {
        let mut operands = Vec::with_capacity(self.operand_count());
        self.visit_operands(|operand| operands.push(operand));
        operands
    }

    /// Returns the exact number of SSA operands without allocating.
    pub fn operand_count(&self) -> usize {
        let mut count = 0_usize;
        self.visit_operands(|_| count = count.saturating_add(1));
        count
    }

    /// Visits every SSA operand in stable semantic order without allocating,
    /// stopping immediately when the visitor rejects an operand.
    pub fn try_visit_operands<E>(
        &self,
        mut visitor: impl FnMut(ValueId) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Constant(_)
            | Self::Intrinsic(_)
            | Self::Barrier(_)
            | Self::Fence(_)
            | Self::WorkgroupBarrier(_)
            | Self::WorkgroupMemory(_)
            | Self::Wave(WaveOperation {
                kind: WaveOperationKind::LaneId,
                ..
            }) => {}
            Self::MemoryIntrinsic(intrinsic) => match intrinsic {
                MemoryIntrinsicOperation::PointerDistance {
                    pointer, origin, ..
                } => {
                    visitor(*pointer)?;
                    visitor(*origin)?;
                }
                MemoryIntrinsicOperation::VolatileLoad { pointer, .. } => visitor(*pointer)?,
                MemoryIntrinsicOperation::VolatileStore { pointer, value, .. } => {
                    visitor(*pointer)?;
                    visitor(*value)?;
                }
                MemoryIntrinsicOperation::CopyNonOverlapping {
                    source,
                    destination,
                    count,
                    ..
                } => {
                    visitor(*source)?;
                    visitor(*destination)?;
                    visitor(*count)?;
                }
            },
            Self::Matrix(matrix) => match &matrix.kind {
                crate::MatrixOperationKind::MultiplyAccumulate {
                    lhs,
                    rhs,
                    accumulator,
                    ..
                } => lhs
                    .iter()
                    .chain(rhs)
                    .chain(accumulator)
                    .copied()
                    .try_for_each(&mut visitor)?,
                crate::MatrixOperationKind::ScaledMultiplyAccumulate {
                    lhs,
                    rhs,
                    accumulator,
                    ..
                } => lhs
                    .iter()
                    .chain(rhs)
                    .chain(accumulator)
                    .copied()
                    .try_for_each(&mut visitor)?,
                crate::MatrixOperationKind::LdsLoad { base, .. } => visitor(*base)?,
                crate::MatrixOperationKind::LdsStore { base, values, .. } => {
                    visitor(*base)?;
                    values.iter().copied().try_for_each(&mut visitor)?;
                }
            },
            Self::Gfx950LdsTranspose(transpose) => match transpose.kind {
                crate::Gfx950LdsTransposeOperationKindV1::Current { .. } => {}
                crate::Gfx950LdsTransposeOperationKindV1::Stage {
                    storage,
                    source_slice,
                    offset,
                    rows,
                    columns,
                    stride,
                    token_base,
                    reduction_base,
                    ..
                } => [
                    storage,
                    source_slice,
                    offset,
                    rows,
                    columns,
                    stride,
                    token_base,
                    reduction_base,
                ]
                .into_iter()
                .try_for_each(&mut visitor)?,
                crate::Gfx950LdsTransposeOperationKindV1::Publish { storage, .. }
                | crate::Gfx950LdsTransposeOperationKindV1::Read { storage, .. } => {
                    visitor(storage)?;
                }
            },
            Self::Unary { operand, .. } => visitor(*operand)?,
            Self::Binary { lhs, rhs, .. } | Self::Compare { lhs, rhs, .. } => {
                visitor(*lhs)?;
                visitor(*rhs)?;
            }
            Self::Cast { value, .. } => visitor(*value)?,
            Self::Select {
                condition,
                true_value,
                false_value,
            } => {
                visitor(*condition)?;
                visitor(*true_value)?;
                visitor(*false_value)?;
            }
            Self::Call { arguments, .. } => {
                arguments.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::Alloca { count, .. } => {
                count.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::SliceLength { slice } | Self::SliceData { slice } => visitor(*slice)?,
            Self::GetElementPointer { base, offset } => {
                visitor(*base)?;
                visitor(*offset)?;
            }
            Self::Load { pointer, .. } => visitor(*pointer)?,
            Self::GuardedLoad {
                pointer,
                predicate,
                fallback,
                ..
            } => {
                visitor(*pointer)?;
                visitor(*predicate)?;
                visitor(*fallback)?;
            }
            Self::GuardedStore {
                pointer,
                predicate,
                value,
                ..
            } => {
                visitor(*pointer)?;
                visitor(*predicate)?;
                visitor(*value)?;
            }
            Self::Store { pointer, value, .. } => {
                visitor(*pointer)?;
                visitor(*value)?;
            }
            Self::Atomic(atomic) => {
                visitor(atomic.pointer)?;
                atomic.value.iter().copied().try_for_each(&mut visitor)?;
                atomic.compare.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::Wave(wave) => match wave.kind {
                WaveOperationKind::LaneId => {}
                WaveOperationKind::Ballot { predicate }
                | WaveOperationKind::Any { predicate }
                | WaveOperationKind::All { predicate } => visitor(predicate)?,
                WaveOperationKind::ShuffleIndex {
                    value, source_lane, ..
                }
                | WaveOperationKind::BroadcastF32 {
                    value, source_lane, ..
                } => {
                    visitor(value)?;
                    visitor(source_lane)?;
                }
                WaveOperationKind::ReduceF32 { value, .. } => visitor(value)?,
            },
            Self::InlineAssembly(assembly) => {
                for operand in &assembly.operands {
                    match operand.kind {
                        AssemblyOperandKind::Input(value)
                        | AssemblyOperandKind::InOut { input: value, .. } => visitor(value)?,
                        AssemblyOperandKind::Output { .. }
                        | AssemblyOperandKind::ImmediateI32(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Visits every SSA operand in stable semantic order without allocating.
    pub fn visit_operands(&self, mut visitor: impl FnMut(ValueId)) {
        let result: Result<(), std::convert::Infallible> = self.try_visit_operands(|operand| {
            visitor(operand);
            Ok(())
        });
        match result {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }
}
