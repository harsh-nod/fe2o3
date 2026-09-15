//! Real source terminal ABIs, independent of unrelated zero/SSA legalization.
use super::*;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use rustc_middle::ty::{Instance, Ty, TyKind};

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    phase_brand: bool,
) {
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .unwrap()
        .into_records();
    let producers = plan
        .terminal_producers()
        .iter()
        .map(|terminal| terminal.abi.clone())
        .collect::<Vec<_>>();
    let abis = construct_production_semantic_fn_abis_v1(tcx, &producers, plan.type_producers())
        .unwrap()
        .into_records();
    let mut found = BTreeSet::new();
    super::accumulator_zero::check(tcx, plan, &abis, &types);
    let mut checked_policy_receivers = 0;
    for (terminal, actual_abi) in plan.terminal_producers().iter().zip(&abis) {
        let expansion = terminal.expansion;
        if !abi::handles(expansion) {
            continue;
        }
        assert!(found.insert(expansion));
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(terminal.instance.def_id())
                .instantiate(tcx, terminal.instance.args),
        );
        let inputs = signature.inputs();
        let output = signature.output();
        assert!(abi::source_matches(tcx, expansion, inputs, output));
        let operation =
            abi::operation(tcx, terminal.instance, expansion, actual_abi, &types).unwrap();
        for index in 0..inputs.len() {
            let mut changed = inputs.to_vec();
            changed[index] = tcx.types.u32;
            assert!(
                !abi::source_matches(tcx, expansion, &changed, output),
                "input {index}"
            );
        }
        assert!(!abi::source_matches(tcx, expansion, inputs, tcx.types.u32));
        let mut arguments = terminal.instance.args.to_vec();
        let index = arguments
            .iter()
            .position(|arg| arg.as_type().is_some())
            .unwrap();
        arguments[index] = tcx.types.u16.into();
        let substituted = Instance {
            args: tcx.mk_args(&arguments),
            ..terminal.instance
        };
        assert!(abi::operation(tcx, substituted, expansion, actual_abi, &types).is_err());
        let wrong_ownership = actual_abi
            .clone()
            .with_source_argument_ownership(vec![
                SemanticSourceArgumentOwnershipV1::SharedBorrow;
                inputs.len()
            ])
            .unwrap();
        assert!(
            abi::operation(tcx, terminal.instance, expansion, &wrong_ownership, &types).is_err()
        );
        match operation {
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment,
                values,
            } => {
                assert_eq!(actual_abi.source_input_types(), [fragment]);
                assert_eq!(actual_abi.source_output_type(), values);
                assert_eq!(
                    types[fragment.index() as usize].identity(),
                    rustc_type_identity_v1(tcx, inputs[0])
                );
                let borrowed = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, inputs[0]);
                assert!(!abi::source_matches(tcx, expansion, &[borrowed], output));
                let other = if expansion == Expansion::Gfx950Fp4AccumulatorIntoValues {
                    Expansion::Gfx950Fp8AccumulatorIntoValues
                } else {
                    Expansion::Gfx950Fp4AccumulatorIntoValues
                };
                assert!(!abi::source_matches(tcx, other, inputs, output));
                // A valid same-layout Brand substitution is not the retained ABI.
                let TyKind::Adt(definition, args) = *inputs[0].kind() else {
                    panic!()
                };
                let mut args = args.to_vec();
                let brand = args
                    .iter()
                    .enumerate()
                    .filter(|(_, arg)| arg.as_type().is_some())
                    .nth(1)
                    .unwrap()
                    .0;
                args[brand] = tcx.types.u16.into();
                let changed = Ty::new_adt(tcx, definition, tcx.mk_args(&args));
                assert!(abi::source_matches(tcx, expansion, &[changed], output));
            }
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context,
                lhs_fragment,
                rhs_fragment,
                accumulator_fragment,
                lhs,
                rhs,
                accumulator,
            } => {
                assert_eq!(
                    actual_abi.source_input_types()[1..],
                    [lhs_fragment, rhs_fragment, accumulator_fragment]
                );
                assert_eq!(actual_abi.source_output_type(), accumulator_fragment);
                assert_eq!(
                    types[context.index() as usize].identity(),
                    rustc_type_identity_v1(tcx, rust_shared_reference_v1(inputs[0]).unwrap())
                );
                assert_eq!(lhs.role, SemanticMfmaOperandRoleV1::A);
                assert_eq!(rhs.role, SemanticMfmaOperandRoleV1::B);
                let (lhs_profile, rhs_profile) = match expansion {
                    Expansion::Gfx950Fp4MultiplyAccumulate => (
                        SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
                        SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
                    ),
                    Expansion::Gfx950Fp4Fp8MultiplyAccumulate => (
                        SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
                        SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
                    ),
                    Expansion::Gfx950Fp8MultiplyAccumulate => (
                        SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
                        SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
                    ),
                    _ => panic!("not a policy MFMA"),
                };
                assert_eq!(lhs.profile, lhs_profile);
                assert_eq!(rhs.profile, rhs_profile);
                assert_eq!(lhs.wave_width, 64);
                assert_eq!(rhs.wave_width, 64);
                assert_eq!(accumulator.wave_width, 64);
                assert_eq!(
                    lhs.register_distribution,
                    SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128
                );
                assert_eq!(
                    rhs.register_distribution,
                    SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128
                );
                assert_eq!(
                    accumulator.distribution,
                    SemanticMfmaAccumulatorDistributionV1::RowMajor
                );
                for other in [
                    Expansion::Gfx950Fp4MultiplyAccumulate,
                    Expansion::Gfx950Fp4Fp8MultiplyAccumulate,
                    Expansion::Gfx950Fp8MultiplyAccumulate,
                ] {
                    if other != expansion {
                        assert!(!abi::source_matches(tcx, other, inputs, output));
                        assert!(
                            abi::operation(tcx, terminal.instance, other, actual_abi, &types)
                                .is_err()
                        );
                    }
                }
                check_policy_receiver(tcx, expansion, inputs, output, phase_brand);
                checked_policy_receivers += 1;
                assert_eq!(accumulator.profile, lhs.profile);
                let mut changed = inputs.to_vec();
                changed.swap(1, 2);
                assert!(!abi::source_matches(tcx, expansion, &changed, output));
                for index in [1, 2, 3] {
                    let TyKind::Adt(definition, args) = *inputs[index].kind() else {
                        panic!()
                    };
                    let mut args = args.to_vec();
                    let brand = args
                        .iter()
                        .rposition(|arg| arg.as_type().is_some())
                        .unwrap();
                    args[brand] = tcx.types.u16.into();
                    let mut changed = inputs.to_vec();
                    changed[index] = Ty::new_adt(tcx, definition, tcx.mk_args(&args));
                    assert!(!abi::source_matches(tcx, expansion, &changed, output));
                }
            }
            _ => panic!("unexpected matrix terminal operation"),
        }
    }
    assert_eq!(
        found.len(),
        5,
        "both projections and all three actual policy MFMA formats"
    );
    assert_eq!(checked_policy_receivers, 3);
}

