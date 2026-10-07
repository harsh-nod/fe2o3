//! Genuine Carrier-only Product demands, separate from the V280 tile matrix.
use super::*;
use fe2o3_mir_model::{SsaVariableIdV1, semantic_mir_v1 as mir};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::product_frames::product_frame_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Observation {
    pub(super) model: [u8; 32],
    pub(super) census: [u8; 32],
    pub(super) runtime: [u8; 32],
    pub(super) carries: [usize; 2],
    pub(super) forwarding: [usize; 2],
    pub(super) forwarding_identity: [u8; 32],
}

pub(super) fn definition_identity(
    hash: &mut Sha256,
    definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut Budget<'_>,
) -> Result<(), SourceError> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as D;
    let fields = match definition {
        D::FunctionArgument { function, argument } => [0, function.0, argument, 0, 0],
        D::BlockArgument { block, argument } => [1, block.function.0, block.block, argument, 0],
        D::Result { operation, result } => [
            2,
            operation.block.function.0,
            operation.block.block,
            operation.operation,
            result,
        ],
    };
    for field in fields {
        number(hash, field as usize, budget)?;
    }
    Ok(())
}

fn forwarding_identity(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    tile: &fe2o3_lower_mir_kernel::ProductionSourceTileExpansionV159<'_, '_>,
    rows: &ExpandedSupportCensusV280<'_>,
    budget: &mut Budget<'_>,
) -> Result<([usize; 2], [u8; 32]), SourceError> {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
    use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as D, ScalarType, Type};
    use fe2o3_verifier::ExpandedSupportForwardingV288 as E;
    let scratch = 3 * std::mem::size_of::<Sha256>()
        + 4 * std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 4 * std::mem::size_of::<E>()
        + 2 * std::mem::size_of::<Inventory<'_>>()
        + 2 * std::mem::size_of::<Type>()
        + 32 * std::mem::size_of::<&()>()
        + 128 * std::mem::size_of::<usize>();
    budget.reserve_storage(scratch)?;
    let input = original.inventory(budget)?;
    let neutral = tile.neutral_source_v162(budget)?;
    let prior = neutral.output_inventory(budget)?;
    let (actual, storage) = Inventory::derive_v18(tile.output(budget)?, budget)
        .expect("independent inventory of the authentic expanded owner");
    budget.reserve_storage(storage.retained_storage())?;
    assert!(std::ptr::eq(actual.owner(), tile.output(budget)?));
    let mut hash = Sha256::new();
    let mut counts = [0usize; 2];
    let mut at = 0;
    while at < rows.forwarding_v288.len() {
        budget.charge_work(12)?;
        let E::Begin {
            root,
            instance,
            value,
            atom,
            source_type,
            original: first,
            coordinate,
        } = rows.forwarding_v288[at]
        else {
            panic!("each complete observed path starts at its demanded original component");
        };
        at += 1;
        assert!(root < 2);
        assert!(rows.frame_demands_v281.iter().any(|demand| (
            demand.root,
            demand.instance,
            demand.value
        ) == (root, instance, value)));
        budget.charge_work(rows.frame_demands_v281.len())?;
        let endpoint = original.ssa_typed_endpoint_v36(root, instance, value, budget)?;
        let component = endpoint.component(atom, budget)?;
        assert_eq!(component.source_type(budget)?.index(), source_type);
        assert_eq!(component.original_definition(budget)?, Some(first));
        assert_eq!(input.definitions()[first].coordinate, coordinate);
        assert_eq!(
            component.physical_type(budget)?,
            Some(input.definitions()[first].ty)
        );
        let Type::Slice(slice) = input.definitions()[first].ty else {
            panic!("whole original Slice")
        };
        assert_eq!(slice.element.as_ref(), &Type::Scalar(ScalarType::U32));
        for field in [
            root,
            instance,
            atom,
            source_type as usize,
            first,
            slice.address_space as usize,
            slice.access as usize,
        ] {
            number(&mut hash, field, budget)?;
        }
        definition_identity(&mut hash, coordinate, budget)?;
        let cfg = neutral.output_root_cfg_v18(root, budget)?;
        let owner = cfg.function().coordinate;
        assert_eq!(rows.roots[root].target_function, owner);
        assert_eq!(
            actual.functions()[owner.0 as usize].function.id,
            cfg.function().function.id
        );
        assert!(actual.kernels().iter().any(|kernel| kernel.entry == owner));
        budget.charge_work(actual.kernels().len())?;
        let mut current = first;
        let mut erased = 0;
        let mut terminal = None;
        for _ in 0..input.definitions().len() {
            budget.charge_work(8)?;
            let row = &input.definitions()[current];
            assert_eq!(row.ty, input.definitions()[first].ty);
            let descendants = neutral.definition_descendants(row.coordinate, budget)?;
            if !descendants.is_empty() {
                assert_eq!(
                    rows.forwarding_v288[at],
                    E::Retained {
                        original: current,
                        coordinate: row.coordinate,
                        descendants: descendants.len(),
                    }
                );
                at += 1;
                // These scalar-root fixtures retain one whole-Slice descendant;
                // independently locate its exact value/type in the actual owner.
                let [descendant] = descendants else {
                    panic!("one complete retained Slice descendant")
                };
                let mut found = None;
                for definition in prior.definitions() {
                    budget.charge_work(1)?;
                    if definition.coordinate == descendant.output {
                        assert!(found.is_none());
                        found = Some(definition);
                    }
                }
                let retained = found.unwrap();
                assert_eq!(retained.ty, row.ty);
                let mut selected = None;
                for index in actual.functions()[owner.0 as usize].definitions.clone() {
                    budget.charge_work(3)?;
                    let candidate = &actual.definitions()[index];
                    if candidate.value == retained.value && candidate.ty == retained.ty {
                        assert!(selected.is_none());
                        selected = Some(index);
                    }
                }
                let selected =
                    selected.expect("unique whole-Slice target with original retained identity");
                assert_eq!(
                    rows.forwarding_v288[at],
                    E::Target {
                        actual: selected,
                        coordinate: actual.definitions()[selected].coordinate,
                        function: owner,
                    }
                );
                at += 1;
                for field in [current, descendants.len(), selected, owner.0 as usize] {
                    number(&mut hash, field, budget)?;
                }
                definition_identity(&mut hash, actual.definitions()[selected].coordinate, budget)?;
                terminal = Some(selected);
                break;
            }
            assert_eq!(
                rows.forwarding_v288[at],
                E::Erased {
                    original: current,
                    coordinate: row.coordinate
                }
            );
            at += 1;
            erased += 1;
            let D::BlockArgument { block, .. } = row.coordinate else {
                panic!("erased Slice is an original block argument")
            };
            number(&mut hash, current, budget)?;
            definition_identity(&mut hash, row.coordinate, budget)?;
            let mut incoming = None;
            let mut edges = 0;
            // Enumerate the full independent inventory, not a producer path count.
            for edge in input.edge_arguments() {
                budget.charge_work(1)?;
                if edge.target_definition != current {
                    continue;
                }
                budget.charge_work(8)?;
                let predecessor = &input.definitions()[edge.incoming_definition];
                assert_eq!(edge.coordinate.edge.source.function, block.function);
                assert!(
                    input.functions()[block.function.0 as usize]
                        .definitions
                        .contains(&edge.incoming_definition)
                );
                assert_eq!(predecessor.value, Some(edge.value));
                assert_eq!(predecessor.ty, row.ty);
                assert!(incoming.is_none_or(|prior| prior == edge.incoming_definition));
                incoming = Some(edge.incoming_definition);
                assert_eq!(
                    rows.forwarding_v288[at],
                    E::Incoming {
                        original: current,
                        edge: edge.coordinate,
                        incoming: edge.incoming_definition,
                        coordinate: predecessor.coordinate
                    }
                );
                at += 1;
                edges += 1;
                for field in [
                    edge.coordinate.edge.source.function.0,
                    edge.coordinate.edge.source.block,
                    edge.coordinate.edge.successor,
                    edge.coordinate.argument,
                ] {
                    number(&mut hash, field as usize, budget)?;
                }
                number(&mut hash, edge.incoming_definition, budget)?;
                definition_identity(&mut hash, predecessor.coordinate, budget)?;
            }
            assert!(edges > 0);
            current = incoming.unwrap();
        }
        assert!(
            terminal.is_some(),
            "bounded complete path, not a cyclic prefix"
        );
        if erased > 0 {
            counts[root] += 1;
        }
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "required genuine erased whole-Slice path for every root"
    );
    let digest = hash.finalize().into();
    drop(actual);
    budget.release_storage(storage.retained_storage())?;
    budget.release_storage(scratch)?;
    Ok((counts, digest))
}

