//! The existing move-only Context owner carries this live-source seal. It has
//! no public constructor and is never decoded from the canonical phase record.
use super::*;
use execution_source::{CheckedSource,Protocols};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

#[path = "emission_handoff.rs"]
mod emission_handoff;

#[cfg(test)]
#[path = "production_tests.rs"]
pub(super) mod tests;

pub(in crate::collector::production_importer_v1) enum SourceStatus {
    Checked(SourceSeal),
    /// Canonical import may retain a source-valid family outside the first
    /// executable subset. The production SSA transition must fail closed here.
    Unsupported(&'static str),
}
pub(in crate::collector::production_importer_v1) struct SourceSeal {
    semantic:[u8;32],
    root:SemanticFunctionIdV1,
    protocols:Vec<source_protocol::Protocol>,
    identity:[u8;32],
    remaining_work:usize,
}
impl SourceSeal {
    fn rebind<'a>(&'a self,owner:&'a ProductionSemanticSsaOwnerV1)->PhaseResult<ssa_protocol::CheckedSsa<'a,'a>> {
        if owner.source_semantic().semantic_sha256().as_bytes()!=&self.semantic
            || owner.source_semantic().roots()!=[self.root] || self.protocols.is_empty() {
            return Err(rejected("phase execution lost its exact live final-source seal"));
        }
        let view=owner.execution_view_for_root(self.root).ok_or_else(||rejected("phase sealed root has no expanded view"))?;
        let source=CheckedSource {semantic:owner.source_semantic(),protocols:Protocols::Borrowed(&self.protocols),
            identity:self.identity,remaining_work:self.remaining_work};
        source.bind_expansion(owner.execution_expansion(),view)?.bind_ssa(owner)
    }
}

pub(in crate::collector::production_importer_v1) fn attach<'tcx>(tcx:TyCtxt<'tcx>,
    plan:&ProductionSemanticPreflightPlanV1<'tcx>,contexts:&mut AuthenticatedProductionKernelContextsV1,
    semantic:&AdmittedInertSemanticMirV1)->PhaseResult<()> {
    if contexts.reusable_phase_source.is_some() {return Err(rejected("phase source seal was already attached"));}
    if !semantic.functions().iter().any(|f|matches!(f.defined_capability_contract(),Some(SemanticDefinedCapabilityContractV1::ReusablePhase(_)))) {return Ok(());}
    // This runs after the final common footer and existing Context entry hook.
    // Source-invalid metadata already failed canonical live carriage replay.
    let observed=CheckedSource::observe(tcx,plan,contexts,semantic);
    let status=match observed {
        Ok(source)=> {
            let [root]=semantic.roots() else {return Err(rejected("phase executable source requires one complete root partition"));};
            let Protocols::Owned(protocols)=source.protocols else {return Err(rejected("phase source sealing requires a new live observation"));};
            SourceStatus::Checked(SourceSeal {semantic:*semantic.semantic_sha256().as_bytes(),root:*root,
                protocols,identity:source.identity,remaining_work:source.remaining_work})
        }
        Err(ProductionSemanticImportErrorV1::KernelContextBinding(detail))=>SourceStatus::Unsupported(detail),
        Err(error)=>return Err(error),
    };
    contexts.reusable_phase_source=Some(status);
    Ok(())
}

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn validate_reusable_phase_source_ssa(
        contexts:Option<&Self>,owner:&ProductionSemanticSsaOwnerV1,
    )->PhaseResult<()> {
        let needed=owner.source_semantic().functions().iter().any(|f|
            matches!(f.defined_capability_contract(),Some(SemanticDefinedCapabilityContractV1::ReusablePhase(_))));
        if !needed {
            if contexts.is_some_and(|c|c.reusable_phase_source.is_some()) {
                return Err(rejected("phase source seal attached to a different semantic owner"));
            }
            return Ok(());
        }
        match contexts.and_then(|c|c.reusable_phase_source.as_ref()) {
            Some(SourceStatus::Checked(seal))=> {let _checked=seal.rebind(owner)?;Ok(())}
            Some(SourceStatus::Unsupported(detail))=>Err(rejected(detail)),
            None=>Err(rejected("phase SSA requires its complete live source seal")),
        }
    }
}
