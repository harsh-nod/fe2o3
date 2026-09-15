//! Actual two-phase source-derived ownership tests, not live source authority.
//! Alias and restarted-storage mutants query the real SSA owner's Borrow.
//! Unrestarted dead storage requires the exact earlier partial-move diagnostic.
//! A stale source seal is never the negative.
use super::super::{
    PhaseResult, linear_events, rejected,
    source_calls::{reserve, spend},
};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaBlockIdV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticMirOwnerV1, ProductionSemanticSsaErrorV1, ProductionSemanticSsaOwnerV1,
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError, ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceSiteV1, SemanticPartialMoveViolationV1,
};

// Reuse the real complete-predecessor ordering consumer, not numeric block order
// or a second CFG implementation. This additional module is test-only.
#[path = "ssa_phase_order.rs"]
mod existing_order;

#[path = "scoped_unique_carrier_tests.rs"]
mod scoped_aliases;

type Point = (usize, usize);
#[derive(Clone, Copy, Debug)]
enum Mutation {
    Original,
    CopyCapture,
    OverlappingRoots,
    DuplicateCaptureDefinition,
    DuplicateUniqueFields,
    UncheckedUniqueCopy,
    OwnerStorageDeath,
    OwnerStorageRestart,
    DuplicateScopedFieldCopy,
    DuplicateScopedFieldDefinition,
    UncheckedScopedLeafCopy,
}
struct Capture {
    caller: SemanticFunctionIdV1,
    closure: SemanticFunctionIdV1,
    local: SemanticLocalIdV1,
    construction: Point,
    reference: SemanticLocalIdV1,
    borrow: Point,
    owner: SemanticLocalIdV1,
    reference_type: SemanticTypeIdV1,
    owned_type: SemanticTypeIdV1,
    field: usize,
}

fn assignment(body: &SemanticFunctionDeclV1, point: Point) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(a) = body.blocks()[point.0].statements()[point.1].kind()
    else {
        panic!("actual source assignment")
    };
    a
}

