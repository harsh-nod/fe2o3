//! Independent final-U agreement. Neither framing nor history creates a source proof.
use super::{
    Budget, Descriptor, Graph, Inputs, Native, Resource, Role, Roster, Source,
    analyze_canonical_output_guarded_formal_memory_v1,
    check_native_v12_text_descriptor_relation_v1,
    check_production_target_coordinate_preservation_v1,
};
use fe2o3_compiler_ffi::{
    INERT_LOOP_UNROLL_HASH_STORAGE_V1 as HASH_STORAGE,
    INERT_LOOP_UNROLL_READ_STORAGE_V1 as READ_STORAGE,
    InertLoopUnrollContentIdentityV1 as Identity, InertLoopUnrollOutputErrorV1 as FrameError,
    InertLoopUnrollOutputFieldV1 as Field, InertLoopUnrollOutputRefV1 as Frame,
    MAX_INERT_LOOP_UNROLL_STORAGE_V1 as MAX_STORAGE, inert_loop_unroll_history_identity_v1,
    inert_loop_unroll_output_identity_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 as Limits;
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryLimitsV1 as PrefixLimits,
    DecodedLoopUnrollHistoryV1 as History, LoopUnrollHistoryErrorV1,
};
use std::{fmt, mem::size_of};

#[path = "compiler_loop_unroll_output_joins_v1.rs"]
mod joins;

/// Original independently validated signed source, not a final-F or final-U receipt.
pub type LoopUnrollOriginalSourceProofV1<'s> = super::RefinedForwardingOriginalSourceProofV1<'s>;

#[derive(Debug)]
pub enum CompilerLoopUnrollOutputErrorV1 {
    Resource(Resource),
    Framing(FrameError<Resource>),
    History(LoopUnrollHistoryErrorV1),
    Join(super::CompilerRefinedForwardingOutputErrorV1),
    Mismatch(&'static str),
}
type E = CompilerLoopUnrollOutputErrorV1;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert final-U/source agreement: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) | Self::Framing(FrameError::Charge(error)) => Some(error),
            Self::History(error) => Some(error),
            Self::Join(error) => Some(error),
            // The allocation-free FFI framing enum is not an Error implementor.
            Self::Framing(_) | Self::Mismatch(_) => None,
        }
    }
}

/// New borrowed header only, returned unreserved. Caller retains the complete
/// signed source and decoded history receipts and actual input backing capacities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerLoopUnrollOutputStorageV1(usize);
impl CompilerLoopUnrollOutputStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CompilerLoopUnrollOutputStorageV1;

/// Move-only inert content agreement over U and independently signed original N/E.
/// It is not execution, rustc ABI authentication, artifact or launch authority.
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerLoopUnrollOutputV1 as Checked;
/// fn copy(v: Checked<'_, '_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerLoopUnrollOutputV1 as Checked;
/// fn escape<'a>(v: Checked<'a, 'a, 'a>) -> Checked<'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerLoopUnrollOutputV1 as Checked;
/// fn mutate(v: Checked<'_, '_, '_>) { v.output().module().kernels.clear(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerLoopUnrollOutputV1 as Checked;
/// fn forge() -> Checked<'static, 'static, 'static> { Checked::default() }
/// ```
pub struct CheckedCompilerLoopUnrollOutputV1<'a, 'h, 'w> {
    frame: &'a Frame<'w>,
    history: &'a History<'h, 'w>,
    source: Source<'a>,
    identity: Identity,
    history_identity: Identity,
    prefix_limits: PrefixLimits,
    limits: Limits,
    storage: Storage,
}
type Checked<'a, 'h, 'w> = CheckedCompilerLoopUnrollOutputV1<'a, 'h, 'w>;
impl<'a, 'h, 'w> Checked<'a, 'h, 'w> {
    pub fn output(&self) -> &Graph {
        self.history.output()
    }
    pub const fn identity(&self) -> Identity {
        self.identity
    }
    pub const fn history_identity(&self) -> Identity {
        self.history_identity
    }
    pub const fn prefix_limits(&self) -> PrefixLimits {
        self.prefix_limits
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn storage(&self) -> Storage {
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
    pub fn replay(&self, budget: &mut Budget<'_>) -> R<()> {
        let required = input_floor(self.frame, self.history, self.source)?
            .checked_add(self.storage.0)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            let (fresh, storage) = check_compiler_loop_unroll_output_v1(
                self.frame,
                self.history,
                self.source,
                budget,
            )?;
            budget.reserve_storage(storage.0)?;
            budget.charge_work(
                size_of::<Limits>()
                    .checked_add(size_of::<PrefixLimits>())
                    .and_then(|n| n.checked_add(2 * size_of::<Identity>()))
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if fresh.identity != self.identity
                || fresh.history_identity != self.history_identity
                || fresh.prefix_limits != self.prefix_limits
                || fresh.limits != self.limits
            {
                return Err(E::Mismatch(
                    "exact retained U content identities and both limits",
                ));
            }
            drop(fresh);
            budget.release_storage(storage.0)?;
            Ok(())
        })
    }
}

fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    super::scoped(budget, |budget| Ok(run(budget))).map_err(E::Join)?
}