fn carrier_fields(
    types: &[mir::SemanticTypeDeclV1],
    ty: mir::SemanticTypeIdV1,
) -> Option<[mir::SemanticTypeIdV1; 2]> {
    let mir::SemanticTypeShapeV1::Tuple(tuple) = types.get(ty.index() as usize)?.shape() else {
        return None;
    };
    let [slice, word] = tuple.fields() else {
        return None;
    };
    if !matches!(
        types[word.index() as usize].shape(),
        mir::SemanticTypeShapeV1::Scalar(mir::SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    ) {
        return None;
    }
    let mir::SemanticTypeShapeV1::Pointer(pointer) = types[slice.index() as usize].shape() else {
        return None;
    };
    if pointer.kind() != mir::SemanticPointerKindV1::Reference
        || pointer.mutability() != mir::SemanticMutabilityV1::Immutable
        || !matches!(types[pointer.pointee().index() as usize].shape(),
            mir::SemanticTypeShapeV1::Slice { element } if element == word)
    {
        return None;
    }
    Some([*slice, *word])
}

#[derive(Default)]
struct ProductCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for ProductCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let mut called = 0;
            let result = transaction.with_original_source_expanded_model_v280(&mut budget,
                |source, original, tile, roots, _, pair, model, budget| {
                    called += 1;
                    pair.check(source, original, tile, budget)?;
                    assert_eq!(roots.len(), 2);
                    assert_eq!(source.root_count(budget)?, 2);
                    assert!(pair.roots(budget)?.iter().all(|root| root.tile.is_none()));
                    let scratch = 2 * std::mem::size_of::<Sha256>()
                        + 4 * std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
                        + 64 * std::mem::size_of::<usize>();
                    budget.reserve_storage(scratch)?;
                    let semantic = source.source_semantic(budget)?;
                    let archive = source.source_ssa(budget)?;
                    let rows = model.census(budget).map_err(SourceError::ExpandedModel)?;
                    let mut carries = [0; 2];
                    for call in rows.calls {
                        budget.charge_work(1)?;
                        if !call.reachable || call.carries_v281.is_empty() {
                            continue;
                        }
                        let instance = &rows.instances[rows.roots[call.root].instances.start + call.caller];
                        let function_id = mir::SemanticFunctionIdV1::from_index(instance.source_function);
                        let function = &semantic.functions()[instance.source_function as usize];
                        for demand in &rows.frame_demands_v281[call.carries_v281.clone()] {
                            budget.charge_work(24)?;
                            let ty = function.locals()[demand.local].ty();
                            let Some(fields) = carrier_fields(semantic.types(), ty) else { continue; };
                            assert!(instance.active);
                            let child = call.child.expect("a suspended frame has an actual child");
                            assert!(source.instance_active(call.root, child, budget)?);
                            assert_eq!(original.defined_call_instance(call.root, call.caller,
                                mir::SemanticBlockIdV1::from_index(call.block), budget)?, child);
                            let ssa = archive.plan_for_function(function_id).unwrap();
                            assert!(ssa.plan().promoted_variables().contains(&SsaVariableIdV1::new(demand.local as u32)));
                            assert!(!ssa.retained_cross_edge_variables().contains(&SsaVariableIdV1::new(demand.local as u32)));
                            let endpoint = original.ssa_typed_endpoint_v36(call.root, call.caller, demand.value, budget)?;
                            assert_eq!(endpoint.source_function(budget)?, function_id);
                            assert_eq!(endpoint.source_local(budget)?.index() as usize, demand.local);
                            assert_eq!(endpoint.source_type(budget)?, ty);
                            for (ordinal, field) in fields.into_iter().enumerate() {
                                budget.charge_work(2)?;
                                let component = endpoint.component(ordinal, budget)?;
                                assert_eq!(component.source_type(budget)?, field);
                                assert!(component.original_definition(budget)?.is_some());
                                match (ordinal, component.physical_type(budget)?) {
                                    (0, Some(fe2o3_kernel_ir::Type::Slice(_)))
                                    | (1, Some(fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U32))) => (),
                                    _ => panic!("exact original Slice/U32 carrier types"),
                                }
                            }
                            let (mut constructions, mut whole_uses) = (0, 0);
                            for block in function.blocks() {
                                for statement in block.statements() {
                                    budget.charge_work(16)?;
                                    let mir::SemanticStatementKindV1::Assign(assign) = statement.kind() else { continue; };
                                    if assign.destination().local().index() as usize == demand.local {
                                        let mir::SemanticRvalueKindV1::Aggregate(aggregate) = assign.value().kind() else {
                                            panic!("held Product is constructed from genuine original operands");
                                        };
                                        assert_eq!(assign.value().result_type(), ty);
                                        assert_eq!(aggregate.kind(), &mir::SemanticAggregateKindV1::Tuple);
                                        assert_eq!(aggregate.operands().len(), 2);
                                        for (operand, field) in aggregate.operands().iter().zip(fields) {
                                            budget.charge_work(2)?;
                                            let (mir::SemanticOperandV1::Copy(place) | mir::SemanticOperandV1::Move(place)) = operand else {
                                                panic!("actual source carrier operands");
                                            };
                                            assert_eq!(place.ty(), field);
                                        }
                                        constructions += 1;
                                    }
                                    if matches!(assign.value().kind(), mir::SemanticRvalueKindV1::Use(
                                        mir::SemanticOperandV1::Copy(place) | mir::SemanticOperandV1::Move(place))
                                        if place.local().index() as usize == demand.local && place.projections().is_empty()) {
                                        whole_uses += 1;
                                    }
                                }
                            }
                            assert_eq!(constructions, 1);
                            assert!(whole_uses > 0);
                            let continuation = call.continuation_v281.expect("real original return continuation");
                            let cut = &rows.cuts[instance.cuts.start + continuation];
                            assert_eq!((cut.root, cut.instance, cut.block as usize),
                                (call.root, call.caller, continuation));
                            assert!(rows.frame_demands_v281[cut.current_v281.clone()].iter()
                                .any(|current| current.local == demand.local));
                            carries[call.root] += 1;
                        }
                    }
                    assert_eq!(carries, [1, 1], "one genuine original Product across a call per root");
                    let (forwarding, forwarding_identity) = forwarding_identity(original, tile, &rows, budget)?;
                    let bytes = model.generated_source(budget).map_err(SourceError::ExpandedModel)?;
                    budget.charge_work(bytes.len())?;
                    let text = std::str::from_utf8(bytes).unwrap();
                    assert!(text.contains("InvocationSourceProductAtomV282::Carrier(original)"));
                    assert!(text.contains("micro.observations == target_prefix"));
                    let observation = Observation {
                        model: Sha256::digest(bytes).into(),
                        census: census_identity(&rows, budget)?,
                        runtime: pair.subject(budget)?.runtime_and_instances,
                        carries,
                        forwarding,
                        forwarding_identity,
                    };
                    model_export::observe_product_frame(bytes, &observation, budget)?;
                    budget.release_storage(scratch)?;
                    Ok((observation, 0))
                }).map_err(|error| format!("actual Product frame model: {error:?}"))?;
            assert_eq!(called, 1);
            Ok(result.into_observation())
        })());
        Compilation::Stop
    }
}

