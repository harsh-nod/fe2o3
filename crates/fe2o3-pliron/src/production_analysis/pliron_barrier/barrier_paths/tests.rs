use super::*;
use dialect_gpu::{
    AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr,
    optimization_v1::{BranchOp as GpuBranchOp, ConstantOp, ReturnOp as GpuReturnOp},
    switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3, SwitchOpV3},
};
use dialect_kernel::{BranchArgsOp, ReturnOp};
use pliron::{
    attribute::AttrObj,
    basic_block::BasicBlock,
    builtin::{
        attributes::{IntegerAttr, OperandSegmentSizesAttr},
        op_interfaces::{ATTR_KEY_OPERAND_SEGMENT_SIZES, OneRegionInterface},
        ops::FuncOp,
        types::{FunctionType, IntegerType, Signedness},
    },
    context::Ptr,
    dialect::DialectName,
    op::Op,
    operation::verify_operation,
    parsable::{Parsable, parse_from_str},
    utils::apint::APInt,
};

fn barrier(context: &mut Context, block: Ptr<BasicBlock>) {
    BarrierOp::new(
        context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Workgroup,
        AddressSpaceAttr::Workgroup,
        MemoryOrderAttr::AcquireRelease,
    )
    .get_operation()
    .insert_at_back(block, context);
}

fn fixture(
    width: u32,
    signed: bool,
    kind: SwitchKeyKindAttrV3,
    keys: Vec<u64>,
    alternative: Option<(usize, bool, bool)>,
    cycle: bool,
) -> (Context, FuncOp, SwitchOpV3) {
    let mut context = Context::new();
    dialect_kernel::register_dialect(&mut context, &DialectName::try_new("kernel").unwrap())
        .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let ty = IntegerType::get(
        &context,
        width,
        if signed {
            Signedness::Signed
        } else {
            Signedness::Unsigned
        },
    )
    .into();
    let signature = FunctionType::get(&context, vec![ty; 3], vec![]);
    let function = FuncOp::new(
        &mut context,
        "barrier_switch".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(&context);
    let selector = entry.deref(&context).get_argument(0);
    let a = entry.deref(&context).get_argument(1);
    let b = entry.deref(&context).get_argument(2);
    let join = BasicBlock::new(&mut context, None, vec![ty; 2]);
    join.insert_at_back(function.get_region(&context), &context);
    barrier(&mut context, join);
    let end = if cycle {
        let x = join.deref(&context).get_argument(0);
        let y = join.deref(&context).get_argument(1);
        BranchArgsOp::new(&mut context, vec![x, y], join).get_operation()
    } else {
        ReturnOp::new(&mut context).get_operation()
    };
    end.insert_at_back(join, &context);
    let alternate = alternative.map(|(_, has_barrier, trap)| {
        let block = BasicBlock::new(&mut context, None, vec![ty; 2]);
        block.insert_at_back(function.get_region(&context), &context);
        if has_barrier {
            barrier(&mut context, block);
        }
        let end = if trap {
            TrapOp::new(&mut context).get_operation()
        } else {
            ReturnOp::new(&mut context).get_operation()
        };
        end.insert_at_back(block, &context);
        block
    });
    let edges = (0..=keys.len())
        .map(|ordinal| {
            let target = if alternative.is_some_and(|(selected, _, _)| selected == ordinal) {
                alternate.unwrap()
            } else {
                join
            };
            SwitchEdgeV3::new(
                target,
                if ordinal % 2 == 0 {
                    vec![a, b]
                } else {
                    vec![b, a]
                },
            )
        })
        .collect();
    let switch = SwitchOpV3::try_new(&mut context, selector, kind, keys, edges).unwrap();
    switch.get_operation().insert_at_back(entry, &context);
    verify_operation(function.get_operation(), &context).unwrap();
    (context, function, switch)
}

fn summary(context: &Context, function: &FuncOp) -> BarrierPathSummaryV1 {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    summarize_all_barrier_paths(context, &inventory, || {
        panic!("this fixture must not request a progress proof")
    })
    .unwrap()
}

#[test]
fn repeated_switch_slots_and_typed_payloads_reach_the_same_barrier() {
    use SwitchKeyKindAttrV3 as K;
    for (width, signed, kind, keys) in [
        (128, false, K::LegacyU64, vec![]),
        (128, true, K::EmptyTyped, vec![]),
        (
            64,
            true,
            K::I64,
            vec![1 << 63, u64::MAX, 0, i64::MAX as u64],
        ),
        (128, false, K::LegacyU64, (0..16).collect()),
        (128, false, K::LegacyU64, (0..17).collect()),
    ] {
        let count = keys.len() + 1;
        let (context, function, _) = fixture(width, signed, kind, keys, None, false);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let nodes = build_barrier_cfg_v1(&context, &inventory).ok().unwrap();
        assert_eq!(nodes[0].successors, vec![1; count]);
        assert!(matches!(
            summary(&context, &function),
            BarrierPathSummaryV1::Unique
        ));
        // The private path algorithm is not whole-pipeline native admission.
        assert!(matches!(
            crate::derive_pliron_ir_structural_identity_v1(&context, &function),
            Err(crate::PlironIrIdentityErrorV1::UnsupportedOperation { .. })
        ));
    }
}

#[test]
fn every_case_and_default_is_compared_even_with_a_constant_selector() {
    for ordinal in [0, 8, 17] {
        let (mut context, function, switch) = fixture(
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..17).collect(),
            Some((ordinal, false, false)),
            false,
        );
        let ty = IntegerType::get(&context, 128, Signedness::Unsigned);
        let constant = ConstantOp::new(
            &mut context,
            IntegerAttr::new(ty, APInt::zero(128.try_into().unwrap())).into(),
        );
        constant
            .get_operation()
            .insert_at_front(function.get_entry_block(&context), &context);
        Operation::replace_operand(
            switch.get_operation(),
            &context,
            0,
            constant.result(&context),
        );
        verify_operation(function.get_operation(), &context).unwrap();
        assert!(matches!(
            summary(&context, &function),
            BarrierPathSummaryV1::Divergent { .. }
        ));
    }
}

