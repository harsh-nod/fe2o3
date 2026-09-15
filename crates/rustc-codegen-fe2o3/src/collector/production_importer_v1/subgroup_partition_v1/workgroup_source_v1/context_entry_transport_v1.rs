use super::context_entry_source_v1::ContextEntrySourceV1;
use crate::collector::production_importer_v1::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use fe2o3_lower_mir_kernel::ProductionKernelContextEntryTransferV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticConstantValueV1, SemanticLocalIdV1, SemanticOperandV1,
    SemanticTerminatorKindV1,
};

pub(in crate::collector::production_importer_v1) fn attach_context_entry_transfers_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    semantic: &AdmittedInertSemanticMirV1,
    contexts: &mut AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let reject = ProductionSemanticImportErrorV1::KernelContextBinding;
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .map_err(|_| reject("Context source work ceiling does not fit usize"))?;
    for root in &mut contexts.roots {
        let producer = plan
            .function_producers()
            .get(root.selected_root.index() as usize)
            .ok_or_else(|| reject("Context entry root producer is missing"))?;
        let source_body = plan
            .body_producers()
            .get(root.selected_root.index() as usize)
            .ok_or_else(|| reject("Context entry root body producer is missing"))?;
        let helpers = plan
            .function_producers()
            .iter()
            .enumerate()
            .filter(|(_, helper)| {
                helper.identities.function().as_bytes() == &root.logical_helper_identity
            })
            .collect::<Vec<_>>();
        let [(helper_index, helper)] = helpers.as_slice() else {
            return Err(reject(
                "Context entry lost its authenticated logical helper",
            ));
        };
        let helper_id = SemanticFunctionIdV1::from_index(*helper_index as u32);
        let source = semantic
            .functions()
            .get(root.selected_root.index() as usize)
            .ok_or_else(|| reject("Context entry source root is missing"))?;
        if source.identity().as_bytes() != &root.root_function_identity {
            return Err(reject("Context entry source root identity changed"));
        }
        let mut erased = Vec::new();
        for (block, data) in source.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
                continue;
            };
            if !matches!(semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == helper_id)
            {
                continue;
            }
            if matches!(call.arguments().first(), Some(SemanticOperandV1::Constant(c))
                if matches!(c.value(), SemanticConstantValueV1::ZeroSized))
            {
                erased.push(block);
            }
        }
        if erased.is_empty() {
            continue;
        }
        if erased.len() != 1 {
            return Err(reject("Context entry has ambiguous erased helper calls"));
        }
        let issuers = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|recipe| {
                recipe.caller == root.selected_root
                    && recipe.expansion == ProductionTerminalExpansionV1::KernelContextIssue
            })
            .collect::<Vec<_>>();
        let [issuer] = issuers.as_slice() else {
            return Err(reject("Context entry lost its exact trusted issuance"));
        };
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(issuer.instance.def_id())
                .instantiate(tcx, issuer.instance.args),
        );
        let context_ty = signature.output();
        let checked = ContextEntrySourceV1::check(
            tcx,
            producer.instance,
            helper.instance,
            issuer.instance,
            context_ty,
            0,
            &mut work,
        )
        .map_err(reject)?;
        checked.replay(tcx, &mut work).map_err(reject)?;
        let (issued, local, called, argument) = checked.coordinates();
        let map_block = |rustc_block| {
            source_body
                .blocks
                .iter()
                .position(|block| block.rustc_block == rustc_block)
                .map(|index| SemanticBlockIdV1::from_index(index as u32))
                .ok_or_else(|| reject("Context source block lost its canonical binding"))
        };
        let issued_block = map_block(issued.as_u32())?;
        let call_block = map_block(called.as_u32())?;
        let issued_local = source_body
            .locals
            .iter()
            .position(|item| item.rustc_local == local.as_u32())
            .map(|index| SemanticLocalIdV1::from_index(index as u32))
            .ok_or_else(|| reject("Context source local lost its canonical binding"))?;
        let context = source.locals()[issued_local.index() as usize].ty();
        let context_identity = semantic.types()[context.index() as usize].identity();
        if call_block.index() as usize != erased[0] || issued.as_u32() != issuer.block {
            return Err(reject(
                "Context source occurrence and canonical caller differ",
            ));
        }
        let issuer_identity = canonical_function_identities_v1(tcx, issuer.instance).function();
        let (binding, use_site) = checked.source_nodes();
        let mut digest =
            SemanticIdentityDigestV1::new(b"fe2o3/production/kernel-context/source-entry-edge/v1");
        digest.field(semantic.semantic_sha256().as_bytes());
        digest.field(source.identity().as_bytes());
        digest.field(helper.identities.function().as_bytes());
        digest.field(issuer_identity.as_bytes());
        digest.field(context_identity.as_bytes());
        digest.field(&binding.to_le_bytes());
        digest.field(&use_site.to_le_bytes());
        digest.field(&issued.as_u32().to_le_bytes());
        digest.field(&local.as_u32().to_le_bytes());
        digest.field(&called.as_u32().to_le_bytes());
        digest.field(&(argument as u32).to_le_bytes());
        root.entry_transfer = Some(ProductionKernelContextEntryTransferV1::new(
            *semantic.semantic_sha256().as_bytes(),
            source.identity(),
            helper.identities.function(),
            issuer_identity,
            context_identity,
            issued_block,
            issued_local,
            call_block,
            argument as u32,
            digest.finish(),
        ));
    }
    contexts.custody_identity = kernel_context_custody_identity_v1(
        contexts.frontend_unit_identity,
        contexts.target_brand_identity,
        &contexts.expected_roots,
        &contexts.roots,
    );
    Ok(())
}
