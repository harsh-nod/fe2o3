//! Whole-body, nonterminal source expansion for exact primitive `From` items.
//! No block/call exclusion authority escapes the proof. The shared producer
//! selects this body before recursion and retains it through semantic import.

use super::{Helper, helper_identity, reviewed_body};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::mir::{
    BasicBlockData, Body, CastKind, Local, Operand, Rvalue, SourceInfo, SourceScope, Statement,
    StatementKind, Terminator, TerminatorKind,
};
use rustc_middle::ty::{self, Instance, TyCtxt, TypingEnv};

const MAX_DEBUG_INFO: usize = 32;
const MAX_USER_TYPE_ANNOTATIONS: usize = 8;

/// Private fields prevent a caller-authored identity or conversion selection.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReviewedCorePrimitiveCastV1<'tcx> {
    instance: Instance<'tcx>,
    helper: Helper<'tcx>,
    source_fingerprint: [u8; 16],
}

impl<'tcx> ReviewedCorePrimitiveCastV1<'tcx> {
    pub(crate) fn instance(&self) -> Instance<'tcx> {
        self.instance
    }

    /// Expands the proved function, never a panic callee or an arbitrary block.
    /// Original MIR/signature/FnAbi identities remain the shared plan's inputs.
    pub(crate) fn expand_mir(&self, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
        expand_body(tcx.instance_mir(self.instance.def), self.helper, tcx)
    }

    /// Commitment only: consumers must retain the body produced by `expand_mir`.
    pub(crate) fn expansion_fingerprint(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 16] {
        let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            "fe2o3/core-primitive-value/cast-expansion/v1".hash_stable(&mut context, &mut hasher);
            self.instance.hash_stable(&mut context, &mut hasher);
            self.source_fingerprint
                .hash_stable(&mut context, &mut hasher);
            body.hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }

    #[cfg(test)]
    pub(crate) fn revalidate(
        &self,
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        expansion: &Body<'tcx>,
    ) -> bool {
        if instance != self.instance {
            return false;
        }
        let Some(observed) = prove_core_primitive_cast_v1(tcx, instance) else {
            return false;
        };
        observed == *self
            && self.expansion_fingerprint(tcx, expansion)
                == self.expansion_fingerprint(tcx, &observed.expand_mir(tcx))
    }
}

/// Returns a producer only after exact instance, signature and closed-body proof.
/// `PartialEq::ne` deliberately remains on ordinary recursive MIR construction.
pub(crate) fn prove_core_primitive_cast_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<ReviewedCorePrimitiveCastV1<'tcx>> {
    let helper = helper_identity(tcx, instance)?;
    prove_body(tcx, instance, tcx.instance_mir(instance.def), helper)
}

fn prove_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    helper: Helper<'tcx>,
) -> Option<ReviewedCorePrimitiveCastV1<'tcx>> {
    // A fingerprint commits to a body; it cannot establish its source owner
    // or make unbounded/error-tainted metadata safe to hash and clone.
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.tainted_by_errors.is_some()
        || body.var_debug_info.len() > MAX_DEBUG_INFO
        || body.user_type_annotations.len() > MAX_USER_TYPE_ANNOTATIONS
        || matches!(helper, Helper::Ne(_))
        || !reviewed_body(tcx, body, helper)
    {
        return None;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).ok()?;
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut hasher = StableHasher::new();
        "fe2o3/core-primitive-value/source-mir-abi/v1".hash_stable(&mut context, &mut hasher);
        instance.hash_stable(&mut context, &mut hasher);
        body.hash_stable(&mut context, &mut hasher);
        signature.hash_stable(&mut context, &mut hasher);
        abi.hash_stable(&mut context, &mut hasher);
        hasher.finish()
    });
    Some(ReviewedCorePrimitiveCastV1 {
        instance,
        helper,
        source_fingerprint: fingerprint.to_le_bytes(),
    })
}

fn expand_body<'tcx>(source: &Body<'tcx>, helper: Helper<'tcx>, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
    let kind = match helper {
        Helper::U32ToU64 => CastKind::IntToInt,
        Helper::U8ToF32 => CastKind::IntToFloat,
        Helper::Ne(_) => unreachable!("only proved From conversions have a cast producer"),
    };
    let mut body = source.clone();
    let source_info = SourceInfo {
        span: source.span,
        scope: SourceScope::from_usize(0),
    };
    let cast = Statement::new(
        source_info,
        StatementKind::Assign(Box::new((
            Local::from_usize(0).into(),
            Rvalue::Cast(
                kind,
                Operand::Copy(Local::from_usize(1).into()),
                helper.output(tcx),
            ),
        ))),
    );
    body.basic_blocks_mut().raw.clear();
    body.basic_blocks_mut().push(BasicBlockData::new_stmts(
        vec![cast],
        Some(Terminator {
            source_info,
            kind: TerminatorKind::Return,
        }),
        false,
    ));
    body.local_decls.truncate(2);
    for declaration in &mut body.local_decls {
        declaration.source_info = source_info;
    }
    body.source_scopes.truncate(1);
    // Source custody is in the original body; raw temporary debug locations
    // cannot be transported into the two-local expansion without a new map.
    body.var_debug_info.clear();
    body
}

#[cfg(test)]
#[path = "production_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "production_header_tests.rs"]
mod header_tests;