#[test]
fn switch_trap_prefix_and_collective_cycle_rules_are_unchanged() {
    for has_barrier in [false, true] {
        let (context, function, _) = fixture(
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            vec![0, 1],
            Some((2, has_barrier, true)),
            false,
        );
        let result = summary(&context, &function);
        if has_barrier {
            assert!(matches!(result, BarrierPathSummaryV1::Divergent { .. }));
        } else {
            assert!(matches!(result, BarrierPathSummaryV1::Unique));
        }
    }
    let (context, function, _) = fixture(
        128,
        false,
        SwitchKeyKindAttrV3::LegacyU64,
        vec![0, 1],
        None,
        true,
    );
    assert!(
        matches!(summary(&context, &function), BarrierPathSummaryV1::Incomplete(detail) if detail.contains("cyclic block"))
    );
}

#[test]
fn malformed_switch_metadata_and_foreign_edges_are_incomplete() {
    for malformed in 0..4 {
        let (mut context, function, switch) = fixture(
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            vec![0, 1],
            None,
            false,
        );
        match malformed {
            0 => {
                let attribute = parse_from_str(
                    AttrObj::parser(()),
                    &mut context,
                    "gpu.switch_successor_offsets_v3 [0, 2, 5, 6]",
                )
                .unwrap();
                switch
                    .get_operation()
                    .deref_mut(&context)
                    .attributes
                    .0
                    .insert("gpu_switch_offsets".try_into().unwrap(), attribute);
            }
            1 => switch.get_operation().deref_mut(&context).attributes.set(
                ATTR_KEY_OPERAND_SEGMENT_SIZES.clone(),
                OperandSegmentSizesAttr(vec![1, 5]),
            ),
            2 => {
                let ty = IntegerType::get(&context, 128, Signedness::Unsigned).into();
                let foreign = BasicBlock::new(&mut context, None, vec![ty; 2]);
                Operation::replace_successor(switch.get_operation(), &context, 2, foreign);
            }
            _ => {
                let wrong = BasicBlock::new(&mut context, None, vec![]);
                wrong.insert_at_back(function.get_region(&context), &context);
                ReturnOp::new(&mut context)
                    .get_operation()
                    .insert_at_back(wrong, &context);
                Operation::replace_successor(switch.get_operation(), &context, 2, wrong);
            }
        }
        assert!(matches!(
            summary(&context, &function),
            BarrierPathSummaryV1::Incomplete(_)
        ));
    }
}

