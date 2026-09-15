//! Real registered AMD source through ranked checks and KIR, not final proof admission.
use super::*;
use fe2o3_kernel_ir::{
    BinaryOp, GlobalDisjointIndexSpaceV1, OperationKind, Type, VerifiedCanonicalKernelIrV13,
};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{
    Blocked, DisjointWrite, Global, Index1D, KernelCapabilityBrand, KernelContext,
    KernelError, KernelLaunch, KernelResult, KernelTarget, ReadOnly, kernel,
};
type Output<'k, K, T, L> =
    Global<'k, f32, DisjointWrite<Blocked<Index1D, 16, 4>>, KernelCapabilityBrand<'k, K, T, L>>;

fn grouped<'k, K, T: KernelTarget, L: KernelLaunch>(
    context: &KernelContext<'k, K, T, L>,
    first: &mut Output<'k, K, T, L>, second: &mut Output<'k, K, T, L>,
    a: f32, b: f32,
) -> KernelResult {
    let Some(block) = context.invocation().index_1d().checked_block::<16, 4>()
        else { return Err(KernelError::OutOfBounds); };
    if !first.store_block(&block, 0, a) || !first.store_block(&block, 1, a)
        || !first.store_block(&block, 2, a) || !first.store_block(&block, 3, a + b)
        || !second.store_block(&block, 0, b) || !second.store_block(&block, 1, b)
        || !second.store_block(&block, 2, b) || !second.store_block(&block, 3, a - b)
    { return Err(KernelError::OutOfBounds); }
    Ok(())
}

fn interleaved<'k, K, T: KernelTarget, L: KernelLaunch>(
    context: &KernelContext<'k, K, T, L>,
    first: &mut Output<'k, K, T, L>, second: &mut Output<'k, K, T, L>,
    a: f32, b: f32,
) -> KernelResult {
    let Some(first_block) = context.invocation().index_1d().checked_block::<16, 4>()
        else { return Err(KernelError::OutOfBounds); };
    let Some(second_block) = context.invocation().index_1d().checked_block::<16, 4>()
        else { return Err(KernelError::OutOfBounds); };
    if !first.store_block(&first_block, 0, a) || !second.store_block(&second_block, 0, b)
        || !first.store_block(&first_block, 1, a) || !second.store_block(&second_block, 1, b)
        || !first.store_block(&first_block, 2, a) || !second.store_block(&second_block, 2, b)
        || !first.store_block(&first_block, 3, a + b) || !second.store_block(&second_block, 3, a - b)
    { return Err(KernelError::OutOfBounds); }
    Ok(())
}

macro_rules! root {
    ($name:ident, $store:ident) => {
        #[kernel(typed, launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]))]
        pub fn $name(
            context: KernelContext<'_>, a: Global<'_, f32, ReadOnly>, b: Global<'_, f32, ReadOnly>,
            mut first: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
            mut second: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
        ) -> KernelResult {
            let index = context.invocation().index_1d().get();
            let Some(a) = a.load(index) else { return Err(KernelError::OutOfBounds); };
            let Some(b) = b.load(index) else { return Err(KernelError::OutOfBounds); };
            $store(&context, &mut first, &mut second, a, b)
        }
    };
}
root!(blocked_grouped, grouped);
root!(blocked_interleaved, interleaved);
"#;

struct BlockedProbe {
    cpu: &'static str,
    completed: bool,
}

impl Callbacks for BlockedProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("typed_blocked_global_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("collect actual branded blocked stores");
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("import actual blocked stores without body replacement");
        assert_eq!(imported.rustc_target.contract().cpu(), self.cpu);
        let mir = round_trip(&imported.semantic_mir);
        let u32_type = semantic_type_for_rust_v1(tcx, mir.types(), tcx.types.u32).unwrap();
        let mut blocked = 0;
        for (index, callable) in mir.callables().iter().enumerate() {
            if matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock { .. },
                    ..
                }
            ) {
                blocked += 1;
                mutations::check_record(&mir, index, u32_type);
            }
        }
        assert_eq!(
            blocked, 2,
            "one blocked terminal instance per distinct root brand"
        );
        check(imported, typed);
        self.completed = true;
        Compilation::Stop
    }
}

