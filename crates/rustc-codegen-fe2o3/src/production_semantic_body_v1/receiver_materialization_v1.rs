//! Canonical placement of the single shared receiver temporary.

use super::receiver_reborrow_v1::{
    ReceiverReborrowErrorV1, ReceiverReborrowV1, derive_fn_receiver_reborrow_v1,
};
use super::*;
use crate::rustc_semantic_adapter_v1::{
    SemanticIdentityDigestV1, borrowed_rustc_mir_body_sha256_v1, canonical_function_identities_v1,
    rustc_block_identity_v1, rustc_local_identity_v1,
};

#[path = "receiver_fingerprint_v1.rs"]
mod receiver_fingerprint_v1;
pub(super) use receiver_fingerprint_v1::charge_once_shim_fingerprint_v1;
#[path = "receiver_source_v1.rs"]
mod receiver_source_v1;
#[cfg(test)]
pub(super) use receiver_source_v1::assert_receiver_source_guards_v1;
pub(super) use receiver_source_v1::reobserve_receiver_source_v1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReceiverLocalV1 {
    pub(crate) identity: SemanticLocalIdentityV1,
    pub(crate) local: SemanticLocalIdV1,
}

impl ReceiverLocalV1 {
    pub(crate) fn derive(
        function: SemanticFunctionIdentityV1,
        block: SemanticBlockIdentityV1,
        raw_local_identities: impl IntoIterator<Item = SemanticLocalIdentityV1>,
    ) -> Result<Self, &'static str> {
        let mut digest = SemanticIdentityDigestV1::new(
            b"fe2o3/semantic-mir/fn-receiver-shared-reborrow/local/v1",
        );
        digest.field(function.as_bytes());
        digest.field(block.as_bytes());
        digest.field(&0_u32.to_le_bytes());
        let identity = SemanticLocalIdentityV1::from_sha256(digest.finish());
        let mut position = 0_u32;
        for raw in raw_local_identities {
            if raw == identity {
                return Err("shared receiver local identity collision");
            }
            if raw < identity {
                position = position
                    .checked_add(1)
                    .ok_or("shared receiver local rank overflow")?;
            }
        }
        Ok(Self {
            identity,
            local: SemanticLocalIdV1::from_index(position),
        })
    }

    pub(crate) fn remap(self, original: SemanticLocalIdV1) -> Option<SemanticLocalIdV1> {
        let index = original.index();
        index
            .checked_add(u32::from(index >= self.local.index()))
            .map(SemanticLocalIdV1::from_index)
    }
}

pub(super) struct ReceiverMaterializationV1<'tcx> {
    pub(super) observation: ReceiverReborrowV1<'tcx>,
    pub(super) local: ReceiverLocalV1,
    pub(super) ty: SemanticTypeIdV1,
    pub(super) consumed: bool,
}

