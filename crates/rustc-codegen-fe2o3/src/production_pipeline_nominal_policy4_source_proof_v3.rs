//! Source-proof continuation retaining the actual nominal native-output owner.
use super::{Admitted, Budget, E, PreparedNominalPolicy4NativeV3 as Input, R, Resource, scoped};
use crate::production_native_source_lineage_v1::{
    PreparedNativeSourceProofPacketV1 as Packet, prepare_borrowed_erased_native_source_packet_v1,
    prepare_borrowed_native_source_packet_v1,
};
use crate::production_pipeline::native_checked_output_handoff_v1::{
    OutputOwnerV1, SourceProofV1, check_original_native_packet_v1, check_source_binding_v1,
};
use fe2o3_lower_mir_kernel::with_checked_source_pipeline_catalog_v1;
use fe2o3_verifier::{
    ValidatedNativeCompilerRankedSourceProofV1 as DirectProof,
    ValidatedNativeCompilerUnitLocalErasedSourceProofV1 as ErasedProof,
};
use std::mem::size_of;

enum SourcePacket {
    Direct(Packet<DirectProof>),
    Erased(Packet<ErasedProof>),
}
impl SourcePacket {
    fn retained_addition(&self) -> R<usize> {
        let (header, storage) = match self {
            Self::Direct(packet) => (header::<DirectProof>()?, packet.retained_storage()),
            Self::Erased(packet) => (header::<ErasedProof>()?, packet.retained_storage()),
        };
        header
            .checked_add(storage.map_err(E::SourceProof)?)
            .ok_or(Resource::Arithmetic.into())
    }
    fn proof(&self) -> SourceProofV1<'_> {
        match self {
            Self::Direct(packet) => SourceProofV1::Direct(packet.proof()),
            Self::Erased(packet) => SourceProofV1::Erased(packet.proof()),
        }
    }
    fn native_module(&self) -> &[u8] {
        match self {
            Self::Direct(packet) => packet.original_native_module(),
            Self::Erased(packet) => packet.original_native_module(),
        }
    }
}

/// A typed source proof is not protected compiler, artifact, or launch authority.
/// The original source, ranked roster, actual O, descriptor and native text stay
/// in one owner. No source proof is synthesized from O or descriptor bytes.
pub(crate) struct PreparedNominalPolicy4SourceProofV3 {
    native: Input,
    packet: SourcePacket,
    input_floor: usize,
    retained_floor: usize,
}
type Output = PreparedNominalPolicy4SourceProofV3;

/// Addition only; the consumed native owner's reservation remains caller-paid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalPolicy4SourceProofStorageV3(usize);
impl NominalPolicy4SourceProofStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

fn header<P>() -> R<usize> {
    size_of::<Output>()
        .checked_sub(size_of::<Input>())
        .and_then(|n| n.checked_sub(size_of::<Packet<P>>()))
        .ok_or(Resource::Accounting.into())
}

impl Input {
    /// On error the input is consumed and dropped; retire its original receipt
    /// once. On success reserve the returned addition before further allocation.
    pub(crate) fn into_nominal_source_proof_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(Output, NominalPolicy4SourceProofStorageV3)> {
        let incoming = budget.storage();
        if incoming < self.retained_storage_floor_v3() {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, move |budget| {
            budget.charge_work(4)?;
            self.verify_equivalence(budget)?;
            let ranked = &self.input.ranked_verification;
            let packet = match &self.input.admitted {
                Admitted::Direct(owner) => {
                    budget.reserve_storage(header::<DirectProof>()?)?;
                    SourcePacket::Direct(
                        prepare_borrowed_native_source_packet_v1(
                            owner.source_semantic_kir(),
                            ranked,
                            budget,
                        )
                        .map_err(E::SourceProof)?,
                    )
                }
                Admitted::Erased(owner) => {
                    budget.reserve_storage(header::<ErasedProof>()?)?;
                    SourcePacket::Erased(
                        prepare_borrowed_erased_native_source_packet_v1(
                            owner.erased_source(),
                            ranked,
                            budget,
                        )
                        .map_err(E::SourceProof)?,
                    )
                }
            };
            let packet_storage = match &packet {
                SourcePacket::Direct(p) => p.retained_storage(),
                SourcePacket::Erased(p) => p.retained_storage(),
            }
            .map_err(E::SourceProof)?;
            budget.reserve_storage(packet_storage)?;
            let addition = packet.retained_addition()?;
            let value = Output {
                native: self,
                packet,
                input_floor: incoming,
                retained_floor: incoming.checked_add(addition).ok_or(Resource::Arithmetic)?,
            };
            value.verify_equivalence(budget)?;
            Ok((value, NominalPolicy4SourceProofStorageV3(addition)))
        })
    }
}

impl Output {
    pub(crate) fn native(&self) -> &Input {
        &self.native
    }
    pub(crate) fn original_native_module(&self) -> &[u8] {
        self.packet.native_module()
    }
    pub(crate) const fn retained_storage_floor_v3(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            if self.input_floor < self.native.retained_storage_floor_v3()
                || self
                    .input_floor
                    .checked_add(self.packet.retained_addition()?)
                    != Some(self.retained_floor)
            {
                return Err(E::Mismatch("exact cumulative nominal source-proof receipt"));
            }
            self.native.verify_equivalence(budget)?;
            with_checked_source_pipeline_catalog_v1(
                self.native.source_anchor(),
                budget,
                |view, budget| {
                    let catalog = view.catalog(budget)?;
                    let owner = match &self.native.input.admitted {
                        Admitted::Direct(owner) => OutputOwnerV1::Direct(owner),
                        Admitted::Erased(owner) => OutputOwnerV1::Erased(owner),
                    };
                    let original = owner.source(catalog)?;
                    check_source_binding_v1(
                        &self.native.input.bindings,
                        original,
                        self.packet.proof(),
                        budget,
                    )?;
                    check_original_native_packet_v1(
                        original,
                        self.packet.proof(),
                        self.packet.native_module(),
                        budget,
                    )
                },
            )
            .map_err(E::SourceBinding)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_nominal_policy4_source_proof_v3_tests.rs"]
mod tests;