fn assigned(
    source: SemanticSourceProvenanceV1,
    destination: SemanticPlaceV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    let ty = destination.ty();
    SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

fn definition(body: &SemanticFunctionDeclV1, local: SemanticLocalIdV1) -> Point {
    let mut found = None;
    for (block, b) in body.blocks().iter().enumerate() {
        for (statement, s) in b.statements().iter().enumerate() {
            if matches!(s.kind(), SemanticStatementKindV1::Assign(a)
                if a.destination().local() == local && a.destination().projections().is_empty())
            {
                assert!(
                    found.replace((block, statement)).is_none(),
                    "one actual source definition"
                );
            }
        }
    }
    found.expect("actual caller-local definition")
}

fn captures(
    owner: &ProductionSemanticSsaOwnerV1,
    borrow_points: [linear_events::Point; 2],
) -> Vec<Capture> {
    let semantic = owner.source_semantic();
    let root = semantic.roots()[0];
    let bindings = owner
        .execution_expansion()
        .defined_capability_bindings(semantic)
        .unwrap();
    let storage_pairs = bindings
        .iter()
        .filter_map(|b| match b.contract() {
            SemanticDefinedCapabilityContractV1::ReusablePhase(r) => match r.recipe() {
                R::Bind {
                    storage_reference, ..
                } => Some((storage_reference.reference, storage_reference.pointee)),
                _ => None,
            },
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        storage_pairs.len(),
        1,
        "one actual reused allocation, not scalar stand-ins"
    );
    let (reference_type, owned_type) = *storage_pairs.first().unwrap();
    assert!(
        matches!(semantic.types()[reference_type.index() as usize].shape(),
        SemanticTypeShapeV1::Pointer(p) if p.kind() == SemanticPointerKindV1::Reference
            && p.mutability() == SemanticMutabilityV1::Mutable && p.pointee() == owned_type)
    );
    let mut result = Vec::new();
    for b in &bindings {
        if b.root() != root {
            continue;
        }
        let SemanticDefinedCapabilityContractV1::ReusablePhase(record) = b.contract() else {
            continue;
        };
        let R::WithPhase { relay, .. } = record.recipe() else {
            continue;
        };
        let body = &semantic.functions()[b.caller_function().index() as usize];
        assert!(
            body.defined_capability_contract().is_none(),
            "mutate only ordinary caller source"
        );
        let SemanticTerminatorKindV1::Call(call) = body.blocks()[b.call_block().index() as usize]
            .terminator()
            .kind()
        else {
            panic!("original wrapper call")
        };
        let [_, SemanticOperandV1::Move(environment)] = call.arguments() else {
            panic!("actual moved closure environment")
        };
        let construction = definition(body, environment.local());
        let SemanticRvalueKindV1::Aggregate(aggregate) =
            assignment(body, construction).value().kind()
        else {
            panic!("actual closure aggregate")
        };
        let fields = aggregate
            .operands()
            .iter()
            .enumerate()
            .filter(|(_, operand)| operand.ty() == reference_type)
            .collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            1,
            "one exact unique storage leaf in each real capture"
        );
        let (field, operand) = fields[0];
        let (SemanticOperandV1::Copy(reference) | SemanticOperandV1::Move(reference)) = operand
        else {
            panic!("actual reference operand")
        };
        assert!(reference.projections().is_empty());
        let borrow = definition(body, reference.local());
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        } = assignment(body, borrow).value().kind()
        else {
            panic!("original unique borrow, not an invented issuer")
        };
        assert!(place.projections().is_empty());
        assert_eq!(place.ty(), owned_type);
        result.push(Capture {
            caller: b.caller_function(),
            closure: relay.closure_function,
            local: environment.local(),
            construction,
            reference: reference.local(),
            borrow,
            owner: place.local(),
            reference_type,
            owned_type,
            field,
        });
    }
    assert_eq!(result.len(), 2);
    let view = owner.execution_view_for_root(root).unwrap();
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    let mut work = 65_536usize;
    let mut indices = [0; 2];
    for (slot, point) in borrow_points.iter().enumerate() {
        let site = query
            .event_site(point.block, point.event, &mut || {
                spend(&mut work, 1).is_ok()
            })
            .unwrap();
        let origin = &view.block_origins()[site.block().index() as usize];
        let Some(SemanticExpandedStatementOriginV1::Source { statement }) =
            origin.statements().get(site.statement().unwrap() as usize)
        else {
            panic!("checked root borrow must be original source");
        };
        let source_point = (origin.block().index() as usize, *statement as usize);
        let a = assignment(
            view.body(),
            (
                site.block().index() as usize,
                site.statement().unwrap() as usize,
            ),
        );
        assert!(
            matches!(query.plan().plan().resolved_event(point.block, point.event),
            Some(fe2o3_mir_model::SsaResolvedEventV1::Define { variable, .. })
                if *variable == SsaVariableIdV1::new(a.destination().local().index()))
        );
        let local = view.local_origins()[a.destination().local().index() as usize];
        assert_eq!(local.instance(), origin.instance());
        assert_eq!(local.function(), origin.function());
        assert!(matches!(
            a.value().kind(),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                ..
            }
        ));
        let matches = result
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.caller == origin.function()
                    && c.borrow == source_point
                    && c.reference == local.local()
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            panic!("one capture per exact checked root-borrow point");
        };
        indices[slot] = *index;
    }
    assert_ne!(indices[0], indices[1]);
    let forward =
        existing_order::precedes(&query, borrow_points[0], borrow_points[1], &mut work).unwrap();
    let reverse =
        existing_order::precedes(&query, borrow_points[1], borrow_points[0], &mut work).unwrap();
    assert_ne!(
        forward, reverse,
        "two actual captures must have one strict predecessor order"
    );
    let first_index = indices[usize::from(!forward)];
    if first_index != 0 {
        result.swap(0, first_index);
    }
    assert_eq!(result[0].caller, result[1].caller);
    assert_ne!(result[0].closure, result[1].closure);
    assert_eq!(result[0].owner, result[1].owner);
    assert_ne!(result[0].reference, result[1].reference);
    assert_ne!(result[0].local, result[1].local);
    result
}