#[test]
fn native_branch_and_return_use_the_same_closed_control_observer() {
    let (mut context, function, switch) = fixture(
        128,
        false,
        SwitchKeyKindAttrV3::LegacyU64,
        vec![],
        None,
        false,
    );
    let entry = function.get_entry_block(&context);
    let join = switch.get_operation().deref(&context).get_successor(0);
    let a = entry.deref(&context).get_argument(1);
    let b = entry.deref(&context).get_argument(2);
    let signature = FunctionType::get(&context, vec![], vec![]);
    let other = FuncOp::new(&mut context, "native_single".try_into().unwrap(), signature);
    let other_entry = other.get_entry_block(&context);
    let exit = BasicBlock::new(&mut context, None, vec![]);
    exit.insert_at_back(other.get_region(&context), &context);
    GpuBranchOp::new(&mut context, exit, vec![])
        .get_operation()
        .insert_at_back(other_entry, &context);
    barrier(&mut context, exit);
    GpuReturnOp::new(&mut context, vec![])
        .get_operation()
        .insert_at_back(exit, &context);
    verify_operation(other.get_operation(), &context).unwrap();
    assert!(matches!(
        summary(&context, &other),
        BarrierPathSummaryV1::Unique
    ));
    // A detached branch with two exact payloads is observed without value scans.
    let branch = GpuBranchOp::new(&mut context, join, vec![a, b]);
    assert_eq!(
        ControlViewV1::observe(&context, branch.get_operation())
            .unwrap()
            .successor_count(),
        1
    );
}

#[test]
fn actual_multiway_edge_count_has_exact_existing_resource_bounds() {
    use crate::production_analysis::{
        pliron_barrier::preflight_barrier_convergence_resource_upper_bound_v1,
        pliron_resource_envelope::{
            ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitsV1,
        },
    };
    for (cases, work, peak) in [(0, 16883, 19821), (16, 100595, 19997), (17, 105827, 20008)] {
        let (context, function, _) = fixture(
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..cases).collect(),
            None,
            false,
        );
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let successors = inventory
            .operations()
            .iter()
            .map(|site| site.pointer().deref(&context).get_num_successors())
            .sum();
        let census = ProductionAnalysisInputCensusV1 {
            blocks: inventory.blocks().len(),
            operations: inventory.operations().len(),
            successors,
            ..Default::default()
        };
        let limits = ProductionAnalysisResourceLimitsV1::new(work, peak);
        let bound =
            preflight_barrier_convergence_resource_upper_bound_v1(census, None, limits).unwrap();
        assert_eq!(bound.work_upper_bound(), work);
        assert_eq!(bound.peak_storage_upper_bound(), peak);
        assert_eq!(bound.retained_storage_upper_bound(), 5120);
        for limits in [
            ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
            ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
        ] {
            assert!(
                preflight_barrier_convergence_resource_upper_bound_v1(census, None, limits)
                    .is_err()
            );
        }
        assert!(matches!(
            summary(&context, &function),
            BarrierPathSummaryV1::Unique
        ));
    }
}