impl<'tcx> ReceiverMaterializationV1<'tcx> {
    pub(super) fn derive(
        input: &ProductionSemanticBodyInputV1<'_, 'tcx>,
        blocks: &[&ProductionSemanticBlockBindingV1],
        calls: &CallTablesV1<'_, 'tcx>,
        types: &HashMap<Ty<'tcx>, SemanticTypeIdV1>,
        has_context: bool,
        owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
    ) -> Result<Option<Self>, ProductionSemanticBodyErrorV1> {
        if matches!(
            input.instance.def,
            rustc_middle::ty::InstanceKind::ClosureOnceShim { .. }
        ) {
            charge_once_shim_fingerprint_v1(input.tcx, input.body, |amount| {
                owner.charge(SemanticMirResourceV1::ValidationWork, amount)
            })
            .map_err(|error| match error {
                ReceiverReborrowErrorV1::Resource(error) => error,
                ReceiverReborrowErrorV1::Unsupported(construct) => {
                    unsupported(construct, None, None)
                }
            })?;
        }
        let observation =
            derive_fn_receiver_reborrow_v1(input.tcx, input.instance, input.body, |amount| {
                owner.charge(SemanticMirResourceV1::ValidationWork, amount)
            })
            .map_err(|error| match error {
                ReceiverReborrowErrorV1::Resource(error) => error,
                ReceiverReborrowErrorV1::Unsupported(construct) => {
                    unsupported(construct, None, None)
                }
            })?;
        let Some(observation) = observation else {
            return Ok(None);
        };
        if has_context || input.role != SemanticFunctionRoleV1::InternalHelper {
            return Err(table("shared receiver on a context or exported root"));
        }
        owner.charge(SemanticMirResourceV1::ValidationWork, 5)?;
        let raw_block = observation.block() as usize;
        let block = blocks
            .get(raw_block)
            .copied()
            .ok_or_else(|| table("shared receiver block"))?;
        let direct = calls
            .0
            .get(raw_block)
            .copied()
            .flatten()
            .ok_or_else(|| table("shared receiver direct call"))?;
        if direct.caller != input.function
            || direct.expected_callee != observation.callee()
            || calls.1.get(raw_block).is_some_and(Option::is_some)
            || calls.2.get(raw_block).is_some_and(Option::is_some)
        {
            return Err(table("shared receiver call binding"));
        }
        let function = canonical_function_identities_v1(input.tcx, input.instance).function();
        if function != input.identities.identity {
            return Err(table("shared receiver function identity"));
        }
        // Stable span hashing uses byte columns. Reject a changed body before
        // converting its potentially untrusted spans to character coordinates.
        let body_sha256 = borrowed_rustc_mir_body_sha256_v1(input.tcx, input.instance, input.body);
        if block.identity != rustc_block_identity_v1(function, body_sha256, observation.block()) {
            return Err(table("shared receiver borrowed body identity"));
        }
        let terminator_span = input.body.basic_blocks
            [rustc_middle::mir::BasicBlock::from_usize(raw_block)]
        .terminator()
        .source_info
        .span;
        let source =
            reobserve_receiver_source_v1(input.tcx, terminator_span, input.body.span, owner)?;
        if source != block.terminator_source {
            return Err(table("shared receiver terminator provenance"));
        }
        owner.charge(
            SemanticMirResourceV1::ValidationWork,
            input.local_bindings.len(),
        )?;
        for binding in input.local_bindings {
            if binding.rustc_local as usize >= input.body.local_decls.len()
                || binding.identity
                    != rustc_local_identity_v1(function, body_sha256, binding.rustc_local)
            {
                return Err(table("shared receiver raw local identity"));
            }
        }
        owner.charge(
            SemanticMirResourceV1::ValidationWork,
            input.local_bindings.len(),
        )?;
        let local = ReceiverLocalV1::derive(
            function,
            block.identity,
            input.local_bindings.iter().map(|binding| binding.identity),
        )
        .map_err(table)?;
        let ty = types
            .get(&observation.shared_receiver_type())
            .copied()
            .ok_or_else(|| table("shared receiver canonical type"))?;
        owner.charge(SemanticMirResourceV1::Locals, 1)?;
        Ok(Some(Self {
            observation,
            local,
            ty,
            consumed: false,
        }))
    }
}

impl<'tcx> BodyProducerV1<'_, '_, 'tcx> {
    pub(super) fn consume_receiver_reborrow(
        &mut self,
        block: u32,
        callee: Instance<'tcx>,
    ) -> Result<Option<(SemanticStatementKindV1, SemanticOperandV1)>, ProductionSemanticBodyErrorV1>
    {
        let Some(reborrow) = self
            .receiver_reborrow
            .as_ref()
            .filter(|reborrow| reborrow.observation.block() == block)
        else {
            return Ok(None);
        };
        if reborrow.consumed
            || reborrow.observation.callee() != callee
            || !self
                .consumed_direct_calls
                .get(block as usize)
                .copied()
                .unwrap_or(false)
        {
            return Err(table("shared receiver adjustment consumption"));
        }
        let (observation, local, ty) = (reborrow.observation, reborrow.local, reborrow.ty);
        let TyKind::Ref(_, pointee, Mutability::Not) = observation.shared_receiver_type().kind()
        else {
            return Err(table("shared receiver pointee"));
        };
        self.work()?;
        let base = self.construct_place(observation.receiver(), Some(block), None)?;
        self.owner.charge(SemanticMirResourceV1::Projections, 1)?;
        let pointee = self.type_id(*pointee, Some(block), None)?;
        let mut projections = try_vec_v1(1, SemanticMirResourceV1::Projections)?;
        projections.push(SemanticProjectionV1::new(
            SemanticProjectionKindV1::Dereference,
            pointee,
        )?);
        let place = SemanticPlaceV1::new(base.local(), projections, pointee)?;
        let temporary = SemanticPlaceV1::new(local.local, Vec::new(), ty)?;
        let assignment = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            temporary.clone(),
            SemanticRvalueV1::new(
                ty,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                },
            ),
        ));
        self.receiver_reborrow.as_mut().unwrap().consumed = true;
        Ok(Some((assignment, SemanticOperandV1::Move(temporary))))
    }
}

#[cfg(test)]
#[path = "receiver_materialization_tests.rs"]
mod tests;