fn rebuild_function(
    f: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut result = SemanticFunctionDeclV1::new(
        f.identity(),
        f.role(),
        f.item_definition_identity(),
        f.monomorphization_identity(),
        f.generic_type_arguments_identity(),
        f.const_generic_arguments_identity(),
        f.source(),
        f.abi().clone(),
        locals,
        f.entry(),
        blocks,
    )
    .unwrap();
    if let Some(export) = f.export() {
        result = match export {
            SemanticFunctionExportV1::Kernel(entry) => result.with_kernel_entry(entry.clone()),
            SemanticFunctionExportV1::DeviceFfi { export_symbol } => {
                result.with_device_ffi_export_symbol(export_symbol.clone())
            }
        };
    }
    if let Some(contract) = f.defined_capability_contract()
        && !matches!(
            contract,
            SemanticDefinedCapabilityContractV1::ReusablePhase(_)
        )
    {
        result = result.with_defined_capability_contract(*contract).unwrap();
    }
    result
}

fn next_identity(mut value: [u8; 32]) -> [u8; 32] {
    for byte in value.iter_mut().rev() {
        if let Some(next) = byte.checked_add(1) {
            *byte = next;
            return value;
        }
        *byte = 0;
    }
    panic!("fixture identity inventory exhausted")
}

fn duplicate_type(
    types: &mut Vec<SemanticTypeDeclV1>,
    reference: SemanticTypeIdV1,
) -> SemanticTypeIdV1 {
    let id = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    let identity = next_identity(*types.last().unwrap().identity().as_bytes());
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity),
        SemanticLayoutIdentityV1::from_sha256(identity),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: pointer.clone(),
                second: pointer,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![reference, reference]).unwrap(),
        ),
    ));
    id
}

