//! Preserve rustc's mutable receiver and explicitly reborrow it for an Fn body.

use super::*;
use crate::collector::closure_once_shim_v1::authenticate_closure_once_shim_body_v1;
use crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1;
use rustc_middle::ty::{ClosureKind, InstanceKind};

#[derive(Clone, Copy)]
pub(super) struct ClosureOnceReborrowV1<'tcx> {
    callee: Instance<'tcx>,
    mutable_local: SemanticLocalIdV1,
    mutable_type: SemanticTypeIdV1,
    environment_type: SemanticTypeIdV1,
    shared_local: SemanticLocalIdV1,
    shared_type: SemanticTypeIdV1,
}

impl<'tcx> BodyProducerV1<'_, '_, 'tcx> {
    pub(super) fn normalize_closure_once_receiver_v1(
        &mut self,
        input: &ProductionSemanticBodyInputV1<'_, 'tcx>,
        locals: &mut Vec<SemanticLocalDeclV1>,
    ) -> Result<Option<ClosureOnceReborrowV1<'tcx>>, ProductionSemanticBodyErrorV1> {
        if !matches!(self.instance.def, InstanceKind::ClosureOnceShim { .. }) {
            return Ok(None);
        }
        self.work()?;
        if !authenticate_closure_once_shim_body_v1(self.tcx, self.instance, self.body)
            || self.inserted_local_order.is_some()
        {
            return Err(table("authenticated closure once adapter"));
        }
        let environment = self.instance.args[0]
            .as_type()
            .ok_or_else(|| table("closure once receiver type"))?;
        let TyKind::Closure(definition, arguments) = *environment.kind() else {
            return Err(table("closure once receiver type"));
        };
        // FnMut bodies already take exactly the adapter's mutable receiver.
        if arguments.as_closure().kind() == ClosureKind::FnMut {
            return Ok(None);
        }
        let callee = Instance::new_raw(definition, arguments);
        let binding = self.direct_calls_by_raw[START_BLOCK.index()]
            .ok_or_else(|| table("closure once forwarding call"))?;
        if binding.expected_callee != callee {
            return Err(table("closure once forwarding callee"));
        }
        let shared = Ty::new_imm_ref(self.tcx, self.tcx.lifetimes.re_erased, environment);
        let shared_type = self.type_id(shared, None, None)?;
        let mutable_type = self.type_id(
            self.body.local_decls[rustc_middle::mir::Local::from_usize(3)].ty,
            None,
            None,
        )?;
        let environment_type = self.type_id(environment, None, None)?;
        let mut identity =
            SemanticIdentityDigestV1::new(b"fe2o3/semantic-mir/closure-once-reborrow/v1");
        identity.field(input.identities.identity.as_bytes());
        identity.field(input.abi.identity().as_bytes());
        identity.field(self.locals_by_raw[3].identity.as_bytes());
        let identity = SemanticLocalIdentityV1::from_sha256(identity.finish());
        self.owner.charge(SemanticMirResourceV1::Locals, 1)?;
        let order = rust_call_v1::InsertedLocalOrderV1::new(locals, identity, self.owner)?;
        locals
            .try_reserve(1)
            .map_err(|_| allocation(SemanticMirResourceV1::Locals))?;
        locals.insert(
            order.inserted_local.index() as usize,
            SemanticLocalDeclV1::new(
                identity,
                shared_type,
                SemanticLocalRoleV1::Temporary,
                self.blocks_by_raw[START_BLOCK.index()].terminator_source,
            ),
        );
        self.inserted_local_order = Some(order);
        Ok(Some(ClosureOnceReborrowV1 {
            callee,
            mutable_local: self.local_id(3)?,
            mutable_type,
            environment_type,
            shared_local: order.inserted_local,
            shared_type,
        }))
    }

    pub(super) fn closure_once_reborrow_statement_v1(
        &mut self,
        source: SemanticSourceProvenanceV1,
    ) -> Result<SemanticStatementV1, ProductionSemanticBodyErrorV1> {
        let reborrow = self
            .closure_once_reborrow
            .ok_or_else(|| table("closure once reborrow"))?;
        self.owner.charge(SemanticMirResourceV1::Projections, 1)?;
        self.work()?;
        Ok(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(reborrow.shared_local, vec![], reborrow.shared_type)?,
                SemanticRvalueV1::new(
                    reborrow.shared_type,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: SemanticPlaceV1::new(
                            reborrow.mutable_local,
                            vec![SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                reborrow.environment_type,
                            )?],
                            reborrow.environment_type,
                        )?,
                    },
                ),
            )),
        ))
    }

    pub(super) fn reborrow_closure_once_call_v1(
        &mut self,
        raw_block: u32,
        callee: Instance<'tcx>,
        arguments: &mut [SemanticOperandV1],
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let Some(reborrow) = self.closure_once_reborrow else {
            return Ok(());
        };
        if raw_block != 0 {
            return Ok(());
        }
        let original = SemanticOperandV1::Move(SemanticPlaceV1::new(
            reborrow.mutable_local,
            vec![],
            reborrow.mutable_type,
        )?);
        if callee != reborrow.callee || arguments.len() != 2 || arguments[0] != original {
            return Err(table("closure once reborrow call receiver"));
        }
        arguments[0] = SemanticOperandV1::Move(SemanticPlaceV1::new(
            reborrow.shared_local,
            vec![],
            reborrow.shared_type,
        )?);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
