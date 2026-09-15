//! Logical types referenced by authenticated terminals but erased from their ABI layout.
use super::*;
use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1;
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(super) fn retain<'tcx>(
    preflight: &mut BodyPreflightV1<'_, 'tcx>,
    terminal: &RetainedSemanticTerminalProducerV1<'tcx>,
    site: RejectionSiteV1,
) -> Result<(), PendingRejectionV1> {
    if terminal.expansion
        != ProductionTerminalExpansionV1::Execution(ProductionExecutionTerminalV1::LdsAllocate)
    {
        return Ok(());
    }
    preflight.charge(SemanticMirResourceV1::ValidationWork, 16)?;
    let element = allocation_element(preflight.tcx, terminal.instance, terminal.abi.source_output)
        .ok_or_else(|| {
            reject(
                "authenticated LDS allocation logical element dependency",
                site,
            )
        })?;
    // This uses the existing normalization, identity collision, layout and work limits.
    // It does not traverse the handle's unrelated phantom brands or grant authority.
    preflight.inspect_type(element, site)
}

fn allocation_element<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    output: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    if trusted_device_items::classify(tcx, instance.def_id())
        != Some(TrustedDeviceItem::ExecutionLdsAllocate)
    {
        return None;
    }
    let TyKind::Adt(definition, arguments) = *output.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::ExecutionWorkgroupLds)
        || arguments.len() != 6
    {
        return None;
    }
    let mut types = arguments.types();
    let element = types.next()?;
    let _state = types.next()?;
    let _brand = types.next()?;
    let _epoch = types.next()?;
    if types.next().is_some() {
        return None;
    }
    let mut consts = arguments.consts();
    let elements = consts.next()?.try_to_target_usize(tcx)?;
    (consts.next().is_none() && elements != 0).then_some(element)
}

#[cfg(test)]
pub(crate) fn assert_live_roster<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
) {
    for terminal in plan.terminal_producers() {
        if terminal.expansion
            != ProductionTerminalExpansionV1::Execution(ProductionExecutionTerminalV1::LdsAllocate)
        {
            continue;
        }
        let element = allocation_element(tcx, terminal.instance, terminal.abi.source_output)
            .expect("exact reviewed allocation output retains its logical element");
        let identity = rustc_type_identity_v1(tcx, element);
        let mut found = plan
            .type_producers()
            .iter()
            .filter(|p| p.identity == identity);
        let producer = found
            .next()
            .expect("logical LDS element is in the live preflight roster");
        assert_eq!(producer.ty, element);
        assert_eq!(producer.layout.ty, element);
        assert!(
            found.next().is_none(),
            "logical LDS element has one exact identity"
        );
        assert_eq!(
            allocation_element(tcx, terminal.instance, tcx.types.u8),
            None
        );
        let root = plan.roots()[0];
        assert_eq!(
            allocation_element(
                tcx,
                plan.function_producers()[root.index() as usize].instance,
                terminal.abi.source_output
            ),
            None
        );
    }
}