fn mutant(
    source: &AdmittedInertSemanticMirV1,
    captures: &[Capture],
    mutation: Mutation,
) -> AdmittedInertSemanticMirV1 {
    assert!(source.transpose_owned_flows().is_empty());
    let first = &captures[0];
    let second = &captures[1];
    let mut functions = source
        .functions()
        .iter()
        .map(|f| rebuild_function(f, f.locals().to_vec(), f.blocks().to_vec()))
        .collect::<Vec<_>>();
    let f = &source.functions()[first.caller.index() as usize];
    let mut locals = f.locals().to_vec();
    let mut statements = f
        .blocks()
        .iter()
        .map(|b| b.statements().to_vec())
        .collect::<Vec<_>>();
    let mut types = source.types().to_vec();
    if !matches!(mutation, Mutation::Original) {
        for capture in captures {
            let a = assignment(f, capture.construction);
            let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
                unreachable!()
            };
            let mut operands = aggregate.operands().to_vec();
            operands[capture.field] = SemanticOperandV1::Copy(
                SemanticPlaceV1::new(capture.reference, vec![], capture.reference_type).unwrap(),
            );
            statements[capture.construction.0][capture.construction.1] = assigned(
                f.source(),
                a.destination().clone(),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(aggregate.kind().clone(), operands).unwrap(),
                ),
            );
        }
    }
    match mutation {
        Mutation::Original
        | Mutation::CopyCapture
        | Mutation::DuplicateScopedFieldCopy
        | Mutation::DuplicateScopedFieldDefinition
        | Mutation::UncheckedScopedLeafCopy => {}
        Mutation::OverlappingRoots => {
            let moved = statements[second.borrow.0][second.borrow.1].clone();
            statements[second.borrow.0][second.borrow.1] =
                SemanticStatementV1::new(f.source(), SemanticStatementKindV1::Nop);
            statements[first.borrow.0].insert(first.borrow.1 + 1, moved);
        }
        Mutation::DuplicateCaptureDefinition => {
            let duplicate = statements[first.construction.0][first.construction.1].clone();
            statements[first.construction.0].insert(first.construction.1 + 1, duplicate);
        }
        Mutation::DuplicateUniqueFields => {
            let ty = duplicate_type(&mut types, first.reference_type);
            let local = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(next_identity(
                    *locals.last().unwrap().identity().as_bytes(),
                )),
                ty,
                SemanticLocalRoleV1::Temporary,
                f.source(),
            ));
            let reference = SemanticOperandV1::Copy(
                SemanticPlaceV1::new(first.reference, vec![], first.reference_type).unwrap(),
            );
            let duplicate = assigned(
                f.source(),
                SemanticPlaceV1::new(local, vec![], ty).unwrap(),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![reference.clone(), reference],
                    )
                    .unwrap(),
                ),
            );
            statements[first.construction.0].insert(first.construction.1, duplicate);
        }
        Mutation::UncheckedUniqueCopy => {
            let local = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(next_identity(
                    *locals.last().unwrap().identity().as_bytes(),
                )),
                first.reference_type,
                SemanticLocalRoleV1::Temporary,
                f.source(),
            ));
            let duplicate = assigned(
                f.source(),
                SemanticPlaceV1::new(local, vec![], first.reference_type).unwrap(),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(first.reference, vec![], first.reference_type).unwrap(),
                )),
            );
            statements[first.construction.0].insert(first.construction.1 + 1, duplicate);
        }
        Mutation::OwnerStorageDeath | Mutation::OwnerStorageRestart => {
            statements[first.construction.0].insert(
                first.construction.1 + 1,
                SemanticStatementV1::new(
                    f.source(),
                    SemanticStatementKindV1::StorageDead(first.owner),
                ),
            );
            if matches!(mutation, Mutation::OwnerStorageRestart) {
                statements[first.construction.0].insert(
                    first.construction.1 + 2,
                    SemanticStatementV1::new(
                        f.source(),
                        SemanticStatementKindV1::StorageLive(first.owner),
                    ),
                );
            }
        }
    }
    let blocks = f
        .blocks()
        .iter()
        .zip(statements)
        .map(|(block, statements)| {
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    functions[first.caller.index() as usize] = rebuild_function(f, locals, blocks);
    let changed_closure = scoped_aliases::mutate(source, first, &mut functions, mutation);

    // Recompute inert commitments so failures must be at the ownership boundary,
    // not stale hashes. These rows never replace the live source seal.
    let mut records = source
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) => Some(*record),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !matches!(mutation, Mutation::Original) {
        for record in &records {
            let R::WithPhase { invoke_block, .. } = record.recipe() else {
                continue;
            };
            let index = record.function().index() as usize;
            let body = functions[index].clone();
            let mut blocks = body.blocks().to_vec();
            let block = &blocks[invoke_block.index() as usize];
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                panic!("exact RustCall invocation")
            };
            assert!(call.variadic_argument_abis().is_empty());
            let mut arguments = call.arguments().to_vec();
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = &arguments[0]
            else {
                panic!("actual closure environment")
            };
            arguments[0] = SemanticOperandV1::Copy(place.clone());
            let terminator = SemanticTerminatorV1::new(
                block.terminator().source(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        call.callee(),
                        arguments,
                        call.destination().cloned(),
                        call.unwind(),
                    )
                    .unwrap(),
                ),
            );
            blocks[invoke_block.index() as usize] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                terminator,
            )
            .unwrap();
            functions[index] = rebuild_function(&body, body.locals().to_vec(), blocks);
        }
    }
    records.sort_by_key(|record| match record.recipe() {
        R::WithPhase { .. } => 1,
        _ => 0,
    });
    let mut work = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    for record in records {
        let recipe = scoped_aliases::recommit_closure(
            record.recipe(),
            changed_closure,
            &functions,
            &mut work,
        );
        let replacement = SemanticDefinedReusablePhaseV1::for_defined_function(
            record.function(),
            &functions,
            source.callables(),
            &types,
            record.provenance(),
            *record.source_binding(),
            recipe,
            &mut work,
        )
        .unwrap();
        let index = record.function().index() as usize;
        functions[index] = functions[index]
            .clone()
            .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::ReusablePhase(
                replacement,
            ))
            .unwrap();
    }
    InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn build(source: AdmittedInertSemanticMirV1, mutation: Mutation) -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, Default::default()).unwrap(),
        Default::default(),
    )
    .unwrap_or_else(|error| {
        panic!("{mutation:?}: must reach exact retained Borrow query, got {error:?}")
    })
}