fn source() -> String {
    let mut source = r#"use fe2o3_device::{kernel, KernelContext};
#[inline(never)]
fn shared_scalar(seed: u32) -> u32 { seed.wrapping_add(1) }
"#
    .to_owned();
    for name in ["first", "second"] {
        source.push_str(&format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(_ctx: KernelContext<'_>, input: &[u32], seed: u32) {{
    let held = (input, seed);
    let scalar = shared_scalar(seed);
    let continued = held;
    let _ = (continued.0, continued.1.wrapping_add(scalar));
}}
"#
        ));
    }
    source
}

#[test]
#[ignore = "process helper; exact ordinary source request supplied by its parent"]
fn product_frame_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ProductCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual Product frame callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "Product frame: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic ordinary AMD source compilation"]
fn actual_rustc_product_carriers_retain_current_and_suspended_source_demands_v283() {
    run_actual_sources::<Observation>(
        &[("carrier", "carrier"), ("carrier", "carrier")],
        &[(0, 0), (3, 0)],
        CHILD,
        "EXPANDED_PRODUCT_FRAME_V283",
        |_| source(),
        |_, _, label, outcome, observations| {
            assert_eq!(outcome.carries, [1, 1]);
            assert!(outcome.forwarding.iter().all(|count| *count > 0));
            if let Some(previous) = observations.get(label) {
                assert_eq!(&outcome, previous);
            } else {
                observations.insert(label.to_owned(), outcome);
            }
        },
    );
}
