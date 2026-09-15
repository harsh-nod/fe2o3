//! Actual rustc ABI regressions run by both existing terminal-ABI callbacks.
use super::abi::accumulator_zero as zero;
use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
use rustc_middle::ty::{Instance, Ty, TyKind};

fn replace_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    ordinal: usize,
    replacement: Ty<'tcx>,
) -> Ty<'tcx> {
    let TyKind::Adt(definition, args) = *ty.kind() else {
        panic!("actual nominal ADT");
    };
    let mut args = args.to_vec();
    let slot = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_type().is_some())
        .nth(ordinal)
        .unwrap()
        .0;
    args[slot] = replacement.into();
    Ty::new_adt(tcx, definition, tcx.mk_args(&args))
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    abis: &[SemanticFunctionAbiV1],
    types: &[SemanticTypeDeclV1],
) {
    assert_eq!(plan.terminal_producers().len(), abis.len());
    let mut found = BTreeSet::new();
    for (terminal, actual_abi) in plan.terminal_producers().iter().zip(abis) {
        let expansion = terminal.expansion;
        if !matches!(
            expansion,
            Expansion::Gfx950Fp4AccumulatorZero | Expansion::Gfx950Fp8AccumulatorZero
        ) {
            continue;
        }
        assert!(found.insert(expansion));
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(terminal.instance.def_id())
                .instantiate(tcx, terminal.instance.args),
        );
        let inputs = signature.inputs();
        let output = signature.output();
        assert!(zero::source_matches(tcx, expansion, inputs, output));
        let lane_type = rust_shared_reference_v1(inputs[0]).unwrap();
        let operation =
            zero::operation(tcx, terminal.instance, expansion, actual_abi, types).unwrap();
        let SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
            lane,
            fragment,
            contract,
        } = operation
        else {
            panic!("typed zero operation");
        };
        assert_eq!(
            types[lane.index() as usize].identity(),
            rustc_type_identity_v1(tcx, lane_type)
        );
        assert_eq!(
            types[fragment.index() as usize].identity(),
            rustc_type_identity_v1(tcx, output)
        );
        assert_eq!(fragment, actual_abi.source_output_type());
        assert_eq!(actual_abi.source_input_types().len(), 1);
        assert_eq!(contract.wave_width, 64);
        assert_eq!(
            contract.distribution,
            SemanticMfmaAccumulatorDistributionV1::RowMajor
        );
        assert_eq!(
            contract.profile,
            if expansion == Expansion::Gfx950Fp4AccumulatorZero {
                SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128
            } else {
                SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128
            }
        );

        for changed in [
            tcx.types.u32,
            lane_type,
            Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, lane_type),
            Ty::new_imm_ptr(tcx, lane_type),
        ] {
            assert!(!zero::source_matches(tcx, expansion, &[changed], output));
        }
        assert!(!zero::source_matches(tcx, expansion, &[], output));
        assert!(!zero::source_matches(
            tcx,
            expansion,
            &[inputs[0], inputs[0]],
            output
        ));
        assert!(!zero::source_matches(tcx, expansion, inputs, tcx.types.u32));
        let other = if expansion == Expansion::Gfx950Fp4AccumulatorZero {
            Expansion::Gfx950Fp8AccumulatorZero
        } else {
            Expansion::Gfx950Fp4AccumulatorZero
        };
        assert!(!zero::source_matches(tcx, other, inputs, output));
        assert!(zero::operation(tcx, terminal.instance, other, actual_abi, types).is_err());
        assert!(
            zero::operation(
                tcx,
                terminal.instance,
                Expansion::Gfx950MatrixContextCurrent,
                actual_abi,
                types
            )
            .is_err()
        );

        for ordinal in [0, 1] {
            let changed_lane = replace_type(tcx, lane_type, ordinal, tcx.types.u16);
            let changed = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, changed_lane);
            assert!(
                !zero::source_matches(tcx, expansion, &[changed], output),
                "lane width/full brand axis {ordinal}"
            );
            assert!(
                !zero::source_matches(
                    tcx,
                    expansion,
                    inputs,
                    replace_type(tcx, output, ordinal, tcx.types.u16)
                ),
                "output format/full brand axis {ordinal}"
            );
        }
        let lane_arguments =
            rust_trusted_adt_type_arguments_v1(tcx, lane_type, TrustedDeviceItem::WaveLane)
                .unwrap();
        for axis in 0..3 {
            let changed_brand = replace_type(tcx, lane_arguments[1], axis, tcx.types.u16);
            let changed_lane = replace_type(tcx, lane_type, 1, changed_brand);
            let changed = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, changed_lane);
            assert!(
                !zero::source_matches(tcx, expansion, &[changed], output),
                "subgroup width/execution/epoch axis {axis}"
            );
        }
        // Correlated nominal lookalikes do not become a scoped subgroup brand.
        let changed_lane = replace_type(tcx, lane_type, 1, tcx.types.u16);
        let changed_output = replace_type(tcx, output, 1, tcx.types.u16);
        assert!(!zero::source_matches(
            tcx,
            expansion,
            &[Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, changed_lane)],
            changed_output
        ));

        let wrong_ownership = actual_abi
            .clone()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap();
        assert!(
            zero::operation(tcx, terminal.instance, expansion, &wrong_ownership, types).is_err()
        );
        let mut args = terminal.instance.args.to_vec();
        let index = args.iter().position(|arg| arg.as_type().is_some()).unwrap();
        args[index] = tcx.types.u16.into();
        let substituted = Instance {
            args: tcx.mk_args(&args),
            ..terminal.instance
        };
        assert!(zero::operation(tcx, substituted, expansion, actual_abi, types).is_err());
    }
    assert_eq!(
        found,
        BTreeSet::from([
            Expansion::Gfx950Fp4AccumulatorZero,
            Expansion::Gfx950Fp8AccumulatorZero
        ]),
        "both actual branded zero ABIs are required in root and reusable-phase fixtures"
    );
}
