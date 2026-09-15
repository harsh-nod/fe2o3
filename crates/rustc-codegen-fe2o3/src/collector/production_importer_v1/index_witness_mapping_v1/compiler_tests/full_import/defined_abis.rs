//! Check real body/source agreement without constructing a replacement body.

use super::*;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
) -> Vec<SemanticFunctionAbiV1> {
    assert!(plan.function_producers().len() <= 256);
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .expect("actual defined-function type producers")
        .into_records();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .expect("actual defined-function ABI producers")
    .into_records();
    assert_eq!(abis.len(), plan.function_producers().len());
    assert_eq!(abis.len(), plan.body_producers().len());
    let mut observed_unoptimized_hidden = false;
    for (index, ((function, body), abi)) in plan
        .function_producers()
        .iter()
        .zip(plan.body_producers())
        .zip(&abis)
        .enumerate()
    {
        let owner = SemanticFunctionIdV1::from_index(index as u32);
        assert_eq!(body.function, owner);
        let mir = plan.function_mir(owner).expect("exact retained body owner");
        let raw_arguments = if matches!(
            function
                .instance
                .ty(tcx, TypingEnv::fully_monomorphized())
                .kind(),
            TyKind::Closure(..)
        ) {
            assert_eq!(abi.extern_abi(), SemanticExternAbiV1::RustCall);
            assert!(mir.spread_arg.is_none());
            assert_eq!(abi.source_input_types().len(), 2);
            assert_eq!(abi.fixed_count(), 1);
            let tuple = abi.source_input_types()[1];
            let SemanticTypeShapeV1::Tuple(fields) = types[tuple.index() as usize].shape() else {
                panic!("defined closure {index} source argument is not a tuple");
            };
            let arguments = abi.adjusted_arguments();
            assert_eq!(arguments.len(), fields.fields().len() + 1);
            assert_eq!(arguments[0].role(), SemanticAbiArgumentRoleV1::Source);
            assert_eq!(arguments[0].ty(), abi.source_input_types()[0]);
            for (field, (argument, expected)) in
                arguments[1..].iter().zip(fields.fields()).enumerate()
            {
                assert_eq!(
                    argument.role(),
                    SemanticAbiArgumentRoleV1::RustCallTupleField(field as u32)
                );
                assert_eq!(argument.ty(), *expected);
            }
            arguments
                .iter()
                .map(SemanticAbiArgumentV1::ty)
                .collect::<Vec<_>>()
        } else {
            abi.source_input_types().to_vec()
        };
        assert_eq!(
            mir.arg_count,
            raw_arguments.len(),
            "defined function {index} {:?} source/body argument count; ABI {abi:?}",
            function.instance,
        );
        for (raw_local, expected) in std::iter::once(abi.source_output_type())
            .chain(raw_arguments)
            .enumerate()
        {
            let canonical = body.raw_to_semantic_locals[raw_local];
            let local = &body.locals[canonical.index() as usize];
            assert_eq!(local.rustc_local as usize, raw_local);
            assert_eq!(
                local.ty, expected,
                "defined function {index} {:?} raw local {raw_local} source/body type; ABI {abi:?}",
                function.instance,
            );
        }
        for hidden in abi.hidden_arguments() {
            assert_eq!(
                hidden.role(),
                SemanticAbiArgumentRoleV1::Hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation)
            );
            let ty = &types[hidden.value().source_ty().index() as usize];
            let info = ty
                .abi_properties()
                .first_pointee()
                .expect("retained hidden pointee facts");
            let SemanticAbiPassModeV1::Direct(attributes) = hidden.value().mode() else {
                panic!(
                    "defined function {index} {:?} hidden ABI {abi:?}",
                    function.instance
                );
            };
            assert_eq!(
                info.kind(),
                SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                "cached opt-level=0 caller-location profile for {:?}",
                function.instance,
            );
            assert_eq!(info.guaranteed_size_bytes(), 0);
            assert_eq!(info.reliable_alignment_bytes(), 8);
            assert_eq!(
                *attributes,
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    Some(8),
                )
                .unwrap(),
                "defined function {index} {:?} exact unoptimized hidden ABI",
                function.instance,
            );
            eprintln!(
                "defined function {index} {:?}: exact unoptimized caller-location ABI retained",
                function.instance
            );
            observed_unoptimized_hidden = true;
        }
    }
    assert!(
        observed_unoptimized_hidden,
        "registered source must exercise the real hidden-argument profile"
    );
    abis
}

pub(super) fn check_canonical_locals(mir: &AdmittedInertSemanticMirV1) {
    for (index, function) in mir.functions().iter().enumerate() {
        let abi = function.abi();
        let mut seen = vec![false; abi.source_input_types().len()];
        for local in function.locals() {
            match local.role() {
                SemanticLocalRoleV1::Argument(argument) => {
                    let argument = argument as usize;
                    assert!(!std::mem::replace(&mut seen[argument], true));
                    assert_eq!(
                        local.ty(),
                        abi.source_input_types()[argument],
                        "canonical function {index} source argument {argument}"
                    );
                }
                SemanticLocalRoleV1::Return => assert_eq!(local.ty(), abi.source_output_type()),
                SemanticLocalRoleV1::Temporary => {}
            }
        }
        assert!(
            seen.into_iter().all(|seen| seen),
            "canonical function {index} source arguments"
        );
    }
}
