use super::*;
use crate::kir_bridge_v1::NativeCfgDomainV26;

fn disconnected(mode: usize) -> Module {
    let mut module = if mode == 1 { trap_fixture() } else { fixture() };
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    if mode == 1 {
        let mut terminal = blocks[2].clone();
        terminal.id = BlockId(40);
        blocks.push(terminal);
        return module;
    }
    let mut first = Block::new(BlockId(40));
    first.terminator = Some(match mode {
        0 => Terminator::Return { values: vec![] },
        2 => Terminator::Branch {
            target: BlockId(50),
            arguments: vec![],
        },
        3 => Terminator::Switch {
            selector: ValueId(1),
            cases: vec![fe2o3_kernel_ir::SwitchCase {
                value: 0,
                target: BlockId(40),
                arguments: vec![],
            }],
            default_target: BlockId(50),
            default_arguments: vec![],
        },
        4 => Terminator::IntegerSwitch {
            selector: ValueId(1),
            cases: vec![fe2o3_kernel_ir::IntegerSwitchCase {
                value: fe2o3_kernel_ir::Constant::U32(0),
                target: BlockId(40),
                arguments: vec![],
            }],
            default_target: BlockId(50),
            default_arguments: vec![],
        },
        _ => unreachable!(),
    });
    blocks.push(first);
    if mode >= 2 {
        let mut second = Block::new(BlockId(50));
        second.terminator = Some(Terminator::Branch {
            target: BlockId(40),
            arguments: vec![],
        });
        blocks.push(second);
    }
    module
}

fn complete(input: &NativeCanonicalMixedAdmissionV26<'_>) -> Result<(), Failure> {
    assert_eq!(
        input.cfg_domain_v26(),
        NativeCfgDomainV26::EntryReachableSubgraphV26
    );
    assert_eq!(
        input.private.cfg_domain_v26(),
        NativeCfgDomainV26::AllBlocksReachableV1
    );
    let outcome = crate::canonical_private_v1::run_mixed_v26(
        input,
        crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        None,
    )
    .unwrap();
    assert!(outcome.report.reports().is_clean());
    assert_eq!(outcome.report.paired_stage_count(), 9);
    for position in 0..9 {
        assert_eq!(outcome.report.cfg_domain_version(position), Some(26));
        assert_eq!(outcome.report.global_access_counts(position), Some([1, 1]));
    }
    assert_eq!(outcome.report.cfg_domain_version(9), None);
    assert!(!outcome.report.grants_artifact_or_launch_authority());
    assert!(!input.globals.runtime_requirements_are_discharged());
    Ok(())
}

#[test]
fn mixed_cfg_v26_retains_disconnected_terminals_and_closed_cycles_with_exact_entry_mapping() {
    for mode in 0..5 {
        let module = disconnected(mode);
        // Entry is physical ordinal zero, not a numeric BlockId zero. The
        // preserved dead regions use both successor occurrences of switches.
        assert_eq!(
            module.functions[0].body.as_ref().unwrap().blocks[0].id,
            BlockId(10)
        );
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, 0, complete);
        assert!(result.is_ok(), "mode={mode}: {result:?}");
        assert_eq!(calls, 1);
    }
}

#[test]
fn mixed_cfg_v26_rejects_changed_live_edges_foreign_owner_and_restored_epoch() {
    let module = disconnected(2);
    for fault in [5, 7, 16] {
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, fault, |_| {
            panic!("substituted graph/domain reached the native consumer");
        });
        assert!(result.is_err(), "fault={fault}");
        assert_eq!(calls, 0);
    }
}

#[test]
fn mixed_cfg_v26_does_not_treat_constant_branches_as_structurally_disconnected() {
    for constant in [false, true] {
        let mut module = fixture();
        let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        entry.operations.push(value(
            12,
            Type::BOOL,
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(constant)),
        ));
        let Some(Terminator::ConditionalBranch { condition, .. }) = &mut entry.terminator else {
            unreachable!()
        };
        *condition = ValueId(12);
        // The Global memory block no longer has its actual index<extent guard.
        // A constant-false arm is still a structural edge, not a CFG-domain
        // bypass, and complete Global admission must refuse this unsafe roster.
        let (result, _, _, calls) = run_module_case(&module, AMPLE, AMPLE, 0, |_| {
            panic!("unguarded global accesses acquired native admission");
        });
        assert!(result.is_err(), "constant={constant}");
        assert_eq!(calls, 0);
    }
}

#[test]
fn mixed_cfg_v26_disconnected_graph_has_exact_and_one_short_transaction_limits() {
    let module = disconnected(4);
    let (result, work, peak, calls) = run_module_case(&module, AMPLE, AMPLE, 0, complete);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(calls, 1);
    let (result, exact_work, exact_peak, calls) = run_module_case(&module, work, peak, 0, complete);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!((exact_work, exact_peak, calls), (work, peak, 1));
    for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
        assert!(
            run_module_case(&module, work_limit, storage_limit, 0, |_| Ok(()))
                .0
                .is_err()
        );
    }
}