fn backing_floor(
    source: usize,
    frame: &Frame<'_>,
    history_bytes: &[u8],
    history_frame_storage: usize,
    decoded_storage: usize,
) -> R<usize> {
    let separate = if std::ptr::eq(history_bytes, frame.field(Field::History)) {
        0
    } else {
        history_bytes.len()
    };
    source
        .checked_add(frame.canonical_bytes().len())
        .and_then(|n| n.checked_add(separate))
        .and_then(|n| n.checked_add(size_of::<Frame<'_>>()))
        .and_then(|n| n.checked_add(history_frame_storage))
        .and_then(|n| n.checked_add(decoded_storage))
        .ok_or(Resource::Arithmetic.into())
}

fn input_floor(frame: &Frame<'_>, history: &History<'_, '_>, source: Source<'_>) -> R<usize> {
    // The U receipts already include their embedded F frame/decoded owners.
    backing_floor(
        source.floor().map_err(E::Join)?,
        frame,
        history.frame().canonical_bytes(),
        history.frame().storage().retained_storage(),
        history.storage().retained_storage(),
    )
}

fn header_storage() -> R<usize> {
    size_of::<Checked<'_, '_, '_>>()
        .checked_add(size_of::<Inputs<'_>>())
        .and_then(|n| n.checked_add(READ_STORAGE))
        .ok_or(Resource::Arithmetic.into())
}

/// Independent signed-source, complete F history, F-to-U, fresh U formal, root,
/// physical ABI, capability and native-text agreement. No optimizer runs here.
/// Caller prepays complete source/history receipts and actual backing. New
/// visible headers/copies are metered; reused source/proof/codec engines retain
/// their existing domains, not a whole-process/RSS accounting claim. Existing
/// V1 physical ABI refusals remain unchanged; nominal V3 is not selected here.
pub fn check_compiler_loop_unroll_output_v1<'a, 'h, 'w>(
    frame: &'a Frame<'w>,
    history: &'a History<'h, 'w>,
    source: LoopUnrollOriginalSourceProofV1<'a>,
    budget: &mut Budget<'_>,
) -> R<(Checked<'a, 'h, 'w>, Storage)> {
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        if budget.storage_limit() > MAX_STORAGE {
            return Err(E::Mismatch("bounded storage cap"));
        }
        if frame.route() != source.route() {
            return Err(E::Mismatch("independent source route"));
        }
        if budget.storage() < input_floor(frame, history, source)? {
            return Err(Resource::Accounting.into());
        }
        let storage = CompilerLoopUnrollOutputStorageV1(size_of::<Checked<'_, '_, '_>>());
        budget.reserve_storage(header_storage()?)?;
        super::bytes(
            frame.field(Field::History),
            history.frame().canonical_bytes(),
            "complete U history field",
            budget,
        )
        .map_err(E::Join)?;
        let checked_history = history.check_semantics(budget).map_err(E::History)?;
        budget.reserve_storage(checked_history.storage().retained_storage())?;
        let inputs = source.inputs(budget).map_err(E::Join)?;
        super::original_root_count(frame.root_count(), &inputs, source, budget).map_err(E::Join)?;
        super::bytes(
            frame.field(Field::SemanticMir),
            inputs.semantic.canonical_encoding(),
            "semantic source bytes",
            budget,
        )
        .map_err(E::Join)?;
        let original_subject = super::envelope(
            frame.field(Field::OriginalNative),
            inputs.original,
            inputs.catalog,
            budget,
        )
        .map_err(E::Join)?;
        let neutral = match inputs.erased {
            Some(erased) => {
                super::bytes(
                    frame.field(Field::Erased),
                    erased.canonical().canonical_bytes(),
                    "actual E",
                    budget,
                )
                .map_err(E::Join)?;
                erased
            }
            None => {
                if !frame.field(Field::Erased).is_empty() {
                    return Err(E::Mismatch("Direct empty E"));
                }
                inputs.original
            }
        };
        super::bytes(
            frame.field(Field::OriginalMiddleEnd),
            inputs.middle.canonical_bytes(),
            "signed middle roster",
            budget,
        )
        .map_err(E::Join)?;
        super::bytes(
            frame.field(Field::OriginalCorrespondence),
            inputs.correspondence.canonical_bytes(),
            "signed correspondence roster",
            budget,
        )
        .map_err(E::Join)?;
        super::bytes(
            frame.field(Field::OriginalVerus),
            inputs.verus.canonical_bytes(),
            "signed Verus roster",
            budget,
        )
        .map_err(E::Join)?;
        joins::association(frame, budget)?;
        let (original_formal, original_storage) =
            source.original_formal(budget).map_err(E::Join)?;
        budget.reserve_storage(original_storage.retained_storage())?;
        super::codec::<Roster>(frame.field(Field::OriginalFormalMemory).len(), budget)
            .map_err(E::Join)?;
        let original_roster = Roster::decode(frame.field(Field::OriginalFormalMemory))
            .map_err(|error| E::Join(super::E::Roster(error)))?;
        super::joins::formal(
            &inputs,
            inputs.original,
            &original_subject,
            &original_roster,
            original_formal.kernels(),
            budget,
        )
        .map_err(E::Join)?;
        drop(original_roster);
        drop(original_formal);
        budget.release_storage(original_storage.retained_storage())?;

        super::codec::<Native>(frame.field(Field::NativeV2).len(), budget).map_err(E::Join)?;
        let native = Native::decode(frame.field(Field::NativeV2))
            .map_err(|error| E::Join(super::E::Native(error)))?;
        let profile = super::joins::profile(&native).map_err(E::Join)?;
        let (coordinates, coordinate_storage) = check_production_target_coordinate_preservation_v1(
            neutral,
            history.prefix().graph(Role::B),
            profile,
            budget,
        )
        .map_err(|error| E::Join(super::E::Coordinates(error)))?;
        budget.reserve_storage(coordinate_storage.retained_storage())?;
        drop(coordinates);
        budget.release_storage(coordinate_storage.retained_storage())?;
        let output = history.output();
        let final_subject = super::envelope(
            frame.field(Field::FinalNative),
            output,
            inputs.catalog,
            budget,
        )
        .map_err(E::Join)?;
        let (final_formal, final_storage) =
            analyze_canonical_output_guarded_formal_memory_v1(output, source.anchor(), budget)
                .map_err(|error| E::Join(super::E::FinalFormal(error)))?;
        budget.reserve_storage(final_storage.retained_storage())?;
        super::codec::<Roster>(frame.field(Field::FinalFormalMemory).len(), budget)
            .map_err(E::Join)?;
        let final_roster = Roster::decode(frame.field(Field::FinalFormalMemory))
            .map_err(|error| E::Join(super::E::Roster(error)))?;
        super::joins::formal(
            &inputs,
            output,
            &final_subject,
            &final_roster,
            final_formal.kernels(),
            budget,
        )
        .map_err(E::Join)?;
        super::codec::<Descriptor>(frame.field(Field::Descriptor).len(), budget)
            .map_err(E::Join)?;
        let descriptor = Descriptor::decode(frame.field(Field::Descriptor))
            .map_err(|error| E::Join(super::E::Descriptor(error)))?;
        joins::roots(frame, &inputs, output, &native, &descriptor, budget)?;
        super::joins::capabilities(output, &descriptor, budget).map_err(E::Join)?;
        budget.charge_work(native.module_bytes().len())?;
        let llvm =
            std::str::from_utf8(native.module_bytes()).map_err(|_| E::Mismatch("LLVM UTF-8"))?;
        let relation = check_native_v12_text_descriptor_relation_v1(
            output,
            inputs.catalog,
            output.canonical().canonical_bytes(),
            profile,
            descriptor.table(),
            llvm,
            budget,
        )
        .map_err(|error| E::Join(super::E::TextDescriptor(error)))?;
        budget.reserve_storage(relation.storage().retained_storage())?;
        budget.reserve_storage(HASH_STORAGE)?;
        let limit = budget.storage_limit();
        let identity =
            inert_loop_unroll_output_identity_v1(frame, limit, |work| budget.charge_work(work))
                .map_err(E::Framing)?;
        let history_identity =
            inert_loop_unroll_history_identity_v1(frame.field(Field::History), limit, |work| {
                budget.charge_work(work)
            })
            .map_err(E::Framing)?;
        budget.charge_work(1)?;
        drop(relation);
        drop(descriptor);
        drop(final_roster);
        drop(final_formal);
        drop(native);
        drop(checked_history);
        Ok((
            Checked {
                frame,
                history,
                source,
                identity,
                history_identity,
                prefix_limits: history.prefix().frame().limits(),
                limits: history.limits(),
                storage,
            },
            storage,
        ))
    })
}

#[cfg(test)]
#[path = "compiler_loop_unroll_output_v1_tests.rs"]
mod tests;