fn check_borrow(
    owner: &ProductionSemanticSsaOwnerV1,
    capture: &Capture,
    promoted: bool,
    mutation: Mutation,
) {
    owner.verify_replay().unwrap();
    let root = owner.source_semantic().roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    let mut found = 0;
    let mut work = 65_536usize;
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function() != capture.caller {
            continue;
        }
        for (statement, marker) in origin.statements().iter().enumerate() {
            if !matches!(marker, SemanticExpandedStatementOriginV1::Source { .. }) {
                continue;
            }
            let SemanticStatementKindV1::Assign(a) =
                view.body().blocks()[block].statements()[statement].kind()
            else {
                continue;
            };
            let destination_origin = view.local_origins()[a.destination().local().index() as usize];
            if destination_origin.function() != capture.caller
                || destination_origin.local() != capture.reference
            {
                continue;
            }
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place,
            } = a.value().kind()
            else {
                panic!("original mutable Borrow must survive")
            };
            assert_eq!(place.ty(), capture.owned_type);
            let source_origin = view.local_origins()[place.local().index() as usize];
            assert_eq!(source_origin.instance(), destination_origin.instance());
            assert_eq!(source_origin.function(), capture.caller);
            assert_eq!(source_origin.local(), capture.owner);
            let variable = SsaVariableIdV1::new(place.local().index());
            assert_eq!(
                query.plan().plan().promoted_variables().contains(&variable),
                promoted,
                "{mutation:?}"
            );
            let result = query.borrow_place_use(
                ProductionSemanticSsaSourceSiteV1::new(
                    SemanticBlockIdV1::from_index(block as u32),
                    Some(statement as u32),
                ),
                place,
                &mut || {
                    if work == 0 {
                        false
                    } else {
                        work -= 1;
                        true
                    }
                },
            );
            if promoted {
                result.expect("actual unique owner has its exact original Borrow Use");
            } else {
                assert_eq!(
                    result,
                    Err(QueryError::NoPromotedUse),
                    "{mutation:?}: not a stale owner/site/resource failure"
                );
            }
            found += 1;
        }
    }
    assert_eq!(found, 1, "one exact instantiated source Borrow per capture");
}

fn check_copy_paths(owner: &ProductionSemanticSsaOwnerV1, captures: &[Capture]) {
    let semantic = owner.source_semantic();
    let view = owner.execution_view_for_root(semantic.roots()[0]).unwrap();
    let closures = semantic
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusablePhase(r)) => match r.recipe() {
                R::WithPhase { relay, .. } => Some(relay.closure_function),
                _ => None,
            },
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let reference = captures[0].reference_type;
    let (mut aggregates, mut parameters, mut fields) = (0, 0, 0);
    for (block, origin) in view.block_origins().iter().enumerate() {
        for (statement, marker) in origin.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) =
                view.body().blocks()[block].statements()[statement].kind()
            else {
                continue;
            };
            let destination = view.local_origins()[a.destination().local().index() as usize];
            match (marker, a.value().kind()) {
                (
                    SemanticExpandedStatementOriginV1::Source { .. },
                    SemanticRvalueKindV1::Aggregate(aggregate),
                ) if captures.iter().any(|capture| {
                    destination.function() == capture.caller && destination.local() == capture.local
                }) =>
                {
                    assert_eq!(aggregate.operands().iter().filter(|operand|
                        matches!(operand, SemanticOperandV1::Copy(p) if p.ty() == reference && p.projections().is_empty())).count(), 1);
                    aggregates += 1;
                }
                (
                    SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 0, .. },
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)),
                ) if closures.contains(&destination.function()) => {
                    assert!(p.projections().is_empty());
                    parameters += 1;
                }
                (
                    SemanticExpandedStatementOriginV1::Source { .. },
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)),
                ) if closures.contains(&origin.function())
                    && a.destination().ty() == reference
                    && matches!(p.projections(), [projection] if matches!(projection.kind(), SemanticProjectionKindV1::Field(_))) =>
                {
                    assert!(
                        matches!(p.projections(), [projection] if matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
                    );
                    fields += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        (aggregates, parameters, fields),
        (2, 2, 2),
        "exercise all three actual unique Copy-carriage hooks, not scalar captures"
    );
}

