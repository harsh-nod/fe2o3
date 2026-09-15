//! Bounded records collected from the actual checked LLVM writer. This is a
//! structured translation derivation, not compiler-origin or machine proof.
use super::*;
use fe2o3_kernel_ir::{MemoryAccess, ReusablePhaseCheckLimitsV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredLlvmRecipeV1 {
    ErasedKernelContext,
    GlobalAlias,
    Immediate(ScalarType),
    InvocationIndex {
        kind: IndexKind,
        axis: Axis,
    },
    Binary {
        op: BinaryOp,
        scalar: ScalarType,
    },
    Compare {
        predicate: ComparePredicate,
        scalar: ScalarType,
    },
    Select(ScalarType),
    Cast {
        kind: CastKind,
        from: ScalarType,
        to: ScalarType,
    },
    GetElementPointer {
        space: KernelAddressSpace,
        element: ScalarType,
    },
    Load {
        element: ScalarType,
        access: MemoryAccess,
        guarded: bool,
    },
    Store {
        element: ScalarType,
        access: MemoryAccess,
        guarded: bool,
    },
    StaticLds {
        element: ScalarType,
        elements: u32,
        alignment: u32,
    },
    PhysicalWorkgroupBarrier {
        ordering: MemoryOrdering,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredWriterSegmentKindV1 {
    BlockParameters,
    Operation(DeclaredLlvmRecipeV1),
    Terminator,
    SplitEdges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredWriterSegmentV1 {
    pub function: u32,
    pub block: BlockId,
    pub operation: Option<u32>,
    pub kind: DeclaredWriterSegmentKindV1,
    pub llvm_bytes: u32,
    pub llvm_sha256: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub struct StructuredDeclaredKirToLlvmV1 {
    segments: Vec<DeclaredWriterSegmentV1>,
}
impl StructuredDeclaredKirToLlvmV1 {
    pub fn segments(&self) -> &[DeclaredWriterSegmentV1] {
        &self.segments
    }
    pub const fn establishes_machine_refinement(&self) -> bool {
        false
    }
    pub(super) fn retained_bytes(&self) -> usize {
        self.segments.capacity() * std::mem::size_of::<DeclaredWriterSegmentV1>()
    }
}

pub(super) struct Builder {
    segments: Vec<DeclaredWriterSegmentV1>,
    expected: usize,
    remaining_work: usize,
    unsupported: bool,
}
impl Builder {
    pub(super) fn new(
        module: &Module,
        limits: ReusablePhaseCheckLimitsV1,
    ) -> Result<Self, LoweringErrors> {
        let limits = ReusablePhaseCheckLimitsV1 {
            work: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work),
            temporary_bytes: limits
                .temporary_bytes
                .min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes),
        };
        let mut expected = 0usize;
        let mut work = 0usize;
        for f in &module.functions {
            work = add(module, work, 1)?;
            if let Some(body) = &f.body {
                for b in &body.blocks {
                    expected = add(module, expected, add(module, b.operations.len(), 3)?)?;
                    work = add(module, work, add(module, b.operations.len(), 1)?)?;
                    if work > limits.work {
                        return Err(error(module));
                    }
                }
            }
        }
        if work > limits.work {
            return Err(error(module));
        }
        let requested = expected
            .checked_mul(std::mem::size_of::<DeclaredWriterSegmentV1>())
            .ok_or_else(|| error(module))?;
        if requested > limits.temporary_bytes {
            return Err(error(module));
        }
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(expected)
            .map_err(|_| error(module))?;
        if segments
            .capacity()
            .checked_mul(std::mem::size_of::<DeclaredWriterSegmentV1>())
            .is_none_or(|n| n > limits.temporary_bytes)
        {
            return Err(error(module));
        }
        Ok(Self {
            segments,
            expected,
            remaining_work: limits.work - work,
            unsupported: false,
        })
    }
    pub(super) fn function(
        &mut self,
        lowerer: &FunctionLowerer<'_>,
    ) -> Result<u32, LoweringErrors> {
        self.spend(lowerer.module, 1)?;
        function_ordinal(lowerer.module, lowerer.function).ok_or_else(|| error(lowerer.module))
    }
    fn spend(&mut self, module: &Module, n: usize) -> Result<(), LoweringErrors> {
        self.remaining_work = self
            .remaining_work
            .checked_sub(n)
            .ok_or_else(|| error(module))?;
        Ok(())
    }
    pub(super) fn remaining_work(&self) -> usize {
        self.remaining_work
    }
    pub(super) fn record(
        &mut self,
        module: &Module,
        function: u32,
        block: BlockId,
        operation: Option<usize>,
        kind: Option<DeclaredWriterSegmentKindV1>,
        bytes: usize,
        digest: [u8; 32],
    ) -> Result<(), LoweringErrors> {
        self.spend(module, 1)?;
        let Some(kind) = kind else {
            self.unsupported = true;
            return Ok(());
        };
        if self.segments.len() >= self.expected {
            return Err(error(module));
        }
        self.segments.push(DeclaredWriterSegmentV1 {
            function,
            block,
            operation: operation
                .map(u32::try_from)
                .transpose()
                .map_err(|_| error(module))?,
            kind,
            llvm_bytes: u32::try_from(bytes).map_err(|_| error(module))?,
            llvm_sha256: digest,
        });
        Ok(())
    }
    pub(super) fn finish(
        self,
        module: &Module,
    ) -> Result<Option<StructuredDeclaredKirToLlvmV1>, LoweringErrors> {
        if self.unsupported {
            return Ok(None);
        }
        if self.segments.len() != self.expected {
            return Err(error(module));
        }
        Ok(Some(StructuredDeclaredKirToLlvmV1 {
            segments: self.segments,
        }))
    }
}

fn function_ordinal(module: &Module, function: &Function) -> Option<u32> {
    // This only locates an existing borrowed element. No address is serialized
    // or used as authority, and no pointer arithmetic/dereference is unsafe.
    // Checking the resulting element avoids a repeated whole-roster scan.
    let stride = std::mem::size_of::<Function>();
    let delta = (function as *const Function)
        .addr()
        .checked_sub(module.functions.as_ptr().addr())?;
    if stride == 0 || delta % stride != 0 {
        return None;
    }
    let ordinal = delta / stride;
    let original = module.functions.get(ordinal)?;
    std::ptr::eq(original, function)
        .then(|| u32::try_from(ordinal).ok())
        .flatten()
}

pub(super) struct DigestWriter<'a> {
    output: &'a mut dyn fmt::Write,
    hash: Sha256,
    bytes: usize,
}
impl<'a> DigestWriter<'a> {
    pub(super) fn new(output: &'a mut dyn fmt::Write) -> Self {
        Self {
            output,
            hash: Sha256::new(),
            bytes: 0,
        }
    }
    pub(super) fn finish(self) -> (usize, [u8; 32]) {
        (self.bytes, self.hash.finalize().into())
    }
}
impl fmt::Write for DigestWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.output.write_str(text)?;
        self.bytes = self.bytes.checked_add(text.len()).ok_or(fmt::Error)?;
        self.hash.update(text.as_bytes());
        Ok(())
    }
}