fn replace_type_argument<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    argument: usize,
    replacement: Ty<'tcx>,
) -> Ty<'tcx> {
    let TyKind::Adt(definition, args) = *ty.kind() else {
        panic!("actual reviewed ADT")
    };
    let mut args = args.to_vec();
    let index = args
        .iter()
        .enumerate()
        .filter(|(_, value)| value.as_type().is_some())
        .nth(argument)
        .unwrap()
        .0;
    args[index] = replacement.into();
    Ty::new_adt(tcx, definition, tcx.mk_args(&args))
}

fn check_policy_receiver<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
    phase_brand: bool,
) {
    let context = rust_shared_reference_v1(inputs[0]).unwrap();
    let fields = source::fields(tcx, context).unwrap();
    let bound = rust_shared_reference_v1(fields[0]).unwrap();
    let pair = source::pair(tcx, bound).unwrap();
    assert_eq!(
        pair.identity.execution_brand != pair.identity.root.ty,
        phase_brand,
        "the actual reusable phase E must not be flattened into policy root R"
    );
    let context_args = rust_trusted_adt_type_arguments_v1(
        tcx,
        context,
        TrustedDeviceItem::PolicyGfx950MatrixCapability,
    )
    .unwrap();
    assert_eq!(
        context_args,
        [
            pair.identity.matrix_brand,
            pair.identity.root.ty,
            pair.identity.policy,
        ]
    );

    // A policy-bearing layout or shared reference alone is not the actual receiver.
    let mut changed = inputs.to_vec();
    changed[0] = context;
    assert!(!abi::source_matches(tcx, expansion, &changed, output));
    changed[0] = Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, context);
    assert!(!abi::source_matches(tcx, expansion, &changed, output));
    changed[0] = pair.types[0]; // The real Matrix reference without its policy Bind.
    assert!(!abi::source_matches(tcx, expansion, &changed, output));

    for axis in 0..3 {
        let changed_context = replace_type_argument(tcx, context, axis, tcx.types.u16);
        changed[0] = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, changed_context);
        assert!(
            !abi::source_matches(tcx, expansion, &changed, output),
            "policy receiver axis {axis}"
        );
    }

    // Correlated M -> R substitution still fails the reviewed subgroup/epoch seal.
    let root = pair.identity.root.ty;
    let changed_context = replace_type_argument(tcx, context, 0, root);
    changed[0] = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, changed_context);
    changed[1] = replace_type_argument(tcx, inputs[1], 2, root);
    changed[2] = replace_type_argument(tcx, inputs[2], 2, root);
    changed[3] = replace_type_argument(tcx, inputs[3], 1, root);
    let changed_output = replace_type_argument(tcx, output, 1, root);
    assert!(!abi::source_matches(
        tcx,
        expansion,
        &changed,
        changed_output
    ));
}
