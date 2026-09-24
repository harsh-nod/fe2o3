//! Allocation-free borrowed views of existing edge and local-effect semantics.
//! No purity, transitive call, trap, convergence or contract-authentication claim.

use std::collections::BTreeSet;

use crate::{
    AddressSpace, AssemblyEffect, BlockId, Gfx950LdsTransposeOperationKindV1, IntrinsicKind,
    MatrixOperationKind, MemoryEffect, MemoryElementType, MemoryIntrinsicOperation, MemoryOrdering,
    Operation, OperationKind, SynchronizationScope, Terminator, ValueId,
};

#[derive(Clone, Copy, Debug)]
pub enum KirAddressSpacesRefV1<'a> {
    Borrowed(&'a BTreeSet<AddressSpace>),
    Singleton(AddressSpace),
}

#[derive(Clone, Copy, Debug)]
pub enum KirLocalMemoryEffectRefV1<'a> {
    Allocate(AddressSpace),
    Read(AddressSpace),
    Write(AddressSpace),
    VolatileRead(AddressSpace),
    VolatileWrite(AddressSpace),
    Atomic {
        address_space: AddressSpace,
        scope: SynchronizationScope,
        ordering: MemoryOrdering,
    },
    Synchronize {
        execution_scope: SynchronizationScope,
        memory_scope: SynchronizationScope,
        address_spaces: KirAddressSpacesRefV1<'a>,
    },
    Fence {
        memory_scope: SynchronizationScope,
        ordering: MemoryOrdering,
        address_spaces: KirAddressSpacesRefV1<'a>,
    },
}

impl KirLocalMemoryEffectRefV1<'_> {
    /// Allocating compatibility/inspection conversion, outside inventory builds.
    pub fn to_owned(self) -> MemoryEffect {
        let spaces = |value: KirAddressSpacesRefV1<'_>| match value {
            KirAddressSpacesRefV1::Borrowed(values) => values.clone(),
            KirAddressSpacesRefV1::Singleton(value) => BTreeSet::from([value]),
        };
        match self {
            Self::Allocate(space) => MemoryEffect::Allocate(space),
            Self::Read(space) => MemoryEffect::Read(space),
            Self::Write(space) => MemoryEffect::Write(space),
            Self::VolatileRead(space) => MemoryEffect::VolatileRead(space),
            Self::VolatileWrite(space) => MemoryEffect::VolatileWrite(space),
            Self::Atomic {
                address_space,
                scope,
                ordering,
            } => MemoryEffect::Atomic {
                address_space,
                scope,
                ordering,
            },
            Self::Synchronize {
                execution_scope,
                memory_scope,
                address_spaces,
            } => MemoryEffect::Synchronize {
                execution_scope,
                memory_scope,
                address_spaces: spaces(address_spaces),
            },
            Self::Fence {
                memory_scope,
                ordering,
                address_spaces,
            } => MemoryEffect::Fence {
                memory_scope,
                ordering,
                address_spaces: spaces(address_spaces),
            },
        }
    }
}

impl Terminator {
    /// Cases in stored order, then default; conditional true edge then false.
    /// Borrows original argument slices and stops at the first visitor error.
    pub fn try_visit_edges_v1<'a, E>(
        &'a self,
        mut visitor: impl FnMut(BlockId, &'a [ValueId]) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Branch { target, arguments } => visitor(*target, arguments)?,
            Self::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            } => {
                visitor(*then_target, then_arguments)?;
                visitor(*else_target, else_arguments)?;
            }
            Self::Switch {
                cases,
                default_target,
                default_arguments,
                ..
            } => {
                for case in cases {
                    visitor(case.target, &case.arguments)?;
                }
                visitor(*default_target, default_arguments)?;
            }
            Self::IntegerSwitch {
                cases,
                default_target,
                default_arguments,
                ..
            } => {
                for case in cases {
                    visitor(case.target, &case.arguments)?;
                }
                visitor(*default_target, default_arguments)?;
            }
            Self::Return { .. } | Self::Unreachable => {}
        }
        Ok(())
    }
}