fn check(
    imported: crate::collector::ConstructedProductionSemanticMirV1,
    typed: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
        typed,
        &imported.semantic_mir,
    )
    .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed,
        &imported.semantic_mir,
    )
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        imported.semantic_mir,
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    for root in ssa.source_semantic().roots() {
        let view = ssa.execution_view_for_root(*root).unwrap();
        let count = view
            .body()
            .blocks()
            .iter()
            .filter(|block| {
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                    return false;
                };
                matches!(
                    &ssa.source_semantic().callables()[call.callee().index() as usize],
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock { .. },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(count, 8, "all actual expanded blocked store occurrences");
    }
    let roots = typed
        .iter()
        .map(|root| {
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                root.logical_name(),
                root.kernel_binding_bytes(),
                root.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = crate::production_ranked_projection_v1::global_receiver_replay_v1::observe(|| {
        crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(
            ssa, &roots, &imported.reference_effect_bindings, Some(&imported.kernel_contexts),
        )
    }).expect("exact blocked coordinates, allocation extents, reads, and source correspondence");
    let contexts = imported
        .kernel_contexts
        .into_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            &typed,
        )
        .unwrap();
    let (receipt, verification) = ranked
        .into_verified_roster_receipt()
        .unwrap()
        .into_module_verified_receipt()
        .unwrap();
    assert!(verification.every_functional_verification_is_coherent());
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_contexts(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            contexts,
        )
        .expect("actual shared witness and exact Global source must lower");
    lowered.verify_equivalence().unwrap();
    lowered
        .canonical_kernel_ir_v13()
        .unwrap()
        .revalidate()
        .unwrap();
    let module = lowered.module();
    assert_eq!(module.kernels.len(), 2);
    for kernel in &module.kernels {
        let function = module
            .functions
            .iter()
            .find(|function| function.id == kernel.entry)
            .unwrap();
        let body = function.body.as_ref().unwrap();
        let ops = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        let definitions = ops
            .iter()
            .flat_map(|op| op.results.iter().map(move |r| (r.id, *op)))
            .collect::<BTreeMap<_, _>>();
        let mut value_types = body
            .parameters
            .iter()
            .copied()
            .zip(function.signature.parameters.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        value_types.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .map(|value| (value.id, value.ty.clone())),
        );
        value_types.extend(
            ops.iter()
                .flat_map(|op| &op.results)
                .map(|value| (value.id, value.ty.clone())),
        );
        let mut stores = BTreeMap::new();
        let mut loads = BTreeSet::new();
        for op in &ops {
            match &op.kind {
                OperationKind::GuardedStore {
                    pointer,
                    predicate,
                    value,
                    access,
                } => {
                    assert_eq!(access.address_space, fe2o3_kernel_ir::AddressSpace::Global);
                    assert!(!access.volatile);
                    assert_eq!(value_types[value], Type::F32);
                    let OperationKind::GetElementPointer { offset, .. } =
                        &definitions[pointer].kind
                    else {
                        panic!("exact store address")
                    };
                    let OperationKind::Select {
                        condition,
                        true_value,
                        false_value,
                    } = &definitions[offset].kind
                    else {
                        panic!("safe inactive address")
                    };
                    assert_eq!(condition, predicate);
                    assert!(matches!(
                        definitions[false_value].kind,
                        OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(0))
                    ));
                    let OperationKind::GlobalCapabilityIndex(index) = &definitions[true_value].kind
                    else {
                        panic!("bound blocked index")
                    };
                    assert_eq!(
                        index.index_space.unwrap().mapping(),
                        GlobalDisjointIndexSpaceV1::BlockedIndex1d {
                            lanes_per_block: 16,
                            elements_per_lane: 4,
                        }
                    );
                    let OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs,
                        rhs,
                    } = &definitions[predicate].kind
                    else {
                        panic!("component and bounds guard")
                    };
                    let OperationKind::Compare {
                        predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
                        rhs: component_bound,
                        ..
                    } = &definitions[lhs].kind
                    else {
                        panic!("component limit guard")
                    };
                    assert!(matches!(
                        definitions[component_bound].kind,
                        OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(4))
                    ));
                    let OperationKind::Compare {
                        predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
                        lhs: checked_index,
                        rhs: length,
                    } = &definitions[rhs].kind
                    else {
                        panic!("allocation bounds guard")
                    };
                    assert_eq!(checked_index, true_value);
                    assert!(matches!(definitions[length].kind,
                        OperationKind::SliceLength { slice } if slice == index.capability));
                    *stores.entry(index.capability).or_insert(0) += 1;
                }
                OperationKind::GuardedLoad {
                    pointer, access, ..
                } => {
                    assert!(
                        access.volatile,
                        "both independent input reads stay volatile"
                    );
                    assert!(
                        loads.insert(*pointer),
                        "distinct actual input read pointers"
                    );
                }
                _ => {}
            }
        }
        assert_eq!(stores.len(), 2);
        assert!(stores.values().all(|count| *count == 4));
        assert_eq!(loads.len(), 2);
        for kind in [BinaryOp::Divide, BinaryOp::Remainder] {
            assert!(ops.iter().any(|op| matches!(op.kind,
                OperationKind::Binary { op: actual, .. } if actual == kind)));
        }
    }
    for mutation in 0..2 {
        let mut changed = module.clone();
        let mut ops = changed
            .functions
            .iter_mut()
            .filter_map(|f| f.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|block| &mut block.operations);
        if mutation == 0 {
            let op = ops
                .find(|op| {
                    matches!(&op.kind, OperationKind::GlobalCapabilityIndex(i)
                if i.index_space.is_some())
                })
                .unwrap();
            let OperationKind::GlobalCapabilityIndex(index) = &mut op.kind else {
                unreachable!()
            };
            index.index_space = None;
        } else {
            let op = ops
                .find(|op| matches!(op.kind, OperationKind::GuardedStore { .. }))
                .unwrap();
            let OperationKind::GuardedStore {
                predicate, value, ..
            } = &mut op.kind
            else {
                unreachable!()
            };
            *value = *predicate;
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(changed).is_err(),
            "actual KIR blocked mutation {mutation}"
        );
    }
}

fn run(cpu: &'static str, name: &str) {
    let mut probe = BlockedProbe {
        cpu,
        completed: false,
    };
    if harness::run_probe(cpu, name, &mut probe) {
        assert!(
            probe.completed,
            "actual blocked source must complete all checks"
        );
    }
}

#[test]
#[ignore = "requires complete cached real AMD device/core metadata; never builds dependencies"]
fn typed_blocked_source_kir_gfx942() {
    run(
        "gfx942",
        "blocked_lowering::typed_blocked_source_kir_gfx942",
    );
}

#[test]
#[ignore = "requires complete cached real AMD device/core metadata; never builds dependencies"]
fn typed_blocked_source_kir_gfx950() {
    run(
        "gfx950",
        "blocked_lowering::typed_blocked_source_kir_gfx950",
    );
}
