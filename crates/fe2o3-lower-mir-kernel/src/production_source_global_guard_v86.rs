// Versioned coordinate carriage only. Neither this conversion nor the native
// observation discharges the original-call recipe or formation/access domains.
impl GlobalSourceGuardV85 {
    fn wire_v86(self) -> fe2o3_kernel_descriptor::mixed_conditional_v86::MixedAccessGuardV86 {
        use fe2o3_kernel_descriptor::mixed_conditional_v26 as wire;
        use fe2o3_kernel_descriptor::mixed_conditional_v86::MixedAccessGuardV86 as Guard;
        fn definition(value: SliceDefinition) -> wire::MixedDefinitionV26 {
            match value {
                SliceDefinition::FunctionArgument { function, argument } => {
                    wire::MixedDefinitionV26::FunctionArgument {
                        function: function.0,
                        argument,
                    }
                }
                SliceDefinition::BlockArgument { block, argument } => {
                    wire::MixedDefinitionV26::BlockArgument {
                        function: block.function.0,
                        block: block.block,
                        argument,
                    }
                }
                SliceDefinition::Result { operation, result } => wire::MixedDefinitionV26::Result {
                    operation: wire::MixedOperationV26 {
                        function: operation.block.function.0,
                        block: operation.block.block,
                        operation: operation.operation,
                    },
                    result,
                },
            }
        }
        match self {
            Self::CfgEdge(guard) => Guard::CfgEdge {
                edge: wire::MixedEdgeV26 {
                    function: guard.edge.source.function.0,
                    block: guard.edge.source.block,
                    successor: guard.edge.successor,
                },
                condition: definition(guard.condition),
            },
            Self::ExplicitPredicate {
                condition,
                bound_comparison,
            } => Guard::ExplicitPredicate {
                condition: definition(condition),
                bound_comparison: definition(bound_comparison),
            },
        }
    }
}

#[cfg(test)]
mod typed_source_guard_v86_tests {
    use super::*;
    use fe2o3_kernel_descriptor::mixed_conditional_v26 as wire;
    use fe2o3_kernel_descriptor::mixed_conditional_v86::MixedAccessGuardV86 as Guard;
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    };

    #[test]
    fn typed_source_guard_v86_transports_all_definition_kinds_without_v26_admission() {
        let block = Block {
            function: Function(4),
            block: 8,
        };
        let coordinates = [
            (
                SliceDefinition::FunctionArgument {
                    function: Function(4),
                    argument: 2,
                },
                wire::MixedDefinitionV26::FunctionArgument {
                    function: 4,
                    argument: 2,
                },
            ),
            (
                SliceDefinition::BlockArgument { block, argument: 3 },
                wire::MixedDefinitionV26::BlockArgument {
                    function: 4,
                    block: 8,
                    argument: 3,
                },
            ),
            (
                SliceDefinition::Result {
                    operation: SliceOperation {
                        block,
                        operation: 7,
                    },
                    result: 1,
                },
                wire::MixedDefinitionV26::Result {
                    operation: wire::MixedOperationV26 {
                        function: 4,
                        block: 8,
                        operation: 7,
                    },
                    result: 1,
                },
            ),
        ];
        for (condition, expected) in coordinates {
            let edge = fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                source: block,
                successor: 2,
            };
            let cfg = GlobalSourceCfgGuardV85 { edge, condition };
            assert_eq!(
                GlobalSourceGuardV85::CfgEdge(cfg).wire_v86(),
                Guard::CfgEdge {
                    edge: wire::MixedEdgeV26 {
                        function: 4,
                        block: 8,
                        successor: 2
                    },
                    condition: expected,
                }
            );
            assert_eq!(
                GlobalSourceGuardV85::CfgEdge(cfg)
                    .require_cfg_v26()
                    .unwrap(),
                cfg
            );
            for (bound_comparison, bound) in coordinates {
                let explicit = GlobalSourceGuardV85::ExplicitPredicate {
                    condition,
                    bound_comparison,
                };
                assert_eq!(
                    explicit.wire_v86(),
                    Guard::ExplicitPredicate {
                        condition: expected,
                        bound_comparison: bound
                    }
                );
                assert_eq!(explicit.wire_v86().edge(), None);
                assert!(matches!(
                    explicit.require_cfg_v26(),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                ));
            }
        }
    }
}
