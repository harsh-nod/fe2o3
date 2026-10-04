/// Exact source guard identity. An explicit predicate has no control-flow edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionMixedRuntimeGuardV89 {
    /// An authenticated taken CFG edge.
    CfgEdge {
        /// Actual output condition definition.
        condition: SliceDefinition,
        /// Actual output edge coordinate.
        edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    },
    /// A predicate carried by the actual guarded write.
    ExplicitPredicate {
        /// Complete predicate, including every admitted conjunction.
        condition: SliceDefinition,
        /// Exact bounds-comparison result required by the original write recipe.
        bound_comparison: SliceDefinition,
    },
}

/// One versioned source/native/domain occurrence with independent formation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMixedRuntimeOccurrenceV89 {
    premise: usize,
    source: GlobalSourceAccessPairV18,
    domain: fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26,
    projection: Option<(fe2o3_kernel_ir::Axis, ValueId)>,
}

impl ProductionMixedRuntimeOccurrenceV89 {
    /// Index in the exact source-bound runtime premise roster.
    pub const fn premise_index(&self) -> usize {
        self.premise
    }
    /// Original source instance within its root.
    pub const fn original_instance(&self) -> usize {
        self.source.instance
    }
    /// Original source memory occurrence.
    pub const fn original_operation(&self) -> SliceOperation {
        self.source.input.logical.access.operation
    }
    /// Actual optimized memory occurrence.
    pub const fn output_operation(&self) -> SliceOperation {
        self.source.output.logical.access.operation
    }
    /// Original unconditional or CFG-dominated address formation.
    pub const fn original_address_formation(&self) -> SliceOperation {
        self.source.input.logical.address
    }
    /// Exact output address-formation operation, separate from dereference.
    pub const fn output_address_formation(&self) -> SliceOperation {
        self.source.output.logical.address
    }
    /// Actual output GEP operand definition, including zero selection.
    pub const fn output_address_index(&self) -> SliceDefinition {
        self.source.output.address_index
    }
    /// Typed actual guard; no synthetic edge is returned for an explicit predicate.
    pub const fn output_guard(&self) -> ProductionMixedRuntimeGuardV89 {
        match self.source.output.logical.guard {
            GlobalSourceGuardV85::CfgEdge(guard) => ProductionMixedRuntimeGuardV89::CfgEdge {
                condition: guard.condition,
                edge: guard.edge,
            },
            GlobalSourceGuardV85::ExplicitPredicate {
                condition,
                bound_comparison,
            } => ProductionMixedRuntimeGuardV89::ExplicitPredicate {
                condition,
                bound_comparison,
            },
        }
    }
    /// Conditional access domain with its exact source-bound values.
    pub const fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26 {
        self.domain
    }
    /// Direct invocation projection proved by the independent local analysis.
    pub const fn invocation_projection(&self) -> Option<(fe2o3_kernel_ir::Axis, ValueId)> {
        self.projection
    }
    /// Exact scalar memory shape, without runtime allocation authority.
    pub const fn memory_access(&self) -> MemoryAccess {
        self.source.output.memory
    }
    /// Conservative arithmetic envelope for explicit zero-selected formation.
    /// It does not require allocation of the inactive invocation tail.
    pub const fn explicit_formation_invocation_axis(&self) -> Option<fe2o3_kernel_ir::Axis> {
        match (self.source.output.logical.guard, self.projection) {
            (GlobalSourceGuardV85::ExplicitPredicate { .. }, Some((axis, _))) => Some(axis),
            _ => None,
        }
    }
    /// Formation representability is independent even when no access executes.
    pub const fn requires_address_formation_domain(&self) -> bool {
        true
    }
    /// Always false; this descriptive row is not a launch grant.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn cfg_runtime_occurrence_v26(
    premise: usize,
    source: GlobalSourceAccessPairV18,
    domain: fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26,
    projection: Option<(fe2o3_kernel_ir::Axis, ValueId)>,
) -> SourceOwnedResultV18<ProductionMixedRuntimeOccurrenceV26> {
    Ok(ProductionMixedRuntimeOccurrenceV26 {
        premise,
        source,
        cfg_guard: source.output.logical.guard.require_cfg_v26()?,
        domain,
        projection,
    })
}

fn predicated_runtime_occurrence_v89(
    premise: usize,
    source: GlobalSourceAccessPairV18,
    domain: fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26,
    projection: Option<(fe2o3_kernel_ir::Axis, ValueId)>,
) -> SourceOwnedResultV18<ProductionMixedRuntimeOccurrenceV89> {
    if matches!(
        source.output.logical.guard,
        GlobalSourceGuardV85::ExplicitPredicate { .. }
    ) && (!source.output.writing
        || projection.is_none()
        || !matches!(
            source.origin,
            GlobalSourceAccessOriginV18::SourceWriteV89 { .. }
        ))
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "predicated occurrence lacks source write or formation projection",
        ));
    }
    Ok(ProductionMixedRuntimeOccurrenceV89 {
        premise,
        source,
        domain,
        projection,
    })
}
