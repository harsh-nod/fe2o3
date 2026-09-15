use super::*;
type Result<T> = std::result::Result<T, SemanticMirErrorV1>;
fn ty(w: &mut CanonicalWriterV1, t: SemanticTypeIdV1) -> Result<()> {
    w.u32(t.index())
}
fn reference(w: &mut CanonicalWriterV1, r: SemanticPhaseReferenceV1) -> Result<()> {
    ty(w, r.reference)?;
    ty(w, r.pointee)?;
    w.u8(match r.kind {
        SemanticPhaseReferenceKindV1::Shared => 0,
        SemanticPhaseReferenceKindV1::Unique => 1,
    })
}
fn brands(w: &mut CanonicalWriterV1, b: SemanticPhaseBrandsV1) -> Result<()> {
    for id in [
        b.root_brand,
        b.outer_workgroup_brand,
        b.phase_brand,
        b.dynamic_epoch,
    ] {
        ty(w, id)?;
    }
    Ok(())
}
fn callable(w: &mut CanonicalWriterV1, c: SemanticPhaseCallableV1) -> Result<()> {
    w.u32(c.callable.index())?;
    w.raw(c.identity.as_bytes())?;
    w.raw(c.abi.as_bytes())
}
fn relay(w: &mut CanonicalWriterV1, r: SemanticPhaseCompletionRelayV1) -> Result<()> {
    w.u32(r.closure_function.index())?;
    w.raw(&r.closure_body)?;
    callable(w, r.finish)?;
    for index in [
        r.finish_call_block.index(),
        r.finish_normal_target.index(),
        r.closure_pack_block.index(),
        r.closure_pack_statement,
        r.closure_return_block.index(),
    ] {
        w.u32(index)?;
    }
    match r.completion_field {
        SemanticPhaseCompletionFieldV1::RetainedFinishResult => w.u8(0)?,
        SemanticPhaseCompletionFieldV1::ErasedZstConstant { canonical_operand } => {
            w.u8(1)?;
            w.raw(&canonical_operand)?;
        }
    }
    match r.wrapper_drop {
        SemanticPhaseCompletionDropV1::RetainedPairField => w.u8(0),
        SemanticPhaseCompletionDropV1::ErasedZstConstant { canonical_operand } => {
            w.u8(1)?;
            w.raw(&canonical_operand)
        }
    }
}

impl SemanticDefinedReusablePhaseV1 {
    pub(in crate::semantic_mir_v1) fn encode_payload(
        self,
        w: &mut CanonicalWriterV1,
    ) -> Result<()> {
        w.u32(self.function.index())?;
        w.raw(self.source_identity.as_bytes())?;
        w.raw(self.abi_identity.as_bytes())?;
        w.raw(&self.body_identity)?;
        encode_kernel_capability_provenance(w, self.provenance)?;
        w.u32(self.incoming.count)?;
        w.raw(&self.incoming.digest)?;
        w.raw(&self.source_binding)?;
        use SemanticDefinedReusablePhaseRecipeV1 as R;
        match self.recipe {
            R::OwnerConvert {
                workgroup,
                owner,
                root_brand,
                outer_workgroup_brand,
                input_epoch,
            } => {
                w.u8(0)?;
                for id in [
                    workgroup,
                    owner,
                    root_brand,
                    outer_workgroup_brand,
                    input_epoch,
                ] {
                    ty(w, id)?;
                }
                Ok(())
            }
            R::Issue {
                owner_reference,
                owner,
                phase_workgroup,
                brands: b,
            } => {
                w.u8(1)?;
                reference(w, owner_reference)?;
                ty(w, owner)?;
                ty(w, phase_workgroup)?;
                brands(w, b)
            }
            R::WithPhase {
                owner_reference,
                owner,
                closure,
                call_tuple,
                phase_workgroup,
                completion,
                result_pair,
                result,
                drop_result,
                brands: b,
                issue,
                invoke,
                drop_completion,
                issue_block,
                invoke_block,
                drop_block,
                relay: r,
            } => {
                w.u8(2)?;
                reference(w, owner_reference)?;
                for id in [
                    owner,
                    closure,
                    call_tuple,
                    phase_workgroup,
                    completion,
                    result_pair,
                    result,
                    drop_result,
                ] {
                    ty(w, id)?;
                }
                brands(w, b)?;
                for c in [issue, invoke, drop_completion] {
                    callable(w, c)?;
                }
                for id in [issue_block, invoke_block, drop_block] {
                    w.u32(id.index())?;
                }
                relay(w, r)
            }
            R::Bind {
                phase_reference,
                storage_reference,
                phase_workgroup,
                reusable_storage,
                phase_lds,
                element,
                uninitialized_marker,
                storage_marker,
                thread_marker,
                brands: b,
                elements,
            } => {
                w.u8(3)?;
                reference(w, phase_reference)?;
                reference(w, storage_reference)?;
                for id in [
                    phase_workgroup,
                    reusable_storage,
                    phase_lds,
                    element,
                    uninitialized_marker,
                    storage_marker,
                    thread_marker,
                ] {
                    ty(w, id)?;
                }
                brands(w, b)?;
                w.u64(elements)
            }
            R::Finish {
                workgroup_before_barrier,
                workgroup_after_barrier,
                completion,
                brands: b,
                input_epoch,
                advanced_epoch,
                barrier,
                barrier_block,
                return_block,
            } => {
                w.u8(4)?;
                for id in [
                    workgroup_before_barrier,
                    workgroup_after_barrier,
                    completion,
                ] {
                    ty(w, id)?;
                }
                brands(w, b)?;
                ty(w, input_epoch)?;
                ty(w, advanced_epoch)?;
                callable(w, barrier)?;
                w.u32(barrier_block.index())?;
                w.u32(return_block.index())
            }
        }
    }
}
