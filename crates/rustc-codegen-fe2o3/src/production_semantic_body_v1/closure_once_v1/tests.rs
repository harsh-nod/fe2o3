use super::*;
use crate::collector::closure_once_shim_v1::tests::{entry, first_callee, run, shim};
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_block_identity_v1, rustc_local_identity_v1,
    rustc_mir_body_sha256_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAbiIdentityV1, SemanticAbiPassModeV1, SemanticAbiValueV1,
    SemanticCanonAbiV1, SemanticExternAbiV1, SemanticLayoutIdentityV1,
};

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Receiver,
    CalleeBinding,
    LocalBudget,
    MissingSharedType,
}

// These are producer tests, not final target-ABI or GPU qualification evidence.
fn construct<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    mutation: Mutation,
) -> Result<(SemanticFunctionDeclV1, SemanticFunctionAbiV1), ProductionSemanticBodyErrorV1> {
    let source = SemanticSourceProvenanceV1::unavailable();
    let identities = canonical_function_identities_v1(tcx, instance);
    let body_sha = rustc_mir_body_sha256_v1(tcx, instance);
    let mut body = tcx.instance_mir(instance.def).clone();
    let environment = instance.args[0].expect_ty();
    let shared = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, environment);
    let mut types = Vec::new();
    for ty in body
        .local_decls
        .iter()
        .map(|decl| decl.ty)
        .chain([environment, shared])
    {
        let ty = normalize_type_v1(tcx, instance, ty).unwrap();
        if !types.contains(&ty) {
            types.push(ty);
        }
    }
    let type_id = |ty| {
        SemanticTypeIdV1::from_index(types.iter().position(|entry| *entry == ty).unwrap() as u32)
    };
    let scalar = type_id(tcx.types.u32);
    let tuple = type_id(instance.args[1].expect_ty());
    let abi_value = |ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([101; 32]),
        SemanticLayoutIdentityV1::from_sha256([102; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![type_id(environment), tuple],
        scalar,
        vec![
            SemanticAbiArgumentV1::source(abi_value(type_id(environment))),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, abi_value(scalar)),
        ],
        abi_value(scalar),
    )?;
    let type_bindings = types
        .iter()
        .enumerate()
        .filter(|(_, ty)| !matches!(mutation, Mutation::MissingSharedType) || **ty != shared)
        .map(|(index, ty)| {
            ProductionSemanticTypeBindingV1::new(*ty, SemanticTypeIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    let mut local_bindings = (0..body.local_decls.len())
        .map(|raw| {
            ProductionSemanticLocalBindingV1::new(
                raw as u32,
                SemanticLocalIdV1::from_index(0),
                rustc_local_identity_v1(identities.function(), body_sha, raw as u32),
                source,
            )
        })
        .collect::<Vec<_>>();
    local_bindings.sort_by_key(|local| local.identity);
    for (index, local) in local_bindings.iter_mut().enumerate() {
        local.semantic_local = SemanticLocalIdV1::from_index(index as u32);
    }
    let mut block_bindings = body
        .basic_blocks
        .iter_enumerated()
        .map(|(raw, data)| {
            ProductionSemanticBlockBindingV1::new(
                raw.as_usize() as u32,
                SemanticBlockIdV1::from_index(0),
                rustc_block_identity_v1(identities.function(), body_sha, raw.as_usize() as u32),
                source,
                vec![source; data.statements.len()],
                source,
            )
        })
        .collect::<Vec<_>>();
    block_bindings.sort_by_key(|block| block.identity);
    for (index, block) in block_bindings.iter_mut().enumerate() {
        block.semantic_block = SemanticBlockIdV1::from_index(index as u32);
    }
    let entry_block = block_bindings
        .iter()
        .find(|block| block.rustc_block == 0)
        .unwrap()
        .semantic_block;
    let callee = first_callee(tcx, instance);
    let expected = if matches!(mutation, Mutation::CalleeBinding) {
        entry(tcx, "fake_call_once")
    } else {
        callee
    };
    let function = SemanticFunctionIdV1::from_index(0);
    let direct_calls = [ProductionSemanticDirectCallBindingV1::new(
        function, 0, expected,
    )];
    let callables = [
        ProductionSemanticCallableOwnerEntryV1::defined(
            instance,
            SemanticCallableIdV1::from_index(0),
        ),
        ProductionSemanticCallableOwnerEntryV1::defined(
            callee,
            SemanticCallableIdV1::from_index(1),
        ),
    ];
    if matches!(mutation, Mutation::Receiver) {
        let TerminatorKind::Call { args, .. } =
            &mut body.basic_blocks_mut()[START_BLOCK].terminator_mut().kind
        else {
            unreachable!()
        };
        args[0].node = Operand::Move(Place::from(rustc_middle::mir::Local::from_usize(1)));
    }
    let mut limits = SemanticMirLimitsV1::default();
    if matches!(mutation, Mutation::LocalBudget) {
        limits = limits
            .with_limit(SemanticMirResourceV1::Locals, body.local_decls.len() as u64)
            .unwrap();
    }
    let mut owner = ProductionSemanticBodyRequestOwnerV1::new(limits, types.len(), &callables)?;
    let function = construct_production_semantic_body_v1(
        ProductionSemanticBodyInputV1 {
            tcx,
            instance,
            body: &body,
            function,
            identities: ProductionSemanticFunctionIdentitiesV1::new(
                identities.function(),
                identities.item_definition(),
                identities.monomorphization(),
                identities.generic_type_arguments(),
                identities.const_generic_arguments(),
            ),
            role: SemanticFunctionRoleV1::InternalHelper,
            export: ProductionSemanticFunctionExportV1::None,
            source,
            abi: abi.clone(),
            type_bindings: &type_bindings,
            local_bindings: &local_bindings,
            block_bindings: &block_bindings,
            entry: entry_block,
            direct_calls: &direct_calls,
            terminal_expansions: &[],
            normalized_intrinsics: &[],
        },
        &mut owner,
    )?;
    Ok((function, abi))
}

#[test]
fn closure_once_shim_semantic_producer_preserves_mutable_borrow_and_adds_shared_reborrow() {
    run(
        |tcx| {
            let instance = shim(tcx, "entry");
            let before = rustc_mir_body_sha256_v1(tcx, instance);
            let (function, abi) = construct(tcx, instance, Mutation::None).unwrap();
            assert_eq!(function.abi(), &abi);
            assert_eq!(rustc_mir_body_sha256_v1(tcx, instance), before);
            assert_eq!(function.locals().len(), 5);
            assert!(
                function
                    .locals()
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            let block = &function.blocks()[function.entry().index() as usize];
            assert_eq!(block.statements().len(), 2);
            let SemanticStatementKindV1::Assign(original) = block.statements()[0].kind() else {
                panic!("original receiver assignment")
            };
            let SemanticRvalueKindV1::Borrow { kind, .. } = original.value().kind() else {
                panic!("original borrow")
            };
            assert_ne!(*kind, SemanticBorrowKindV1::Shared);
            let SemanticStatementKindV1::Assign(reborrow) = block.statements()[1].kind() else {
                panic!("shared receiver assignment")
            };
            let SemanticRvalueKindV1::Borrow { kind, place } = reborrow.value().kind() else {
                panic!("shared reborrow")
            };
            assert_eq!(*kind, SemanticBorrowKindV1::Shared);
            assert_eq!(place.local(), original.destination().local());
            assert_eq!(place.projections().len(), 1);
            assert_eq!(
                place.projections()[0].kind(),
                SemanticProjectionKindV1::Dereference
            );
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                panic!("forwarding call")
            };
            assert_eq!(
                call.arguments()[0],
                SemanticOperandV1::Move(reborrow.destination().clone())
            );
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_semantic_producer_leaves_fnmut_receiver_unchanged() {
    run(
        |tcx| {
            let (function, abi) = construct(tcx, shim(tcx, "entry_mut"), Mutation::None).unwrap();
            assert_eq!(function.abi(), &abi);
            assert_eq!(function.locals().len(), 4);
            assert_eq!(
                function.blocks()[function.entry().index() as usize]
                    .statements()
                    .len(),
                1
            );
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_semantic_producer_rejects_changed_receiver_callee_and_missing_custody() {
    run(
        |tcx| {
            for mutation in [
                Mutation::Receiver,
                Mutation::CalleeBinding,
                Mutation::LocalBudget,
                Mutation::MissingSharedType,
            ] {
                assert!(
                    construct(tcx, shim(tcx, "entry"), mutation).is_err(),
                    "{mutation:?}"
                );
            }
        },
        "abort",
    );
}