pub(super) fn recipe(
    lowerer: &FunctionLowerer<'_>,
    operation: &Operation,
) -> Option<DeclaredLlvmRecipeV1> {
    use DeclaredLlvmRecipeV1 as R;
    let scalar = |id| lowerer.value(id).1.as_scalar();
    let pointer = |id| {
        let Type::Pointer(p) = lowerer.value(id).1 else {
            return None;
        };
        Some((p.address_space, p.pointee.as_scalar()?))
    };
    Some(match &operation.kind {
        OperationKind::KernelContextIssue(_) => R::ErasedKernelContext,
        OperationKind::GlobalCapabilityBind(_) | OperationKind::GlobalCapabilityIndex(_) => {
            R::GlobalAlias
        }
        OperationKind::Constant(_) => R::Immediate(operation.results.first()?.ty.as_scalar()?),
        OperationKind::Intrinsic(i) => match i.kind {
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            } => R::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis,
            } => R::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis,
            },
            _ => return None,
        },
        OperationKind::Binary { op, lhs, .. } if !matches!(op, BinaryOp::Checked(_)) => R::Binary {
            op: *op,
            scalar: scalar(*lhs)?,
        },
        OperationKind::Compare { predicate, lhs, .. } => R::Compare {
            predicate: *predicate,
            scalar: scalar(*lhs)?,
        },
        OperationKind::Select { true_value, .. } => R::Select(scalar(*true_value)?),
        OperationKind::Cast { kind, value, to }
            if matches!(
                kind,
                CastKind::ZeroExtend
                    | CastKind::SignExtend
                    | CastKind::Truncate
                    | CastKind::Bitcast
            ) =>
        {
            R::Cast {
                kind: *kind,
                from: scalar(*value)?,
                to: to.as_scalar()?,
            }
        }
        OperationKind::GetElementPointer { base, .. } => {
            let (space, element) = pointer(*base)?;
            R::GetElementPointer { space, element }
        }
        OperationKind::Load { pointer: p, access } => R::Load {
            element: pointer(*p)?.1,
            access: *access,
            guarded: false,
        },
        OperationKind::GuardedLoad {
            pointer: p, access, ..
        } => R::Load {
            element: pointer(*p)?.1,
            access: *access,
            guarded: true,
        },
        OperationKind::Store {
            pointer: p, access, ..
        } => R::Store {
            element: pointer(*p)?.1,
            access: *access,
            guarded: false,
        },
        OperationKind::GuardedStore {
            pointer: p, access, ..
        } => R::Store {
            element: pointer(*p)?.1,
            access: *access,
            guarded: true,
        },
        OperationKind::WorkgroupMemory(memory) => {
            let WorkgroupMemoryExtent::Static(elements) = memory.extent else {
                return None;
            };
            R::StaticLds {
                element: memory.element.as_scalar()?,
                elements,
                alignment: memory.alignment,
            }
        }
        OperationKind::WorkgroupBarrier(barrier)
            if barrier.memory_scope == SynchronizationScope::Workgroup
                && barrier.semantics.address_spaces.len() == 1
                && barrier
                    .semantics
                    .address_spaces
                    .contains(&KernelAddressSpace::Workgroup)
                && barrier.convergence.scope() == SynchronizationScope::Workgroup
                && lowerer.target.requires_physical_workgroup_barrier() =>
        {
            R::PhysicalWorkgroupBarrier {
                ordering: barrier.semantics.ordering,
            }
        }
        _ => return None,
    })
}
fn add(module: &Module, a: usize, b: usize) -> Result<usize, LoweringErrors> {
    a.checked_add(b).ok_or_else(|| error(module))
}
fn error(module: &Module) -> LoweringErrors {
    declared_translation::error(
        module,
        "structured writer trace exceeds its exact census or existing resource ceiling",
    )
}

#[cfg(test)]
#[path = "structured_declared/tests.rs"]
mod tests;
