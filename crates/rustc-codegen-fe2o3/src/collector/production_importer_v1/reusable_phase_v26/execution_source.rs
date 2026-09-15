//! Live source boundary for the first executable phase slice. Construct only
//! after source carriage replay; this is not an SSA value or a loan receipt.
use super::*;
use source_calls::{reserve, spend};

/// Non-Clone and private. The next adapter must bind these source sites to the
/// exact checked expansion and SSA/borrow owner before using the lifecycle core.
pub(super) struct CheckedSource<'mir> {
    pub(super) semantic: &'mir AdmittedInertSemanticMirV1,
    pub(super) protocols: Protocols<'mir>,
    pub(super) identity: [u8; 32],
    pub(super) remaining_work: usize,
}

/// The live observer owns the roster. After the semantic owner moves into SSA,
/// the private source seal lends that same roster without cloning its tables.
pub(super) enum Protocols<'a> {
    Owned(Vec<source_protocol::Protocol>),
    Borrowed(&'a [source_protocol::Protocol]),
}
impl std::ops::Deref for Protocols<'_> {
    type Target=[source_protocol::Protocol];
    fn deref(&self)->&Self::Target {
        match self {Self::Owned(values)=>values,Self::Borrowed(values)=>values}
    }
}

impl<'mir> CheckedSource<'mir> {
    pub(super) fn observe<'tcx>(
        tcx: TyCtxt<'tcx>,
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
        contexts: &AuthenticatedProductionKernelContextsV1,
        mir: &'mir AdmittedInertSemanticMirV1,
    ) -> PhaseResult<Self> {
        let fresh = super::observe(
            tcx,
            plan,
            mir.types(),
            mir.functions(),
            mir.callables(),
            contexts,
        )?;
        let mut work = fresh.remaining_work;
        let mut declared = 0;
        for function in mir.functions() {
            spend(&mut work, 1)?;
            if let Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) =
                function.defined_capability_contract()
            {
                if fresh.records.get(declared) != Some(record) {
                    return Err(rejected("phase execution source carriage mismatch"));
                }
                declared += 1;
            }
        }
        if declared == 0 || declared != fresh.records.len() {
            return Err(rejected(
                "phase execution requires the complete defined source roster",
            ));
        }
        let mut definitions = reserve(declared, &mut work)?;
        for record in &fresh.records {
            spend(&mut work, 1)?;
            let producer = &plan.function_producers()[record.function().index() as usize];
            let role = definitions::classify(tcx, producer.instance, &mut work)
                .map_err(|_| rejected("phase execution original definition work"))?
                .ok_or_else(|| rejected("phase execution original definition identity"))?;
            let definition =
                definitions::Definition::observe(tcx, producer.instance, role, &mut work)
                    .map_err(|_| rejected("phase execution original definition, body or ABI"))?;
            definitions.push((record.function(), definition));
        }
        let roster = numerical_policy_v1::defined_source_roster_v1(
            tcx,
            plan,
            mir.types(),
            mir.functions(),
            mir.callables(),
        )?;
        let mut replay = source_body_v1::Replay::new(tcx, plan, &mut work)?;
        let calls = source_calls::observe(
            tcx,
            plan,
            mir.functions(),
            mir.callables(),
            &definitions,
            &roster,
            &mut replay,
            &mut work,
        )?;
        let mut receipts = reserve(calls.len(), &mut work)?;
        for call in &calls {
            spend(&mut work, definitions.len())?;
            let definition = definitions
                .iter()
                .find(|(id, _)| *id == call.callee)
                .ok_or_else(|| rejected("phase execution callee definition absent"))?;
            receipts.push(hir_calls::observe(
                tcx,
                call,
                &definition.1,
                &definitions,
                &mut work,
            )?);
        }
        let protocols = source_protocol::check(
            tcx,
            plan,
            mir.types(),
            mir.functions(),
            mir.callables(),
            contexts,
            &definitions,
            &calls,
            &receipts,
            &roster,
            &mut replay,
            &mut work,
        )?;
        let mut digest =
            SemanticIdentityDigestV1::new(b"fe2o3/production/reusable-phase/source-owner/v26");
        source_protocol::commit(tcx, &protocols, &mut digest, &mut work)?;
        for record in &fresh.records {
            spend(&mut work, 2)?;
            digest.field(record.source_binding());
            digest.field(record.body_identity());
        }
        Ok(Self {
            semantic: mir,
            protocols: Protocols::Owned(protocols),
            identity: digest.finish(),
            remaining_work: work,
        })
    }
}
