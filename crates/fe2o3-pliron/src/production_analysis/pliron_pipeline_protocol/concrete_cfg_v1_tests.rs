#[cfg(test)]
mod concrete_cfg_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_v1;
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceContractV1;
    use dialect_kernel::{
        BranchOp, DIALECT_NAME, IndexType, MemorySpaceAttr, RankedViewType, ReturnOp, TrapOp,
        register_dialect,
    };
    use pliron::{
        builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
        dialect::DialectName,
        op::Op,
        r#type::TypeHandle,
    };

    fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
        operation.get_operation().insert_at_back(block, context);
    }

    #[derive(Clone, Copy)]
    enum Shape {
        Chain,
        Permuted,
        RepeatedEdge,
        TrapBefore,
        TrapAfterStage,
        TrapAfterRelease,
        Bypass,
        Cycle,
        UnreachableAccess,
        ArgumentEpoch,
        SameBlockCycle,
    }

    fn fixture(shape: Shape, pipelines: usize) -> (Context, FuncOp) {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let index: TypeHandle = IndexType::get(&context).into();
        let signature = FunctionType::get(&context, vec![index], vec![]);
        let function = FuncOp::new(&mut context, "concrete_cfg".try_into().unwrap(), signature);
        let entry = function.get_entry_block(&context);
        let mut blocks = vec![entry];
        for number in 1..5 {
            let arguments =
                if number == 1 && matches!(shape, Shape::RepeatedEdge | Shape::ArgumentEpoch) {
                    vec![index]
                } else {
                    vec![]
                };
            blocks.push(BasicBlock::new(
                &mut context,
                Some(format!("b{number}").try_into().unwrap()),
                arguments,
            ));
        }
        let physical_order = if matches!(shape, Shape::Permuted) {
            [2, 1, 3, 4]
        } else {
            [1, 2, 3, 4]
        };
        for number in physical_order {
            blocks[number].insert_at_back(function.get_region(&context), &context);
        }
        let zero = IndexConstantOp::new(&mut context, 0);
        let zero_value = zero.result(&context);
        append(&context, entry, &zero);
        let one = IndexConstantOp::new(&mut context, 1);
        let one_value = one.result(&context);
        append(&context, entry, &one);
        let selector = entry.deref(&context).get_argument(0);
        for owner in 0..pipelines {
            let ty = RankedViewType::new(&context, 32, true, vec![2, 1]).unwrap();
            let view = RankedViewOp::new_in_space_with_allocation_contract(
                &mut context,
                ty,
                vec![],
                MemorySpaceAttr::Workgroup,
                owner as u64 + 1,
                owner as u64 + 1,
            )
            .unwrap();
            let view_value = view.result(&context);
            append(&context, entry, &view);
            let create = PipelineCreateOp::new(&mut context, view_value, 2, 1).unwrap();
            let pipeline = create.pipeline(&context);
            append(&context, entry, &create);
            let (stage_block, tail_block) = if matches!(shape, Shape::SameBlockCycle) {
                (entry, entry)
            } else {
                (blocks[1], blocks[2])
            };
            let stage_epoch = if matches!(shape, Shape::ArgumentEpoch) {
                blocks[1].deref(&context).get_argument(0)
            } else {
                zero_value
            };
            let stage = PipelineEventOp::new(
                &mut context,
                pipeline,
                stage_epoch,
                zero_value,
                PipelineEventKindAttr::Stage,
            )
            .unwrap();
            append(&context, stage_block, &stage);
            let write = RankedAccessOp::new(
                &mut context,
                AccessKindAttr::Write,
                view_value,
                vec![zero_value, zero_value],
            )
            .unwrap();
            append(&context, stage_block, &write);
            for kind in [
                PipelineEventKindAttr::Commit,
                PipelineEventKindAttr::Wait,
                PipelineEventKindAttr::Consume,
            ] {
                let event =
                    PipelineEventOp::new(&mut context, pipeline, zero_value, zero_value, kind)
                        .unwrap();
                append(&context, tail_block, &event);
            }
            let read = RankedAccessOp::new(
                &mut context,
                AccessKindAttr::Read,
                view_value,
                vec![zero_value, zero_value],
            )
            .unwrap();
            append(&context, tail_block, &read);
            let release = PipelineEventOp::new(
                &mut context,
                pipeline,
                zero_value,
                zero_value,
                PipelineEventKindAttr::Release,
            )
            .unwrap();
            append(&context, tail_block, &release);
            if matches!(shape, Shape::UnreachableAccess) {
                let access = RankedAccessOp::new(
                    &mut context,
                    AccessKindAttr::Read,
                    view_value,
                    vec![zero_value, zero_value],
                )
                .unwrap();
                append(&context, blocks[4], &access);
            }
        }
        let entry_end = match shape {
            Shape::SameBlockCycle => {
                BranchArgsOp::new(&mut context, vec![selector], entry).get_operation()
            }
            Shape::TrapBefore | Shape::Bypass => IndexEqualBranchArgsOp::new(
                &mut context,
                selector,
                zero_value,
                vec![],
                vec![],
                blocks[1],
                if matches!(shape, Shape::TrapBefore) {
                    blocks[4]
                } else {
                    blocks[3]
                },
            )
            .get_operation(),
            Shape::RepeatedEdge => IndexEqualBranchArgsOp::new(
                &mut context,
                selector,
                zero_value,
                vec![zero_value],
                vec![one_value],
                blocks[1],
                blocks[1],
            )
            .get_operation(),
            Shape::ArgumentEpoch => {
                BranchArgsOp::new(&mut context, vec![zero_value], blocks[1]).get_operation()
            }
            _ => BranchOp::new(&mut context, blocks[1]).get_operation(),
        };
        entry_end.insert_at_back(entry, &context);
        let stage_end = if matches!(shape, Shape::TrapAfterStage) {
            IndexEqualBranchArgsOp::new(
                &mut context,
                selector,
                zero_value,
                vec![],
                vec![],
                blocks[2],
                blocks[4],
            )
            .get_operation()
        } else {
            BranchOp::new(&mut context, blocks[2]).get_operation()
        };
        stage_end.insert_at_back(blocks[1], &context);
        let tail_end = if matches!(shape, Shape::TrapAfterRelease) {
            IndexEqualBranchArgsOp::new(
                &mut context,
                selector,
                zero_value,
                vec![],
                vec![],
                blocks[3],
                blocks[4],
            )
            .get_operation()
        } else {
            BranchOp::new(&mut context, blocks[3]).get_operation()
        };
        tail_end.insert_at_back(blocks[2], &context);
        let exit = if matches!(shape, Shape::Cycle) {
            BranchOp::new(&mut context, blocks[1]).get_operation()
        } else {
            ReturnOp::new(&mut context).get_operation()
        };
        exit.insert_at_back(blocks[3], &context);
        TrapOp::new(&mut context)
            .get_operation()
            .insert_at_back(blocks[4], &context);
        (context, function)
    }

    fn reject_detail(shape: Shape, expected: &str) {
        let (context, function) = fixture(shape, 1);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(report.findings().iter().any(|finding| matches!(finding,
            PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. } if detail.contains(expected)
        )), "{report:?}");
        assert!(report.certificates().is_empty());
    }

    #[test]
    fn actual_cross_block_access_interleaving_has_static_certificates() {
        for shape in [
            Shape::Chain,
            Shape::Permuted,
            Shape::RepeatedEdge,
            Shape::TrapBefore,
        ] {
            let (context, function) = fixture(shape, 2);
            let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
            assert!(report.is_clean(), "{report:?}");
            assert_eq!(report.certificates().len(), 2);
            for certificate in report.certificates() {
                assert!(certificate.dynamic_loop().is_none());
                assert_eq!(certificate.concrete_epochs(), 1);
                assert_eq!(certificate.staged_writes(), 1);
                assert_eq!(certificate.consuming_reads(), 1);
                assert!(certificate.access_refinement_proven());
            }
        }
    }

    #[test]
    fn partial_and_drained_trap_paths_do_not_discharge_a_lifecycle() {
        reject_detail(
            Shape::TrapAfterStage,
            "trap follows an event or matching access",
        );
        reject_detail(
            Shape::TrapAfterRelease,
            "trap follows an event or matching access",
        );
    }

    #[test]
    fn static_trace_rejects_bypass_cycles_and_uncovered_accesses() {
        reject_detail(Shape::Bypass, "lifecycle");
        reject_detail(Shape::Cycle, "reachable cycle");
        reject_detail(Shape::UnreachableAccess, "does not dominate");
    }

    #[test]
    fn sparse_facts_prove_an_epoch_argument_grounded_by_its_actual_edge_payload() {
        // The predecessor passes literal zero, not its unknown entry selector.
        // CFG ordinals alone are insufficient; the separately admitted sparse
        // cache now proves the actual incoming value at this exact argument.
        let (context, function) = fixture(Shape::ArgumentEpoch, 1);
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        assert!(!analyses.sparse_indices_prepared());
        let report =
            run_pliron_pipeline_protocol_check_with_analyses_v1(&context, &function, &mut analyses);
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(report.certificates().len(), 1);
        assert_eq!(report.certificates()[0].concrete_epochs(), 1);
        assert!(report.certificates()[0].dynamic_loop().is_none());
        assert!(analyses.sparse_indices_prepared());
        assert!(analyses.sparse_indices().is_ok());
    }

    #[test]
    fn original_same_block_route_does_not_gain_the_cross_block_dag_restriction() {
        let (context, function) = fixture(Shape::SameBlockCycle, 1);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(report.certificates().len(), 1);
        assert!(report.certificates()[0].dynamic_loop().is_none());
    }

    #[test]
    fn shared_cfg_retains_duplicate_occurrences_and_checks_cached_rosters() {
        let (mut context, function) = fixture(Shape::RepeatedEdge, 1);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let census = pipeline_protocol_inventory_census_v1(&context, &inventory).unwrap();
        assert_eq!(census.blocks, 5);
        assert_eq!(census.successors, 4);
        let authenticated = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
        )
        .unwrap()
        .input_census_v1();
        assert_eq!(census.successors, authenticated.successors);
        let mut equivalence = EquivalenceResourceMeterV1::new(128, 128).unwrap();
        let discovery = discover_epoch_loops(&context, &inventory, &mut equivalence);
        assert_eq!(
            discovery.cfg_successors,
            [vec![1, 1], vec![2], vec![3], vec![], vec![]]
        );
        let cfg = ConcreteCfgV1::observe(&context, &inventory, &discovery.cfg_successors).unwrap();
        assert_eq!(cfg.order, [0, 1, 2, 3]);
        assert_eq!(cfg.reachable, [true, true, true, true, false]);
        assert_eq!(cfg.exits[3], ConcreteExitV1::Return);
        assert_eq!(cfg.exits[4], ConcreteExitV1::Trap);
        let mut missing = discovery.cfg_successors.clone();
        missing[0].pop();
        assert!(matches!(
            ConcreteCfgV1::observe(&context, &inventory, &missing),
            Err("cross-block concrete pipeline has an incomplete successor roster")
        ));
        let mut foreign = discovery.cfg_successors.clone();
        foreign[0][1] = inventory.blocks().len();
        assert!(matches!(
            ConcreteCfgV1::observe(&context, &inventory, &foreign),
            Err("cross-block concrete pipeline has a foreign or reordered successor")
        ));
        let signature = FunctionType::get(&context, vec![], vec![]);
        let other_function =
            FuncOp::new(&mut context, "foreign_cfg".try_into().unwrap(), signature);
        let foreign_entry = other_function.get_entry_block(&context);
        let foreign_zero = IndexConstantOp::new(&mut context, 0);
        append(&context, foreign_entry, &foreign_zero);
        let return_op = ReturnOp::new(&mut context);
        append(&context, foreign_entry, &return_op);
        let other = BoundedPlironFunctionInventoryV1::collect(&context, &other_function).unwrap();
        assert_eq!(
            other.operations()[0].block(),
            inventory.operations()[0].block()
        );
        assert_eq!(
            other.operations()[0].operation(),
            inventory.operations()[0].operation()
        );
        assert!(cfg.site_index(&inventory, other.operations()[0]).is_err());
    }

    #[test]
    fn missing_control_is_not_treated_as_a_normal_return() {
        let mut context = Context::new();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "missing_control".try_into().unwrap(),
            signature,
        );
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        assert!(matches!(
            ConcreteCfgV1::observe(&context, &inventory, &[vec![]]),
            Err("cross-block concrete pipeline has a block without a terminator")
        ));
    }

    #[test]
    fn duplicate_exact_sites_cannot_become_an_order_certificate() {
        let (context, function) = fixture(Shape::Chain, 1);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let mut equivalence = EquivalenceResourceMeterV1::new(128, 128).unwrap();
        let discovery = discover_epoch_loops(&context, &inventory, &mut equivalence);
        let mut control = PipelineControlContextV1 {
            inventory: &inventory,
            discovery: &discovery,
            concrete: None,
        };
        let pipeline = inventory
            .operations()
            .iter()
            .copied()
            .find(|site| Operation::get_op::<PipelineCreateOp>(site.pointer(), &context).is_some())
            .unwrap();
        let mut schedule = inventory
            .operations()
            .iter()
            .filter_map(|site| {
                let event = Operation::get_op::<PipelineEventOp>(site.pointer(), &context)?;
                Some(EventSiteV1 {
                    site: *site,
                    kind: event.kind(&context),
                    epoch: event.epoch(&context),
                    slot: event.slot(&context),
                })
            })
            .collect::<Vec<_>>();
        schedule.push(schedule[0]);
        let result =
            verify_cross_block_concrete_trace_v1(&context, &mut control, pipeline, &schedule, &[]);
        assert!(
            matches!(result, Err(PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. })
            if detail.contains("one physical occurrence twice"))
        );
    }

    fn resource_census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            blocks: 3,
            successors: 4,
            operations: 12,
            operands: 14,
            results: 3,
            max_operation_arity: 4,
            pipeline_creates: 2,
            pipeline_events: 4,
            ranked_accesses: 2,
            ..ProductionAnalysisInputCensusV1::default()
        }
    }

    #[test]
    fn independent_concrete_resource_census_and_representation_units() {
        // R=7, shared=32*8=256, two per-create walks=2*32*27=1728.
        // Scratch=17*3+4+12+2*7+48=129 abstract scalar slots.
        assert_eq!(
            pipeline_concrete_cfg_resource_delta_v1(resource_census()).unwrap(),
            (1_984, 129)
        );
        assert_eq!(
            std::mem::size_of::<ConcreteActionV1<'_>>(),
            2 * std::mem::size_of::<usize>()
        );
        let without_pipelines = ProductionAnalysisInputCensusV1 {
            pipeline_creates: 0,
            ..resource_census()
        };
        assert_eq!(
            pipeline_concrete_cfg_resource_delta_v1(without_pipelines).unwrap(),
            (0, 0)
        );
        let overflow = ProductionAnalysisInputCensusV1 {
            successors: usize::MAX,
            ..resource_census()
        };
        assert_eq!(
            pipeline_concrete_cfg_resource_delta_v1(overflow),
            Err(pipeline_resource_overflow_v1())
        );
    }

    #[test]
    fn actual_two_pipeline_duplicate_edge_census_has_independent_budget_boundaries() {
        let (context, function) = fixture(Shape::RepeatedEdge, 2);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let actual = pipeline_protocol_inventory_census_v1(&context, &inventory).unwrap();
        // Five blocks: entry7 + stage5 + tail11 + return1 + trap1 = 25 ops.
        // The duplicate entry edges plus two branches are four occurrences.
        // Operands: creates2 + branch4 + events30 + accesses12 = 48.
        assert_eq!(
            (actual.blocks, actual.operations, actual.successors),
            (5, 25, 4)
        );
        assert_eq!(
            (actual.operands, actual.results, actual.block_arguments),
            (48, 6, 2)
        );
        assert_eq!(
            (
                actual.pipeline_creates,
                actual.pipeline_events,
                actual.ranked_accesses
            ),
            (2, 10, 4)
        );
        assert_eq!(actual.max_operation_arity, 4);
        assert_eq!(actual.index_lt_branch_candidates, 0);
        // R=15. New work=32*10 + 2*32*50=3520, scratch=192.
        assert_eq!(
            pipeline_concrete_cfg_resource_delta_v1(actual).unwrap(),
            (3_520, 192)
        );
        // Q=352/U=64, old W=2750, retained=28434, temporary=5756.
        // Concrete fact queries add 2*(1+(2*10+4)*12)=578.
        // New phase W=6848 and peak=28434+5756+192=34382.
        let phase = ProductionAnalysisResourcePhaseV1::PipelineProtocol;
        let prefix =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 7, 11, 13).unwrap();
        for (work, peak, accepted) in [
            (6_855, 34_393, true),
            (6_854, 34_393, false),
            (6_855, 34_392, false),
        ] {
            let mut contract = ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            );
            contract.admit_retained(phase, prefix).unwrap();
            let before = contract.cumulative();
            let result = preflight_pipeline_protocol_resource_upper_bound_v1(
                actual,
                contract.remaining(phase).unwrap(),
            );
            if accepted {
                let bound = result.unwrap();
                assert_eq!(bound.work_upper_bound(), 6_848);
                assert_eq!(bound.retained_storage_upper_bound(), 28_434);
                assert_eq!(bound.peak_storage_upper_bound(), 34_382);
                contract.admit_retained(phase, bound).unwrap();
                assert_eq!(contract.cumulative().work_upper_bound(), 6_855);
                assert_eq!(contract.cumulative().retained_storage_upper_bound(), 28_445);
                assert_eq!(contract.cumulative().peak_storage_upper_bound(), 34_393);
            } else {
                assert!(result.is_err());
                assert_eq!(contract.cumulative(), before);
            }
        }
    }

    #[test]
    fn independent_phase_limits_preserve_a_nonzero_cumulative_prefix() {
        let phase = ProductionAnalysisResourcePhaseV1::PipelineProtocol;
        let prefix =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 7, 11, 13).unwrap();
        // Existing formula: Q=108, U=9, W=856, retained=14150,
        // temporary=4824. Add concrete (1984,129) and query work242:
        // W3082/peak19103.
        for (work, peak, accepted) in [
            (3_089, 19_114, true),
            (3_088, 19_114, false),
            (3_089, 19_113, false),
        ] {
            let mut contract = ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            );
            contract.admit_retained(phase, prefix).unwrap();
            let before = contract.cumulative();
            let phase_bound = preflight_pipeline_protocol_resource_upper_bound_v1(
                resource_census(),
                contract.remaining(phase).unwrap(),
            );
            if accepted {
                let bound = phase_bound.unwrap();
                assert_eq!(bound.work_upper_bound(), 3_082);
                assert_eq!(bound.retained_storage_upper_bound(), 14_150);
                assert_eq!(bound.peak_storage_upper_bound(), 19_103);
                contract.admit_retained(phase, bound).unwrap();
                assert_eq!(contract.cumulative().work_upper_bound(), 3_089);
                assert_eq!(contract.cumulative().retained_storage_upper_bound(), 14_161);
                assert_eq!(contract.cumulative().peak_storage_upper_bound(), 19_114);
            } else {
                assert!(phase_bound.is_err());
                assert_eq!(contract.cumulative(), before);
            }
        }
    }
}
