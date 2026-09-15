//! Mutations of an actually imported source owner. Rebound inputs below are
//! explicitly inert component inputs, never refreshed frontend authentication.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextEntryTransferV1, ProductionKernelContextLoweringInputV1,
    ProductionScopedMatrixUseRelationV1, ProductionSemanticKirErrorV1,
};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

type Imported = crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1;

include!("scoped_transport_regressions.rs");
include!("scoped_carrier_lifetime_tests.rs");

fn request(
    mir: &AdmittedInertSemanticMirV1,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        functions,
        callables,
        mir.roots().to_vec(),
    )
    .unwrap()
}

fn owner(mir: AdmittedInertSemanticMirV1) -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(mir, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

// Preserve actual entry coordinates and only rebind the inert document digest.
// The source-binding bytes remain inert here; no private frontend custody is
// refreshed or used to authorize this deliberately modified source document.
fn component_input(
    imported: &Imported,
    owner: &ProductionSemanticSsaOwnerV1,
) -> (
    ProductionKernelContextLoweringInputV1,
    Option<ProductionKernelContextEntryTransferV1>,
) {
    let contexts = &imported.kernel_contexts;
    let root = owner.source_semantic().roots()[0];
    let source = contexts
        .roots
        .iter()
        .find(|r| r.selected_root == root)
        .unwrap();
    let transfer = source.entry_transfer.map(|original| {
        let b = original.commitment_bytes();
        let id = |offset| <[u8; 32]>::try_from(&b[offset..offset + 32]).unwrap();
        let word = |offset| u32::from_le_bytes(b[offset..offset + 4].try_into().unwrap());
        let rebound = ProductionKernelContextEntryTransferV1::new(
            *owner.source_semantic().semantic_sha256().as_bytes(),
            SemanticFunctionIdentityV1::from_sha256(id(32)),
            SemanticFunctionIdentityV1::from_sha256(id(64)),
            SemanticFunctionIdentityV1::from_sha256(id(96)),
            SemanticTypeIdentityV1::from_sha256(id(128)),
            SemanticBlockIdV1::from_index(word(192)),
            SemanticLocalIdV1::from_index(word(196)),
            SemanticBlockIdV1::from_index(word(200)),
            word(204),
            id(160),
        );
        assert_eq!(&rebound.commitment_bytes()[32..], &b[32..]);
        rebound
    });
    let mut input = ProductionKernelContextLoweringInputV1::new(
        root,
        contexts.frontend_unit_identity,
        source.kernel_marker_identity,
        contexts.target_brand_identity,
        source.launch_brand_identity,
        source.issuance_identity,
    );
    if let Some(t) = transfer {
        input = input.with_entry_transfer(t);
    }
    (input, transfer)
}

fn check_component(
    imported: &Imported,
    owner: &ProductionSemanticSsaOwnerV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (input, transfer) = component_input(imported, owner);
    let entry = transfer
        .map(|t| t.checked_ssa_relation(owner, input.selected_root(), 1_048_576))
        .transpose()?;
    ProductionScopedMatrixUseRelationV1::checked_source_uses(
        owner,
        &input,
        entry.as_ref(),
        1_048_576,
    )
    .map(|_| ())
}

fn replace_block(
    function: &SemanticFunctionDeclV1,
    block: usize,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    // These mutations target ordinary source callers, not an exported entry or
    // a constructor record whose body digest would need regeneration.
    assert!(function.export().is_none());
    assert!(function.defined_capability_contract().is_none());
    let old = &function.blocks()[block];
    let mut blocks = function.blocks().to_vec();
    blocks[block] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        SemanticTerminatorV1::new(old.terminator().source(), terminator),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}

pub(super) fn check(imported: &Imported) {
    let mir = &imported.semantic_mir;
    let unchanged = request(mir, mir.functions().to_vec(), mir.callables().to_vec())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(unchanged.canonical_encoding(), mir.canonical_encoding());
    let original = owner(unchanged);
    check_component(imported, &original).expect("the unchanged real-owner control must pass first");
    check_actual_transport_roster(&original);
    let root = mir.roots()[0];
    let bindings = original
        .execution_expansion()
        .defined_capability_bindings(mir)
        .unwrap();
    let narrow = bindings
        .iter()
        .find_map(|b| match b.contract() {
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(r) if b.root() == root => {
                Some(r)
            }
            _ => None,
        })
        .expect("actual narrowed source record");

    // Locate an actual shared borrow of the owned narrowed receiver immediately
    // before its original multiply terminal. No local/function numbers are fixed.
    let (function, block, referent) = mir.functions().iter().enumerate().find_map(|(fi, f)| {
        if f.export().is_some() || f.defined_capability_contract().is_some() { return None; }
        f.blocks().iter().enumerate().find_map(|(bi, b)| {
            let SemanticTerminatorKindV1::Call(call) = b.terminator().kind() else { return None; };
            if !matches!(mir.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { context, .. }, ..
                }) if *context == narrow.types().narrowed) { return None; }
            let (SemanticOperandV1::Copy(reference) | SemanticOperandV1::Move(reference)) = &call.arguments()[0]
                else { return None; };
            b.statements().iter().rev().find_map(|s| match s.kind() {
                SemanticStatementKindV1::Assign(a) if a.destination().local() == reference.local() => {
                    match a.value().kind() {
                        SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place }
                            if place.projections().is_empty() && place.ty() == narrow.types().narrowed =>
                            Some((fi, bi, place.clone())),
                        _ => None,
                    }
                }
                _ => None,
            })
        })
    }).expect("real typed multiply must retain its owned receiver borrow");
    for mutation in 0..3 {
        let f = &mir.functions()[function];
        let b = &f.blocks()[block];
        let mut statements = b.statements().to_vec();
        let kill = match mutation {
            0 => SemanticStatementKindV1::StorageDead(referent.local()),
            1 => SemanticStatementKindV1::Deinitialize(referent.clone()),
            _ => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                referent.clone(),
                SemanticRvalueV1::new(
                    referent.ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(referent.clone())),
                ),
            )),
        };
        statements.push(SemanticStatementV1::new(b.source(), kill));
        let mut functions = mir.functions().to_vec();
        functions[function] = replace_block(f, block, statements, b.terminator().kind().clone());
        let changed = request(mir, functions, mir.callables().to_vec())
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        let changed = owner(changed);
        changed.verify_replay().unwrap();
        let error = check_component(imported, &changed).unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail.contains("loan") || detail.contains("storage") || detail.contains("borrow")
                || detail.contains("overwritten") || detail.contains("SSA versions")),
            "owner kill {mutation} must hit custody, not an unrelated resource/fixture failure: {error:?}"
        );
    }

    let (slot, exact) = mir
        .callables()
        .iter()
        .enumerate()
        .find_map(|(i, c)| match c {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } if matches!(
                contract.operation(),
                SemanticExecutionCapabilityOperationV1::MatrixAccess { .. }
            ) =>
            {
                Some((i, *contract))
            }
            _ => None,
        })
        .unwrap();
    let changed_identity = |old: SemanticTypeIdentityV1| {
        let mut bytes = *old.as_bytes();
        bytes[31] ^= 0x80;
        SemanticTypeIdentityV1::from_sha256(bytes)
    };
    for axis in 0..4 {
        let mut operation = exact.operation();
        if axis == 2 || axis == 3 {
            let SemanticExecutionCapabilityOperationV1::MatrixAccess {
                subgroup,
                epoch,
                matrix,
                subgroup_brand,
                width,
            } = operation
            else {
                unreachable!()
            };
            operation = SemanticExecutionCapabilityOperationV1::MatrixAccess {
                subgroup,
                epoch,
                matrix,
                subgroup_brand: if axis == 2 {
                    changed_identity(subgroup_brand)
                } else {
                    subgroup_brand
                },
                width: if axis == 3 { 32 } else { width },
            };
        }
        let changed = SemanticExecutionCapabilityContractV1::new(
            operation,
            exact.signature(),
            exact.provenance(),
            if axis == 0 {
                changed_identity(exact.workgroup_brand().unwrap())
            } else {
                exact.workgroup_brand().unwrap()
            },
            if axis == 1 {
                changed_identity(exact.epoch_before().unwrap())
            } else {
                exact.epoch_before().unwrap()
            },
            exact.epoch_after(),
            exact.source_identity(),
        );
        let Ok(changed) = changed else {
            eprintln!("scoped source mutation {axis}: typed contract rejected");
            continue;
        };
        let mut callables = mir.callables().to_vec();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[slot]
        else {
            unreachable!()
        };
        *operation =
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: changed };
        reject_at_source_or_custody(
            imported,
            request(mir, mir.functions().to_vec(), callables),
            axis,
        );
    }

    // Actual Bind arguments and its exceptional-return mode are part of the
    // source constructor relation, not facts restored from a type-indexed map.
    let bind_function = bindings
        .iter()
        .find_map(|b| match b.contract() {
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(r) if b.root() == root => {
                Some(r.function())
            }
            _ => None,
        })
        .unwrap();
    let (fi, bi, call) = mir
        .functions()
        .iter()
        .enumerate()
        .find_map(|(fi, f)| {
            if f.export().is_some() || f.defined_capability_contract().is_some() {
                return None;
            }
            f.blocks()
                .iter()
                .enumerate()
                .find_map(|(bi, b)| match b.terminator().kind() {
                    SemanticTerminatorKindV1::Call(c)
                        if matches!(mir.callables()[c.callee().index() as usize],
                SemanticCallableDeclV1::Defined { function } if function == bind_function) =>
                    {
                        Some((fi, bi, c))
                    }
                    _ => None,
                })
        })
        .unwrap();
    for mutation in 0..3 {
        let mut arguments = call.arguments().to_vec();
        if mutation == 0 {
            arguments.swap(0, 1);
        }
        let destination = if mutation == 2 {
            None
        } else {
            call.destination().cloned()
        };
        let changed = SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            destination,
            if mutation == 1 {
                SemanticUnwindActionV1::Continue
            } else {
                call.unwind()
            },
        )
        .unwrap();
        let f = &mir.functions()[fi];
        let mut functions = mir.functions().to_vec();
        functions[fi] = replace_block(
            f,
            bi,
            f.blocks()[bi].statements().to_vec(),
            SemanticTerminatorKindV1::Call(changed),
        );
        reject_at_source_or_custody(
            imported,
            request(mir, functions, mir.callables().to_vec()),
            4 + mutation,
        );
    }
}

fn reject_at_source_or_custody(
    imported: &Imported,
    request: InertSemanticMirRequestV1,
    case: usize,
) {
    let admitted = match request.admit_current_production(SemanticMirLimitsV1::default()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("scoped source mutation {case}: admission rejected {error:?}");
            return;
        }
    };
    let semantic = match ProductionSemanticMirOwnerV1::try_new(
        admitted,
        ProductionSemanticMirLimitsV1::default(),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("scoped source mutation {case}: MIR owner rejected {error:?}");
            return;
        }
    };
    let owner = match ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        ProductionSemanticSsaLimitsV1::default(),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("scoped source mutation {case}: SSA owner rejected {error:?}");
            return;
        }
    };
    owner.verify_replay().unwrap();
    let error = check_component(imported, &owner).unwrap_err();
    assert!(
        !matches!(error, ProductionSemanticKirErrorV1::ResourceLimit { .. }),
        "source mutation {case} must not pass only by exhausting work: {error:?}"
    );
    eprintln!("scoped source mutation {case}: live source custody rejected {error:?}");
}