fn check_dead_owner(source: AdmittedInertSemanticMirV1, second: &Capture, mutation: Mutation) {
    let body = &source.functions()[second.caller.index() as usize];
    let point = definition(body, second.reference);
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Mutable,
        place,
    } = assignment(body, point).value().kind()
    else {
        panic!("original second owned-storage borrow must survive");
    };
    assert_eq!(place.local(), second.owner);
    assert_eq!(place.ty(), second.owned_type);
    assert!(place.projections().is_empty());
    let expected = (
        second.caller,
        u32::try_from(point.0).unwrap(),
        Some(u32::try_from(point.1).unwrap()),
        second.owner.index(),
        SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
    );
    // Without a new storage lifetime, partial-move analysis rejects the next
    // borrow before a promoted-use query is possible.
    let error = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, Default::default()).unwrap(),
        Default::default(),
    )
    .err()
    .unwrap_or_else(|| panic!("{mutation:?}: dead owner was accepted"));
    let ProductionSemanticSsaErrorV1::PartialMove {
        function,
        block,
        statement,
        local,
        violation,
    } = error
    else {
        panic!("{mutation:?}: expected exact dead-owner rejection, got {error:?}");
    };
    assert_eq!(
        (function, block, statement, local, violation),
        expected,
        "{mutation:?}"
    );
}

pub(super) fn check(
    original: &ProductionSemanticSsaOwnerV1,
    borrow_points: [linear_events::Point; 2],
) {
    let captures = captures(original, borrow_points);
    let reversed = self::captures(original, [borrow_points[1], borrow_points[0]]);
    for (forward, reverse) in captures.iter().zip(&reversed) {
        assert_eq!(forward.caller, reverse.caller);
        assert_eq!(forward.closure, reverse.closure);
        assert_eq!(forward.local, reverse.local);
        assert_eq!(forward.reference, reverse.reference);
        assert_eq!(forward.borrow, reverse.borrow);
    }
    let same = mutant(original.source_semantic(), &captures, Mutation::Original);
    assert_eq!(
        same.canonical_encoding(),
        original.source_semantic().canonical_encoding(),
        "test rebuild must preserve every original declaration/contract"
    );
    let replay = build(same, Mutation::Original);
    for capture in &captures {
        check_borrow(&replay, capture, true, Mutation::Original);
    }
    let copied = build(
        mutant(original.source_semantic(), &captures, Mutation::CopyCapture),
        Mutation::CopyCapture,
    );
    check_copy_paths(&copied, &captures);
    for capture in &captures {
        check_borrow(&copied, capture, true, Mutation::CopyCapture);
    }
    for mutation in [
        Mutation::OverlappingRoots,
        Mutation::DuplicateCaptureDefinition,
        Mutation::DuplicateUniqueFields,
        Mutation::UncheckedUniqueCopy,
        Mutation::DuplicateScopedFieldCopy,
        Mutation::DuplicateScopedFieldDefinition,
        Mutation::UncheckedScopedLeafCopy,
        Mutation::OwnerStorageRestart,
    ] {
        let changed = mutant(original.source_semantic(), &captures, mutation);
        assert_ne!(
            changed.canonical_encoding(),
            original.source_semantic().canonical_encoding()
        );
        let owner = build(changed, mutation);
        for capture in &captures {
            check_borrow(&owner, capture, false, mutation);
        }
    }
    let mutation = Mutation::OwnerStorageDeath;
    let changed = mutant(original.source_semantic(), &captures, mutation);
    assert_ne!(
        changed.canonical_encoding(),
        original.source_semantic().canonical_encoding()
    );
    check_dead_owner(changed, &captures[1], mutation);
}
