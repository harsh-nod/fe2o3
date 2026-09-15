use super::*;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_mir_body_sha256_v1,
};
use fe2o3_mir_model::{SemanticExpandedTerminatorOriginV1, SsaPlannerErrorV1};
use rustc_middle::mir::{BasicBlock, Local, Operand, TerminatorKind};
use rustc_middle::ty::{self, TypingEnv};

const MAX_FUNCTIONS: usize = 8192;
const MAX_CALLS: usize = 131072;
const MAX_INSTANCES: usize = 16384;

pub(super) fn inspect<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    mir: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    case: Case,
    out: &mut bounded::Output,
) -> SemanticFunctionIdV1 {
    assert!(mir.functions().len() <= MAX_FUNCTIONS);
    assert!(plan.direct_call_producers().len() <= MAX_CALLS);
    let expected = SemanticFunctionIdentityV1::from_sha256(harness::hex32(case.function));
    let selected = bounded::exact_index(
        mir.functions().iter().map(|f| f.identity()),
        expected,
        MAX_FUNCTIONS,
    )
    .expect("exact mixed43 identity join, never a shape lookup");
    let function = SemanticFunctionIdV1::from_index(u32::try_from(selected).unwrap());
    let roster = numerical_policy_v1::defined_source_roster_v1(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
    )
    .unwrap();
    let instance = roster
        .defined(function)
        .expect("exact original definition/body/ABI roster");
    assert_eq!(
        canonical_function_identities_v1(tcx, instance).function(),
        expected
    );
    writeln!(out, "REPORTED_IDENTITY_JOIN function={function:?} identity={expected:?} def_id={:?} instance={instance:?} live_path={}",
        instance.def_id(), tcx.def_path_str(instance.def_id())).unwrap();
    eprintln!("{}", out.as_str());
    assert!(matches!(instance.def, ty::InstanceKind::Item(_)));
    assert!(
        trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            instance.def_id(),
            BIND
        ),
        "reported function is not the proposed nominal helper; diagnose, do not substitute"
    );
    assert!(
        trusted_device_items::authenticate_reviewed_safe_external_helper_v1(tcx, instance.def_id())
            .expect("live reviewed provider source closure")
    );
    assert!(
        !trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            instance.def_id(),
            CONVERSION,
        ),
        "the converter is not an alias for the bind definition"
    );
    let mut substituted = *expected.as_bytes();
    substituted[0] ^= 1;
    assert_eq!(
        bounded::exact_index(
            mir.functions().iter().map(|f| f.identity()),
            SemanticFunctionIdentityV1::from_sha256(substituted),
            MAX_FUNCTIONS
        ),
        Err("reported identity absent; no nominal or shape fallback")
    );
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .unwrap();
    charge(&mut work, plan.direct_call_producers().len()).unwrap();
    let mut replay = source_body_v1::Replay::new(tcx, plan, &mut work).unwrap();
    let mapping = roster
        .reconstructed_body(function, &mut replay, &mut work)
        .unwrap();
    let source = &mir.functions()[selected];
    let raw = plan.function_mir(function).unwrap();
    assert!(std::ptr::eq(raw, tcx.instance_mir(instance.def)));
    // Shape is reported after the exact nominal join, not used to select authority.
    assert!(raw.arg_count <= 16 && raw.local_decls.len() <= 64 && raw.basic_blocks.len() <= 64);
    let raw_statements: usize = raw.basic_blocks.iter().map(|b| b.statements.len()).sum();
    assert!(raw_statements <= 256);
    let abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
        )
        .unwrap();
    assert!(abi.args.len() <= 16);
    let signature = tcx
        .fn_sig(instance.def_id())
        .instantiate(tcx, instance.args);
    let ret = mapping.local(0).unwrap();
    assert_eq!(
        source.locals()[ret.index() as usize].role(),
        SemanticLocalRoleV1::Return
    );
    writeln!(out, "DEFINITION function={function:?} identity={expected:?} def_id={:?} instance={instance:?}\n  provider={} source={}\n  signature={signature:?}\n  raw_body_hash={:02x?}\n  canonical_item={:?} canonical_mono={:?}\n  canonical_abi={:?}\n  raw_return_mode={:?} can_unwind={}",
        instance.def_id(), tcx.def_path_str(instance.def_id()),
        tcx.sess.source_map().span_to_diagnostic_string(tcx.def_span(instance.def_id())),
        rustc_mir_body_sha256_v1(tcx, instance), source.item_definition_identity(),
        source.monomorphization_identity(), source.abi(), abi.ret.mode, abi.can_unwind).unwrap();
    for (index, arg) in abi.args.iter().enumerate() {
        writeln!(out, "RAW_ABI_ARGUMENT index={index} mode={:?}", arg.mode).unwrap();
    }
    for (local, decl) in raw.local_decls.iter_enumerated() {
        let ty = body::normalize(tcx, instance, decl.ty).unwrap();
        let [canonical_ty] = roster.types([ty]).unwrap();
        let canonical = mapping.local(local.as_u32()).unwrap();
        assert_eq!(
            source.locals()[canonical.index() as usize].ty(),
            canonical_ty
        );
        writeln!(out, "LOCAL raw={local:?} canonical={canonical:?} role={:?} type={ty:?} semantic_type={canonical_ty:?} identity={:?}",
            source.locals()[canonical.index() as usize].role(), mir.types()[canonical_ty.index() as usize].identity()).unwrap();
    }
    for (block, data) in raw.basic_blocks.iter_enumerated() {
        writeln!(
            out,
            "ORIGINAL_BODY raw={block:?} canonical={:?} data={data:?}",
            mapping.block(block.as_u32()).unwrap()
        )
        .unwrap();
    }

    // Retain every incoming site. Sorting canonical identities is deterministic;
    // neither a last-caller overwrite nor an expanded-instance singleton is used.
    let mut sites = Vec::with_capacity(bounded::MAX_SITES);
    for edge in plan
        .direct_call_producers()
        .iter()
        .filter(|e| e.callee == function)
    {
        charge(&mut work, bounded::MAX_SITES).unwrap();
        let caller_mapping = roster
            .reconstructed_body(edge.caller, &mut replay, &mut work)
            .unwrap();
        let block = caller_mapping.block(edge.block).unwrap();
        bounded::insert_site(&mut sites, (edge.caller, block, edge.block)).unwrap();
    }
    assert!(
        !sites.is_empty(),
        "complete original incoming-site roster must be nonempty"
    );
    for (caller, block, raw_block) in &sites {
        let caller_instance = plan.function_producers()[caller.index() as usize].instance;
        let original = plan.function_mir(*caller).unwrap();
        let mapped = roster
            .reconstructed_body(*caller, &mut replay, &mut work)
            .unwrap();
        let original_call = original.basic_blocks[BasicBlock::from_u32(*raw_block)].terminator();
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(target),
            unwind,
            ..
        } = &original_call.kind
        else {
            panic!("incoming recipe must be the original normal-edge direct call");
        };
        let normalized =
            body::normalize(tcx, caller_instance, func.ty(&original.local_decls, tcx)).unwrap();
        assert!(matches!(func, Operand::Constant(_)));
        let ty::FnDef(def_id, generic_args) = *normalized.kind() else {
            panic!("exact FnDef receiver");
        };
        let resolved = ty::Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            def_id,
            tcx.erase_and_anonymize_regions(generic_args),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved, instance);
        assert_eq!(args.len(), raw.arg_count);
        assert!(args.len() <= 16);
        let caller_source = &mir.functions()[caller.index() as usize];
        let SemanticTerminatorKindV1::Call(call) = caller_source.blocks()[block.index() as usize]
            .terminator()
            .kind()
        else {
            panic!("reconstructed source call");
        };
        assert_eq!(
            mir.callables()[call.callee().index() as usize],
            SemanticCallableDeclV1::defined(function)
        );
        assert_eq!(call.arguments().len(), args.len());
        let normal = call.destination().expect("normal return destination");
        assert!(destination.projection.is_empty() && normal.place().projections().is_empty());
        assert_eq!(
            normal.place().local(),
            mapped.local(destination.local.as_u32()).unwrap()
        );
        assert_eq!(normal.edge().role(), SemanticEdgeRoleV1::CallReturn);
        assert_eq!(
            normal.edge().target(),
            mapped.block(target.as_u32()).unwrap()
        );
        assert!(matches!(
            unwind,
            rustc_middle::mir::UnwindAction::Unreachable
        ));
        assert_eq!(call.unwind(), SemanticUnwindActionV1::Unreachable);
        writeln!(out, "INCOMING caller={caller:?} identity={:?} instance={caller_instance:?} raw_bb={raw_block} canonical_bb={block:?}\n  raw_call={:?}\n  canonical_call={call:?}\n  span={}",
            caller_source.identity(), original_call.kind,
            tcx.sess.source_map().span_to_diagnostic_string(original_call.source_info.span)).unwrap();
        for (index, arg) in args.iter().enumerate() {
            let ty = body::normalize(
                tcx,
                caller_instance,
                arg.node.ty(&original.local_decls, tcx),
            )
            .unwrap();
            let parameter = body::normalize(
                tcx,
                instance,
                raw.local_decls[Local::from_usize(index + 1)].ty,
            )
            .unwrap();
            assert_eq!(ty, parameter, "exact live typed reference argument edge");
            let [canonical_ty] = roster.types([ty]).unwrap();
            writeln!(out, "INCOMING_ARGUMENT index={index} operand={:?} type={ty:?} semantic_type={canonical_ty:?}", arg.node).unwrap();
        }
    }

    let mut expanded_sites = BTreeSet::new();
    let mut selected_instances = 0usize;
    let mut transfers = 0usize;
    for view in expansion.roots() {
        assert!(view.instances().len() <= MAX_INSTANCES);
        charge(&mut work, view.instances().len()).unwrap();
        for (index, frame) in view
            .instances()
            .iter()
            .enumerate()
            .filter(|(_, f)| f.function() == function)
        {
            selected_instances += 1;
            assert!(selected_instances <= bounded::MAX_SITES);
            assert_eq!(frame.function_identity(), expected);
            let parent_id = frame
                .parent()
                .expect("bind must be an incoming call, never a root");
            let parent = &view.instances()[parent_id.index() as usize];
            let call_block = frame.call_block().unwrap();
            let mut exact_site = sites
                .iter()
                .filter(|(caller, block, _)| *caller == parent.function() && *block == call_block);
            let site = *exact_site
                .next()
                .expect("expanded occurrence must join a checked original incoming site");
            assert!(exact_site.next().is_none());
            expanded_sites.insert(site);
            writeln!(out, "OCCURRENCE root={:?} view={:02x?} instance={index} parent={parent_id:?} original_site={site:?} return_local={ret:?}",
                view.root(), view.identity()).unwrap();
        }
        // Scan once per root rather than once per selected frame. Bound all work,
        // including unmatched origins and output generated for return receipts.
        for (block, origin) in view.block_origins().iter().enumerate() {
            charge(&mut work, 1 + origin.statements().len()).unwrap();
            for (statement, tag) in origin.statements().iter().enumerate() {
                let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = tag else {
                    continue;
                };
                let frame = &view.instances()[callee.index() as usize];
                if frame.function() != function {
                    continue;
                }
                transfers += 1;
                assert!(transfers <= bounded::MAX_SITES);
                assert_eq!(origin.instance(), *callee);
                assert_eq!(origin.function(), function);
                assert_eq!(
                    origin.terminator(),
                    SemanticExpandedTerminatorOriginV1::CallReturn { callee: *callee }
                );
                let raw_statement = &view.body().blocks()[block].statements()[statement];
                let SemanticStatementKindV1::Assign(assignment) = raw_statement.kind() else {
                    panic!("ReturnTransfer is an exact assignment");
                };
                let place = assignment.destination();
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(return_place)) =
                    assignment.value().kind()
                else {
                    panic!("ReturnTransfer consumes the real return local");
                };
                assert!(return_place.projections().is_empty() && place.projections().is_empty());
                let return_origin = view.local_origins()[return_place.local().index() as usize];
                assert_eq!(return_origin.instance(), *callee);
                assert_eq!(return_origin.function(), function);
                assert_eq!(return_origin.local(), ret);
                let parent = &view.instances()[frame.parent().unwrap().index() as usize];
                let SemanticTerminatorKindV1::Call(call) = mir.functions()
                    [parent.function().index() as usize]
                    .blocks()[frame.call_block().unwrap().index() as usize]
                    .terminator()
                    .kind()
                else {
                    unreachable!()
                };
                let target_origin = view.local_origins()[place.local().index() as usize];
                assert_eq!(target_origin.instance(), frame.parent().unwrap());
                assert_eq!(target_origin.function(), parent.function());
                assert_eq!(
                    target_origin.local(),
                    call.destination().unwrap().place().local()
                );
                writeln!(out, "RETURN_RECEIPT root={:?} instance={callee:?} expanded_bb={block} statement={statement} source_block={:?} source_return={return_origin:?} destination={target_origin:?} retained={raw_statement:?}", view.root(), origin.block()).unwrap();
            }
        }
    }
    assert_eq!(
        expanded_sites.len(),
        sites.len(),
        "no original incoming site may disappear"
    );
    assert!(sites.iter().all(|site| expanded_sites.contains(site)));
    assert!(selected_instances > 0);
    assert_eq!(
        transfers, selected_instances,
        "one observed return receipt per empty-return instance"
    );
    writeln!(out, "ROSTER incoming={} occurrences={selected_instances} return_receipts={transfers} remaining_work={work} complete=true; lifetime/convergence/allocation custody NOT PROVED", sites.len()).unwrap();
    function
}

pub(super) fn same_return_error(
    error: &fe2o3_pliron::ProductionSemanticSsaErrorV1,
    selected: SemanticFunctionIdV1,
) -> bool {
    use fe2o3_pliron::ProductionSemanticSsaErrorV1 as E;
    matches!(error, E::ExpandedExecution {
        source_block: Some((instance, function, _)),
        source_statement: Some(SemanticExpandedStatementOriginV1::ReturnTransfer { callee }),
        source_local: Some(local), error, ..
    } if *function == selected && instance == callee && local.instance() == *callee && local.function() == selected
        && matches!(error.as_ref(), E::Planner { error: SsaPlannerErrorV1::UndefinedAtUse { .. }, .. }))
}