impl Operation {
    /// Exact local physical-effect order of memory_effects, without allocating.
    /// Call effects are unresolved here. Inspect the call roster separately;
    /// compiler ordering uses compiler_ordering_effects_v12. Neither an empty
    /// list nor a complete local list establishes purity/trap/convergence facts.
    /// Original payloads remain on the borrowed operation, including predicates,
    /// access flags, addresses, widths and atomic failure ordering.
    pub fn try_visit_local_memory_effects_v1<'a, E>(
        &'a self,
        mut visitor: impl FnMut(KirLocalMemoryEffectRefV1<'a>) -> Result<(), E>,
    ) -> Result<(), E> {
        use KirLocalMemoryEffectRefV1 as Effect;
        use OperationKind as Op;
        match &self.kind {
            Op::Execution(crate::ExecutionOperationV15::MaskedTileLoadU32 { .. }) => {
                visitor(Effect::Read(AddressSpace::Global))?
            }
            Op::Execution(_) => {}
            Op::Alloca { address_space, .. } => visitor(Effect::Allocate(*address_space))?,
            Op::Load { access, .. } | Op::GuardedLoad { access, .. } => {
                visitor(Effect::Read(access.address_space))?
            }
            Op::Store { access, .. } | Op::GuardedStore { access, .. } => {
                visitor(Effect::Write(access.address_space))?
            }
            Op::VectorLoad(load) => visitor(Effect::Read(load.access.memory.address_space))?,
            Op::VectorStore(store) => visitor(Effect::Write(store.access.memory.address_space))?,
            Op::Atomic(atomic) => visitor(Effect::Atomic {
                address_space: atomic.access.address_space,
                scope: atomic.scope,
                ordering: atomic.ordering,
            })?,
            Op::Barrier(barrier) => visitor(Effect::Synchronize {
                execution_scope: barrier.execution_scope,
                memory_scope: barrier.memory_scope,
                address_spaces: KirAddressSpacesRefV1::Borrowed(&barrier.semantics.address_spaces),
            })?,
            Op::WorkgroupBarrier(barrier) => visitor(Effect::Synchronize {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: barrier.memory_scope,
                address_spaces: KirAddressSpacesRefV1::Borrowed(&barrier.semantics.address_spaces),
            })?,
            Op::Fence(fence) => visitor(Effect::Fence {
                memory_scope: fence.memory_scope,
                ordering: fence.semantics.ordering,
                address_spaces: KirAddressSpacesRefV1::Borrowed(&fence.semantics.address_spaces),
            })?,
            Op::WorkgroupMemory(_) => visitor(Effect::Allocate(AddressSpace::Workgroup))?,
            Op::MemoryIntrinsic(intrinsic) => match intrinsic {
                MemoryIntrinsicOperation::PointerDistance { .. } => {}
                MemoryIntrinsicOperation::VolatileLoad {
                    element,
                    address_space,
                    ..
                } => {
                    if *element != MemoryElementType::Unit {
                        visitor(Effect::VolatileRead(*address_space))?;
                    }
                }
                MemoryIntrinsicOperation::VolatileStore {
                    element,
                    address_space,
                    ..
                } => {
                    if *element != MemoryElementType::Unit {
                        visitor(Effect::VolatileWrite(*address_space))?;
                    }
                }
                MemoryIntrinsicOperation::CopyNonOverlapping {
                    source_address_space,
                    destination_address_space,
                    ..
                } => {
                    visitor(Effect::Read(*source_address_space))?;
                    visitor(Effect::Write(*destination_address_space))?;
                }
            },
            Op::Matrix(matrix) => match matrix.kind {
                MatrixOperationKind::LdsLoad { .. } => {
                    visitor(Effect::Read(AddressSpace::Workgroup))?
                }
                MatrixOperationKind::LdsStore { .. } => {
                    visitor(Effect::Write(AddressSpace::Workgroup))?
                }
                MatrixOperationKind::MultiplyAccumulate { .. }
                | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => {}
            },
            Op::Gfx950LdsTranspose(transpose) => match transpose.kind {
                Gfx950LdsTransposeOperationKindV1::Current { .. } => {
                    visitor(Effect::Allocate(AddressSpace::Workgroup))?
                }
                Gfx950LdsTransposeOperationKindV1::Stage { .. } => {
                    visitor(Effect::Read(AddressSpace::Global))?;
                    visitor(Effect::Write(AddressSpace::Workgroup))?;
                }
                Gfx950LdsTransposeOperationKindV1::Publish { .. } => {
                    visitor(Effect::Synchronize {
                        execution_scope: SynchronizationScope::Workgroup,
                        memory_scope: SynchronizationScope::Workgroup,
                        address_spaces: KirAddressSpacesRefV1::Singleton(AddressSpace::Workgroup),
                    })?
                }
                Gfx950LdsTransposeOperationKindV1::Read { .. } => {
                    visitor(Effect::Read(AddressSpace::Workgroup))?
                }
            },
            Op::InlineAssembly(assembly) => {
                for effect in &assembly.declared_effects {
                    match effect {
                        AssemblyEffect::ReadGlobal => visitor(Effect::Read(AddressSpace::Global))?,
                        AssemblyEffect::WriteGlobal => {
                            visitor(Effect::Write(AddressSpace::Global))?
                        }
                        AssemblyEffect::ReadWorkgroup => {
                            visitor(Effect::Read(AddressSpace::Workgroup))?
                        }
                        AssemblyEffect::WriteWorkgroup => {
                            visitor(Effect::Write(AddressSpace::Workgroup))?
                        }
                        // Exact assembly payload remains an unresolved behavior site.
                        AssemblyEffect::Atomic
                        | AssemblyEffect::Barrier
                        | AssemblyEffect::ControlFlow => {}
                    }
                }
            }
            Op::Intrinsic(intrinsic) => match intrinsic.kind {
                IntrinsicKind::InvocationIndex { .. } | IntrinsicKind::LaunchExtent { .. } => {}
            },
            Op::Gfx942OrderedRegion(_)
            | Op::Gfx942OrderedProgram(_)
            | Op::Gfx942CompleteBodyDeclaration(_)
            | Op::Gfx942CompleteBodyStep(_)
            | Op::Gfx942PhysicalEntryDeclaration(_)
            | Op::Gfx942PhysicalEntryStep(_)
            | Op::VerificationContract(_)
            | Op::VectorLayoutConvert(_)
            | Op::Constant(_)
            | Op::Unary { .. }
            | Op::Binary { .. }
            | Op::Compare { .. }
            | Op::Cast { .. }
            | Op::Select { .. }
            | Op::Call { .. }
            | Op::SliceLength { .. }
            | Op::SliceData { .. }
            | Op::GetElementPointer { .. }
            | Op::Wave(_) => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "canonical_graph_visitors_v1_tests.rs"]
mod tests;
