use crate::{
    AddressSpace, Atomic, AtomicKind, Barrier, CanonicalKernelIrVerificationResourceErrorV1,
    DiagnosticCode, Fence, MemoryOrdering, Operation, ScalarType, SynchronizationScope,
    TargetCapabilityRefV1, Type, VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
    WorkgroupBarrier, WorkgroupMemory, WorkgroupMemoryExtent, scope_can_observe_address_space,
    target_capability_is_supported_with_budget_v1, valid_failure_ordering,
    valid_synchronization_semantics, verification_type_facts_v15,
};

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) fn verify_barrier_v1(
        &mut self,
        barrier: &Barrier,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(
            barrier
                .semantics
                .address_spaces
                .len()
                .checked_add(3)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        let invalid_execution_scope = !matches!(
            barrier.execution_scope,
            SynchronizationScope::Subgroup | SynchronizationScope::Workgroup
        );
        let invalid_memory_scope = barrier.memory_scope.rank() < barrier.execution_scope.rank();
        let invalid_semantics = !valid_synchronization_semantics(
            barrier.memory_scope,
            barrier.semantics.ordering,
            &barrier.semantics.address_spaces,
        );
        if invalid_execution_scope || invalid_memory_scope || invalid_semantics {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidBarrier,
                "barrier requires subgroup/workgroup execution, a non-narrower legal memory scope, non-relaxed ordering, and shared writable memory",
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_fence_v1(
        &mut self,
        fence: &Fence,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(
            fence
                .semantics
                .address_spaces
                .len()
                .checked_add(2)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        if fence.memory_scope == SynchronizationScope::Invocation
            || !valid_synchronization_semantics(
                fence.memory_scope,
                fence.semantics.ordering,
                &fence.semantics.address_spaces,
            )
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidFence,
                "fence requires a scope wider than invocation, non-relaxed ordering, and memory visible at that scope",
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_workgroup_barrier_v1(
        &mut self,
        barrier: &WorkgroupBarrier,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(
            barrier
                .semantics
                .address_spaces
                .len()
                .checked_add(3)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        if barrier.convergence.scope() != SynchronizationScope::Workgroup {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidConvergence,
                "workgroup barrier requires a uniform workgroup convergence claim",
            )?;
        }
        if barrier.memory_scope.rank() < SynchronizationScope::Workgroup.rank()
            || !valid_synchronization_semantics(
                barrier.memory_scope,
                barrier.semantics.ordering,
                &barrier.semantics.address_spaces,
            )
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidBarrier,
                "workgroup barrier requires workgroup-or-wider legal memory semantics",
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_workgroup_memory_v1(
        &mut self,
        operation: &Operation,
        memory: &WorkgroupMemory,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(3)?;
        if !verification_type_facts_v15(&memory.element, self.budget)?.storable {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidWorkgroupMemory,
                "workgroup memory element type must be storable",
            )?;
        }
        if matches!(
            memory.extent,
            WorkgroupMemoryExtent::Static(0) | WorkgroupMemoryExtent::DynamicAtLeast(0)
        ) {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidWorkgroupMemory,
                "authenticated workgroup memory extent must be non-zero",
            )?;
        }
        if memory.extent.is_dynamic() {
            self.dynamic_workgroup_memory_declarations = self
                .dynamic_workgroup_memory_declarations
                .checked_add(1)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            if self.dynamic_workgroup_memory_declarations > 1 {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidWorkgroupMemory,
                    "a function may declare at most one dynamic workgroup-memory base",
                )?;
            }
        }
        self.verify_alignment_v1(memory.alignment, location)?;
        self.expect_pointer_result_v1(
            operation,
            &memory.element,
            AddressSpace::Workgroup,
            crate::AccessMode::ReadWrite,
            location,
        )
    }

    pub(crate) fn verify_atomic_v1(
        &mut self,
        operation: &Operation,
        atomic: &Atomic,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(8)?;
        let write = atomic.kind != AtomicKind::Load;
        let pointee =
            self.verify_pointer_access_v1(atomic.pointer, atomic.access, write, location)?;
        let valid_space = matches!(
            atomic.access.address_space,
            AddressSpace::Workgroup | AddressSpace::Global | AddressSpace::Generic
        );
        if !valid_space
            || atomic.scope == SynchronizationScope::Invocation
            || !scope_can_observe_address_space(atomic.scope, atomic.access.address_space)
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidAtomic,
                "atomic scope cannot observe the selected address space",
            )?;
        }

        let Some(pointee) = pointee else {
            return Ok(());
        };

        let Some(scalar) = pointee.as_scalar() else {
            return self.emit_fixed(
                location,
                DiagnosticCode::InvalidAtomic,
                "atomic pointee must be a scalar",
            );
        };
        let Some(width) = scalar.bit_width() else {
            return self.emit_fixed(
                location,
                DiagnosticCode::InvalidAtomic,
                "atomic pointee must have a fixed physical width",
            );
        };
        if atomic.access.alignment < u32::from(width.div_ceil(8)) {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidAtomic,
                "atomic alignment is smaller than the scalar width",
            )?;
        }
        let scalar_class_is_valid = scalar != ScalarType::Bool
            && scalar != ScalarType::Index
            && match atomic.kind {
                AtomicKind::Min
                | AtomicKind::Max
                | AtomicKind::BitAnd
                | AtomicKind::BitOr
                | AtomicKind::BitXor => scalar.is_integer(),
                _ => scalar.is_integer() || scalar.is_float(),
            };
        if !scalar_class_is_valid {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidAtomic,
                128,
                format_args!("{:?} does not support {scalar:?}", atomic.kind),
            )?;
        }
        if valid_space
            && atomic.scope != SynchronizationScope::Invocation
            && scope_can_observe_address_space(atomic.scope, atomic.access.address_space)
            && let Some(supported) = self.supported_capabilities
        {
            let required = TargetCapabilityRefV1::Atomic {
                width_bits: width,
                address_space: atomic.access.address_space,
                max_scope: atomic.scope,
            };
            if !target_capability_is_supported_with_budget_v1(required, supported, self.budget)? {
                self.emit_dynamic(
                    location,
                    DiagnosticCode::UnsupportedCapability,
                    512,
                    format_args!("target does not support required capability {required:?}"),
                )?;
            }
        }

        match atomic.kind {
            AtomicKind::Store => self.expect_no_results_v1(operation, location)?,
            AtomicKind::CompareExchange => {
                self.expect_result_pair_v1(operation, pointee, &Type::BOOL, location)?
            }
            _ => self.expect_single_result_type_v1(operation, pointee, location)?,
        }
        let valid_metadata = match atomic.kind {
            AtomicKind::Load => {
                atomic.value.is_none()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
                    && matches!(
                        atomic.ordering,
                        MemoryOrdering::Relaxed
                            | MemoryOrdering::Acquire
                            | MemoryOrdering::SequentiallyConsistent
                    )
            }
            AtomicKind::Store => {
                atomic.value.is_some()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
                    && matches!(
                        atomic.ordering,
                        MemoryOrdering::Relaxed
                            | MemoryOrdering::Release
                            | MemoryOrdering::SequentiallyConsistent
                    )
            }
            AtomicKind::CompareExchange => {
                atomic.value.is_some()
                    && atomic.compare.is_some()
                    && atomic
                        .failure_ordering
                        .is_some_and(|failure| valid_failure_ordering(atomic.ordering, failure))
            }
            _ => {
                atomic.value.is_some()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
            }
        };
        if !valid_metadata {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidAtomic,
                128,
                format_args!("malformed {:?} operands or orderings", atomic.kind),
            )?;
        }
        if let Some(value) = atomic.value {
            self.expect_type_v1(value, pointee, location)?;
        }
        if let Some(compare) = atomic.compare {
            self.expect_type_v1(compare, pointee, location)?;
        }
        Ok(())
    }
}
