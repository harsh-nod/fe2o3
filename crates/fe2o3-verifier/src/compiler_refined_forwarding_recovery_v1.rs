//! Complete inert replay retaining the recovered signed source and actual final F.
use super::*;
use crate::{
    validate_native_compiler_ranked_source_packet_v1,
    validate_native_compiler_unit_local_erased_source_packet_v1,
};
use fe2o3_compiler_ffi::read_inert_refined_forwarding_output_v1;
use fe2o3_kernel_opt::{
    materialize_refined_forwarding_history_v1, read_refined_forwarding_history_v1,
};

enum RecoveredSource {
    Direct(Direct),
    Erased(Erased),
}
impl RecoveredSource {
    fn proof(&self) -> Source<'_> {
        match self {
            Self::Direct(source) => Source::Direct(source),
            Self::Erased(source) => Source::Erased(source),
        }
    }
}

/// Full source-proof receipt, F's original admission receipt and wrapper header.
/// Returned unreserved. Discarded packet adapters/history/frames are excluded;
/// embedded source/graph headers follow the existing conservative accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveredCompilerRefinedForwardingStorageV1(usize);
impl RecoveredCompilerRefinedForwardingStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owned complete source-to-final-F agreement. All getters are immutable.
/// Content identities bind the checked output fields, not an external publication.
/// Embedded signature keys do not authenticate protected compiler/launch origin.
/// This grants no execution, rustc ABI, artifact or launch authority.
///
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerRefinedForwardingOutputV1 as Owner;
/// fn duplicate(value: Owner) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerRefinedForwardingOutputV1 as Owner;
/// fn manufacture() -> Owner { Owner::default() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerRefinedForwardingOutputV1 as Owner;
/// fn mutate(value: Owner) { value.output().module().kernels.clear(); }
/// ```
pub struct RecoveredCompilerRefinedForwardingOutputV1 {
    source: RecoveredSource,
    output: Graph,
    identity: Identity,
    history_identity: Identity,
    limits: Limits,
    storage: RecoveredCompilerRefinedForwardingStorageV1,
}
impl RecoveredCompilerRefinedForwardingOutputV1 {
    pub fn source_proof(&self) -> Source<'_> {
        self.source.proof()
    }
    pub fn output(&self) -> &Graph {
        &self.output
    }
    pub const fn identity(&self) -> Identity {
        self.identity
    }
    pub const fn history_identity(&self) -> Identity {
        self.history_identity
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn storage(&self) -> RecoveredCompilerRefinedForwardingStorageV1 {
        self.storage
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn authenticates_rustc_abi(&self) -> bool {
        false
    }
}

/// Recovers both source and actual F through the existing independent checkers.
/// The two inert inputs retain their independent size limits and schemas. They
/// are not a capsule and are not accepted as protected compiler execution.
///
/// The caller must reserve both input byte regions (conservatively counted twice
/// if overlapping). On success, failure or unwind, temporary storage returns to
/// that inherited floor without resetting work, peak or failure history. The
/// returned receipt must be reserved before using or retaining the owned result.
pub fn recover_compiler_refined_forwarding_output_v1(
    output_bytes: &[u8],
    source_packet: &[u8],
    budget: &mut Budget<'_>,
) -> R<(
    RecoveredCompilerRefinedForwardingOutputV1,
    RecoveredCompilerRefinedForwardingStorageV1,
)> {
    scoped(budget, |budget| {
        budget.charge_work(8)?;
        if budget.storage_limit() > MAX_STORAGE {
            return Err(E::Mismatch("bounded storage cap"));
        }
        let input_floor = output_bytes
            .len()
            .checked_add(source_packet.len())
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < input_floor {
            return Err(Resource::Accounting.into());
        }
        let header = size_of::<RecoveredCompilerRefinedForwardingOutputV1>();
        budget.reserve_storage(
            header
                .checked_add(READ_STORAGE)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let limit = budget.storage_limit();
        let frame = read_inert_refined_forwarding_output_v1(output_bytes, limit, |work| {
            budget.charge_work(work)
        })
        .map_err(E::Framing)?;
        let (source, source_storage) = match frame.route() {
            Route::Direct => {
                let (proof, storage) =
                    validate_native_compiler_ranked_source_packet_v1(source_packet, budget)
                        .map_err(E::SourcePacket)?;
                (RecoveredSource::Direct(proof), storage.retained_storage())
            }
            Route::Erased => {
                let (proof, storage) = validate_native_compiler_unit_local_erased_source_packet_v1(
                    source_packet,
                    budget,
                )
                .map_err(E::SourcePacket)?;
                (RecoveredSource::Erased(proof), storage.retained_storage())
            }
        };
        budget.reserve_storage(source_storage)?;
        let history_frame = read_refined_forwarding_history_v1(frame.field(Field::History), budget)
            .map_err(E::HistoryWire)?;
        budget.reserve_storage(history_frame.storage().retained_storage())?;
        let history = materialize_refined_forwarding_history_v1(&history_frame, budget)
            .map_err(E::HistoryWire)?;
        let history_storage = history.storage().retained_storage();
        budget.reserve_storage(history_storage)?;
        let (checked, receipt) =
            check_compiler_refined_forwarding_output_v1(&frame, &history, source.proof(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let (identity, history_identity, limits) = (
            checked.identity(),
            checked.history_identity(),
            checked.limits(),
        );
        drop(checked);
        budget.release_storage(receipt.retained_storage())?;
        let (output, output_storage) = history.into_final_graph();
        budget.release_storage(
            history_storage
                .checked_sub(output_storage.retained_storage())
                .ok_or(Resource::Accounting)?,
        )?;
        let storage = RecoveredCompilerRefinedForwardingStorageV1(
            header
                .checked_add(source_storage)
                .and_then(|n| n.checked_add(output_storage.retained_storage()))
                .ok_or(Resource::Arithmetic)?,
        );
        drop(history_frame);
        drop(frame);
        Ok((
            RecoveredCompilerRefinedForwardingOutputV1 {
                source,
                output,
                identity,
                history_identity,
                limits,
                storage,
            },
            storage,
        ))
    })
}
