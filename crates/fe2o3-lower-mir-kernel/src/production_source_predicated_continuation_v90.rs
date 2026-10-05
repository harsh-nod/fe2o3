mod continuation_occurrence_sealed_v90 {
    pub trait Sealed {}
}

/// Closed occurrence family for checked LICM and StoreConsensus continuations.
/// The default family remains V26. Explicit predicates retain a different
/// occurrence and guard type; neither family can be implemented by callers.
#[doc(hidden)]
pub trait ProductionContinuationOccurrenceV90:
    continuation_occurrence_sealed_v90::Sealed + Copy + std::fmt::Debug + Eq
{
    type RelocatedGuard: Copy + std::fmt::Debug + Eq;
    const PREDICATED: bool;
    fn premise_index(&self) -> usize;
    fn original_instance(&self) -> usize;
    fn original_operation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
    fn output_operation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
    fn original_address_formation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
    fn output_address_formation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
    fn output_address_index(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1;
    fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26;
    fn invocation_projection(&self) -> Option<(Axis, ValueId)>;
    fn memory_access(&self) -> MemoryAccess;
    fn relocate_guard<E>(
        &self,
        map: impl FnMut(
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        ) -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, E>,
    ) -> Result<Self::RelocatedGuard, E>;
    fn relocated_guard(&self, guard: Self::RelocatedGuard) -> ProductionMixedRuntimeGuardV89;
}

macro_rules! continuation_occurrence_v90 {
    ($ty:ty, $guard:ty, $predicated:literal, $relocate:expr, $project:expr) => {
        impl continuation_occurrence_sealed_v90::Sealed for $ty {}
        impl ProductionContinuationOccurrenceV90 for $ty {
            type RelocatedGuard = $guard;
            const PREDICATED: bool = $predicated;
            fn premise_index(&self) -> usize {
                <$ty>::premise_index(self)
            }
            fn original_instance(&self) -> usize {
                <$ty>::original_instance(self)
            }
            fn original_operation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                <$ty>::original_operation(self)
            }
            fn output_operation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                <$ty>::output_operation(self)
            }
            fn original_address_formation(
                &self,
            ) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                <$ty>::original_address_formation(self)
            }
            fn output_address_formation(
                &self,
            ) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                <$ty>::output_address_formation(self)
            }
            fn output_address_index(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
                <$ty>::output_address_index(self)
            }
            fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26 {
                <$ty>::domain(self)
            }
            fn invocation_projection(&self) -> Option<(Axis, ValueId)> {
                <$ty>::invocation_projection(self)
            }
            fn memory_access(&self) -> MemoryAccess {
                <$ty>::memory_access(self)
            }
            fn relocate_guard<E>(
                &self,
                mut map: impl FnMut(
                    fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
                )
                    -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, E>,
            ) -> Result<Self::RelocatedGuard, E> {
                ($relocate)(self, &mut map)
            }
            fn relocated_guard(
                &self,
                guard: Self::RelocatedGuard,
            ) -> ProductionMixedRuntimeGuardV89 {
                ($project)(self, guard)
            }
        }
    };
}
continuation_occurrence_v90!(
    ProductionMixedRuntimeOccurrenceV26,
    fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    false,
    relocate_cfg_guard_v90,
    |row: &ProductionMixedRuntimeOccurrenceV26, condition| {
        ProductionMixedRuntimeGuardV89::CfgEdge {
            condition,
            edge: row.output_guard_edge(),
        }
    }
);
continuation_occurrence_v90!(
    ProductionMixedRuntimeOccurrenceV89,
    ProductionMixedRuntimeGuardV89,
    true,
    relocate_explicit_guard_v90,
    |_row: &ProductionMixedRuntimeOccurrenceV89, guard| guard
);

fn relocate_cfg_guard_v90<E>(
    row: &ProductionMixedRuntimeOccurrenceV26,
    map: &mut impl FnMut(
        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    ) -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, E>,
) -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, E> {
    map(row.output_guard_condition())
}

fn relocate_explicit_guard_v90<E>(
    row: &ProductionMixedRuntimeOccurrenceV89,
    map: &mut impl FnMut(
        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    ) -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, E>,
) -> Result<ProductionMixedRuntimeGuardV89, E> {
    Ok(match row.output_guard() {
        ProductionMixedRuntimeGuardV89::CfgEdge { condition, edge } => {
            ProductionMixedRuntimeGuardV89::CfgEdge {
                condition: map(condition)?,
                edge,
            }
        }
        ProductionMixedRuntimeGuardV89::ExplicitPredicate {
            condition,
            bound_comparison,
        } => ProductionMixedRuntimeGuardV89::ExplicitPredicate {
            condition: map(condition)?,
            bound_comparison: map(bound_comparison)?,
        },
    })
}
