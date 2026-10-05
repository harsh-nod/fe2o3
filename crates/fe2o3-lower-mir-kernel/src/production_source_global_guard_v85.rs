// Stored recipe coordinates are descriptive until the original and optimized
// source owners independently authenticate them. An explicit predicate has no edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalSourceCfgGuardV85 {
    condition: SliceDefinition,
    edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlobalSourceGuardV85 {
    CfgEdge(GlobalSourceCfgGuardV85),
    ExplicitPredicate {
        condition: SliceDefinition,
        bound_comparison: SliceDefinition,
    },
}

impl GlobalSourceGuardV85 {
    #[cfg(test)]
    fn cfg_for_test(&self) -> &GlobalSourceCfgGuardV85 {
        let Self::CfgEdge(guard) = self else {
            panic!("test requires the original CFG recipe")
        };
        guard
    }

    #[cfg(test)]
    fn cfg_mut_for_test(&mut self) -> &mut GlobalSourceCfgGuardV85 {
        let Self::CfgEdge(guard) = self else {
            panic!("test requires the original CFG recipe")
        };
        guard
    }

    const fn condition(self) -> SliceDefinition {
        match self {
            Self::CfgEdge(guard) => guard.condition,
            Self::ExplicitPredicate { condition, .. } => condition,
        }
    }

    fn require_cfg_v26(self) -> SourceOwnedResultV18<GlobalSourceCfgGuardV85> {
        match self {
            Self::CfgEdge(guard) => Ok(guard),
            Self::ExplicitPredicate { .. } => Err(ProductionSourceOwnedViewErrorV18::Binding(
                "explicit source predicate requires versioned occurrence handoff",
            )),
        }
    }
}

#[cfg(test)]
mod typed_source_guard_v85_tests {
    use super::*;

    #[test]
    fn typed_source_guard_v85_preserves_cfg_identity_without_inventing_explicit_edge() {
        let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(3);
        let condition = SliceDefinition::FunctionArgument {
            function,
            argument: 5,
        };
        let bound_comparison = SliceDefinition::FunctionArgument {
            function,
            argument: 7,
        };
        let cfg = GlobalSourceCfgGuardV85 {
            condition,
            edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                source: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 { function, block: 4 },
                successor: 2,
            },
        };
        let old = GlobalSourceGuardV85::CfgEdge(cfg);
        assert_eq!(old.condition(), condition);
        assert_eq!(old.require_cfg_v26().unwrap(), cfg);
        let explicit = GlobalSourceGuardV85::ExplicitPredicate {
            condition,
            bound_comparison,
        };
        assert_eq!(explicit.condition(), condition);
        assert!(matches!(
            explicit.require_cfg_v26(),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "explicit source predicate requires versioned occurrence handoff"
            ))
        ));
        assert_ne!(explicit, old);
        assert_ne!(
            explicit,
            GlobalSourceGuardV85::ExplicitPredicate {
                condition: bound_comparison,
                bound_comparison: condition
            }
        );
    }
}
