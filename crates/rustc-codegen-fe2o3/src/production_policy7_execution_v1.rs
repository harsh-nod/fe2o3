//! Exact in-process schedule observation. No decoder, source or proof authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 as Coordinate;
use fe2o3_kernel_opt::{
    CheckedCanonicalKernelIrOwnerPolicy6V1 as Checked6, OwnedRedundantStoreContinuationV1 as OwnedJ,
};

pub(super) const HEADER: usize = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1;
const ROW: usize = fe2o3_kernel_opt::POLICY7_EXECUTION_ROW_BYTES_V1;
const CONDITIONAL_BINDING_WORK: usize = 42;

// Byte observations only, never an ordinary source-admission owner.
#[derive(Clone, Copy)]
struct RecordView<'a> {
    prefix_record: &'a [u8; 256],
    input: &'a Graph,
    output: &'a Graph,
    continuation: &'a OwnedJ,
}
impl<'a> RecordView<'a> {
    fn ordinary(owner: Admitted7Ref<'a>) -> Self {
        Self {
            prefix_record: owner.prefix_record(),
            input: owner.input(),
            output: owner.output(),
            continuation: owner.continuation(),
        }
    }
}

/// Sealed by one actual consuming Policy6-to-J preparation, not inert decoding.
pub(crate) struct Policy7ExecutionWitnessV1 {
    bytes: Box<[u8]>,
}
impl Policy7ExecutionWitnessV1 {
    pub(crate) const fn policy_version(&self) -> u16 {
        7
    }
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::production_pipeline) fn retained_storage(&self) -> usize {
        size_of::<Self>() + self.bytes.len()
    }

    pub(super) fn prepare(owner: &Admitted7, budget: &mut Budget<'_>) -> Result7<Self> {
        Self::prepare_view(RecordView::ordinary(owner.borrowed()), budget)
    }

    /// Actual typed I/J custody is prepaid on the caller's original account.
    /// This only encodes observations; the conditional source/history checker
    /// must independently replay semantics before any private final custody.
    pub(in crate::production_pipeline) fn prepare_conditional_v1(
        checked: &Checked6,
        tail: &OwnedJ,
        budget: &mut Budget<'_>,
    ) -> Result7<Self> {
        let required = checked
            .retained_storage()
            .checked_add(tail.retained_storage())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        scoped(required, budget, |budget| {
            budget
                .charge_work(CONDITIONAL_BINDING_WORK)
                .map_err(resource)?;
            if checked.owner().canonical().identity() != tail.input_identity() {
                return Err(execution_error(
                    "conditional Policy7 actual I/J input identity",
                ));
            }
            Self::prepare_view(
                RecordView {
                    prefix_record: checked.execution().canonical_bytes(),
                    input: checked.owner(),
                    output: tail.output(),
                    continuation: tail,
                },
                budget,
            )
        })
    }

    fn prepare_view(owner: RecordView<'_>, budget: &mut Budget<'_>) -> Result7<Self> {
        scoped(0, budget, |budget| {
            let extent = extent(owner)?;
            let prepaid = extent
                .checked_mul(2)
                .and_then(|n| n.checked_add(size_of::<Self>()))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(prepaid).map_err(resource)?;
            budget.charge_work(extent).map_err(resource)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(extent)
                .map_err(|_| resource(Resource::Allocation))?;
            budget
                .reserve_storage(
                    bytes
                        .capacity()
                        .checked_sub(extent)
                        .ok_or_else(|| resource(Resource::Accounting))?,
                )
                .map_err(resource)?;
            bytes.resize(extent, 0);
            visit(owner, budget, |offset, part| {
                bytes
                    .get_mut(
                        offset
                            ..offset
                                .checked_add(part.len())
                                .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )
                    .ok_or_else(|| execution_error("Policy7 encoding extent"))?
                    .copy_from_slice(part);
                Ok(())
            })?;
            let witness = Self {
                bytes: bytes.into_boxed_slice(),
            };
            witness.check_view(owner, budget)?;
            Ok(witness)
        })
    }

    pub(super) fn check(&self, owner: &Admitted7, budget: &mut Budget<'_>) -> Result7<()> {
        self.check_history_v1(owner.borrowed(), budget)
    }

    pub(in crate::production_pipeline) fn check_history_v1(
        &self,
        owner: Admitted7Ref<'_>,
        budget: &mut Budget<'_>,
    ) -> Result7<()> {
        self.check_view(RecordView::ordinary(owner), budget)
    }

    fn check_view(&self, owner: RecordView<'_>, budget: &mut Budget<'_>) -> Result7<()> {
        if self.bytes.len() != extent(owner)? {
            return Err(execution_error("Policy7 record extent"));
        }
        visit(owner, budget, |offset, part| {
            if self.bytes.get(
                offset
                    ..offset
                        .checked_add(part.len())
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
            ) != Some(part)
            {
                return Err(execution_error("Policy7 complete execution transcript"));
            }
            Ok(())
        })
    }
}

fn extent(owner: RecordView<'_>) -> Result7<usize> {
    extent_counts(
        owner.continuation.rows().len(),
        owner.continuation.retained_operations().len(),
    )
}

pub(super) fn extent_counts(deleted: usize, retained: usize) -> Result7<usize> {
    deleted
        .checked_add(retained)
        .and_then(|n| n.checked_mul(ROW))
        .and_then(|n| n.checked_add(HEADER))
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn coordinate_bytes(value: Coordinate) -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[..4].copy_from_slice(&value.block.function.0.to_le_bytes());
    bytes[4..8].copy_from_slice(&value.block.block.to_le_bytes());
    bytes[8..12].copy_from_slice(&value.operation.to_le_bytes());
    bytes
}

/// Visits every byte once in a fixed field order without another record buffer.
/// These are exact observations; full source/actual-pair replay remains mandatory.
fn visit(
    owner: RecordView<'_>,
    budget: &mut Budget<'_>,
    mut output: impl FnMut(usize, &[u8]) -> Result7<()>,
) -> Result7<()> {
    let length = extent(owner)?;
    budget
        .charge_work(
            length
                .checked_add(7)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    let continuation = owner.continuation;
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(b"F2P7EX1\0");
    header[8..10].copy_from_slice(&1u16.to_le_bytes());
    header[10..12].copy_from_slice(&7u16.to_le_bytes());
    header[12..16].copy_from_slice(&256u32.to_le_bytes());
    header[16..272].copy_from_slice(owner.prefix_record);
    for (offset, graph) in [(272, owner.input), (312, owner.output)] {
        let identity = graph.canonical().identity();
        header[offset..offset + 32].copy_from_slice(identity.digest());
        header[offset + 32..offset + 40]
            .copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    for (offset, count) in [
        (352, continuation.rows().len()),
        (360, continuation.retained_operations().len()),
        (376, length),
    ] {
        header[offset..offset + 8].copy_from_slice(
            &u64::try_from(count)
                .map_err(|_| resource(Resource::Arithmetic))?
                .to_le_bytes(),
        );
    }
    header[368..376].copy_from_slice(&1u64.to_le_bytes());
    output(0, &header)?;
    let mut offset = HEADER;
    let mut previous = None;
    for row in continuation.rows() {
        if row.anchor >= row.removed || previous.is_some_and(|last| last >= row.removed) {
            return Err(execution_error("Policy7 ordered deletion coordinates"));
        }
        previous = Some(row.removed);
        output(offset, &coordinate_bytes(row.anchor))?;
        output(offset + 12, &coordinate_bytes(row.removed))?;
        offset = offset
            .checked_add(ROW)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    let mut previous = None;
    for row in continuation.retained_operations() {
        if previous.is_some_and(|(input, output)| input >= row.input || output >= row.output) {
            return Err(execution_error("Policy7 ordered retained coordinates"));
        }
        previous = Some((row.input, row.output));
        output(offset, &coordinate_bytes(row.input))?;
        output(offset + 12, &coordinate_bytes(row.output))?;
        offset = offset
            .checked_add(ROW)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    if offset != length {
        return Err(execution_error("Policy7 complete row extent"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn exercise_exact_record(owner: &Admitted7, budget: &mut Budget<'_>) {
    let witness = Policy7ExecutionWitnessV1::prepare(owner, budget).unwrap();
    let bytes = witness.canonical_bytes();
    assert_eq!(&bytes[..16], b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    assert_eq!(&bytes[16..272], owner.prefix_record());
    assert_eq!(
        &bytes[272..304],
        owner.input().canonical().identity().digest()
    );
    assert_eq!(
        &bytes[312..344],
        owner.output().canonical().identity().digest()
    );
    assert_eq!(
        u64::from_le_bytes(bytes[304..312].try_into().unwrap()),
        owner.input().canonical().identity().canonical_length()
    );
    assert_eq!(
        u64::from_le_bytes(bytes[344..352].try_into().unwrap()),
        owner.output().canonical().identity().canonical_length()
    );
    assert_eq!(
        u64::from_le_bytes(bytes[352..360].try_into().unwrap()),
        owner.continuation().rows().len() as u64
    );
    assert_eq!(
        u64::from_le_bytes(bytes[360..368].try_into().unwrap()),
        owner.continuation().retained_operations().len() as u64
    );
    let expected = owner
        .continuation()
        .rows()
        .iter()
        .map(|r| [r.anchor, r.removed])
        .chain(
            owner
                .continuation()
                .retained_operations()
                .iter()
                .map(|r| [r.input, r.output]),
        );
    for (encoded, row) in bytes[HEADER..].chunks_exact(24).zip(expected) {
        for (encoded, coordinate) in encoded.chunks_exact(12).zip(row) {
            let fields: Vec<u32> = encoded
                .chunks_exact(4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .collect();
            assert_eq!(
                fields,
                [
                    coordinate.block.function.0,
                    coordinate.block.block,
                    coordinate.operation
                ]
            );
        }
    }
    assert_eq!(u64::from_le_bytes(bytes[368..376].try_into().unwrap()), 1);
    assert_eq!(
        u64::from_le_bytes(bytes[376..384].try_into().unwrap()),
        bytes.len() as u64
    );
    for offset in 0..bytes.len() {
        let mut bad = Policy7ExecutionWitnessV1 {
            bytes: bytes.to_vec().into_boxed_slice(),
        };
        bad.bytes[offset] ^= 1;
        assert!(
            bad.check(owner, budget).is_err(),
            "accepted changed byte {offset}"
        );
    }
    for length in [0, HEADER - 1, bytes.len() - 1, bytes.len() + 1] {
        let mut changed = bytes.to_vec();
        changed.resize(length, 0);
        assert!(
            Policy7ExecutionWitnessV1 {
                bytes: changed.into_boxed_slice()
            }
            .check(owner, budget)
            .is_err()
        );
    }
    assert_eq!(
        Policy7ExecutionWitnessV1::prepare(owner, budget)
            .unwrap()
            .canonical_bytes(),
        bytes
    );
}

#[cfg(test)]
mod conditional_record_tests {
    use super::*;
    use crate::production_ranked_projection_v1::{
        with_backend_policy7_direct_prefix_v1, with_backend_policy7_erased_prefix_v1,
    };

    fn exercise(owner: Admitted7, parent: &mut Budget<'_>) {
        let floor = parent.storage();
        let (_, checked) = owner.borrowed().portable_prefix();
        let tail = owner.continuation();
        let required = checked.retained_storage() + tail.retained_storage();
        assert!(floor >= required);
        // Diagnostic accounts measure the unchanged encoder, not source or
        // final custody. The actual typed owners stay live on the parent.
        let run = |conditional, work_limit, storage_limit, prepaid| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(prepaid).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let address = std::ptr::from_mut(&mut budget);
            let result = if conditional {
                Policy7ExecutionWitnessV1::prepare_conditional_v1(checked, tail, &mut budget)
            } else {
                Policy7ExecutionWitnessV1::prepare(&owner, &mut budget)
            };
            assert_eq!(budget.storage(), prepaid);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(std::ptr::from_mut(&mut budget), address);
            (result, budget.work(), budget.peak_storage())
        };
        let (ordinary, ordinary_work, ordinary_peak) =
            run(false, 1_000_000_000, 1024 * 1024 * 1024, floor);
        let ordinary = ordinary.unwrap();
        assert_eq!(ordinary_work, 3 * ordinary.canonical_bytes().len() + 14);
        let start = parent.work();
        ordinary.check(&owner, parent).unwrap();
        assert_eq!(parent.work() - start, ordinary.canonical_bytes().len() + 7);
        assert_eq!(parent.storage(), floor);
        let (conditional, conditional_work, conditional_peak) =
            run(true, 1_000_000_000, 1024 * 1024 * 1024, floor);
        assert_eq!(
            conditional.unwrap().canonical_bytes(),
            ordinary.canonical_bytes()
        );
        assert_eq!(conditional_work, ordinary_work + CONDITIONAL_BINDING_WORK);
        assert_eq!(conditional_peak, ordinary_peak);
        for (conditional, work, peak) in [
            (false, ordinary_work, ordinary_peak),
            (true, conditional_work, conditional_peak),
        ] {
            assert!(run(conditional, work, peak, floor).0.is_ok());
            assert!(matches!(
                run(conditional, work - 1, peak, floor).0,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Resource(Resource::Work(_))
                ))
            ));
            assert!(matches!(
                run(conditional, work, peak - 1, floor).0,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    CheckedOutputPolicy7StageErrorV1::Resource(Resource::Storage(_))
                ))
            ));
        }
        let (underpaid, work, peak) = run(true, conditional_work, conditional_peak, required - 1);
        assert!(matches!(
            underpaid,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(work, 0);
        assert_eq!(peak, required - 1);
        assert_eq!(parent.storage(), floor);
    }

    #[test]
    fn conditional_record_preserves_direct_erased_bytes_costs_and_prepaid_floors() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_backend_policy7_direct_prefix_v1(profile, true, |prefix, _, budget| {
                let (owner, storage) = prefix.continue_redundant_private_stores_v1(budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                exercise(Admitted7::Direct(owner), budget);
                budget.release_storage(storage.retained_storage()).unwrap();
            });
            with_backend_policy7_erased_prefix_v1(profile, true, |prefix, _, budget| {
                let (owner, storage) = prefix.continue_redundant_private_stores_v1(budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                exercise(Admitted7::Erased(owner), budget);
                budget.release_storage(storage.retained_storage()).unwrap();
            });
        }
    }

    #[test]
    fn conditional_record_refuses_actual_j_from_a_different_checked_i() {
        with_backend_policy7_direct_prefix_v1(Profile::Gfx942, true, |prefix, _, _| {
            with_backend_policy7_direct_prefix_v1(Profile::Gfx942, false, |other, _, parent| {
                let tail = fe2o3_kernel_opt::prepare_owned_redundant_store_continuation_v1(
                    other.checked_output().owner(),
                    parent,
                )
                .unwrap();
                let retained = tail.retained_storage();
                parent.reserve_storage(retained).unwrap();
                let checked = prefix.checked_output();
                assert_ne!(
                    checked.owner().canonical().identity(),
                    tail.input_identity()
                );
                let required = checked.retained_storage() + tail.retained_storage();
                let mut work = Work::new(1_000_000);
                let mut budget = Budget::new(&mut work, required + 1_000_000);
                budget.reserve_storage(required).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                assert!(matches!(
                    Policy7ExecutionWitnessV1::prepare_conditional_v1(checked, &tail, &mut budget),
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        CheckedOutputPolicy7StageErrorV1::Execution(
                            "conditional Policy7 actual I/J input identity"
                        )
                    ))
                ));
                assert_eq!(budget.work(), CONDITIONAL_BINDING_WORK);
                assert_eq!(budget.storage(), required);
                assert_eq!(budget.peak_storage(), required);
                assert!(budget.work_ledger_identity_v1() == ledger);
                drop(tail);
                parent.release_storage(retained).unwrap();
            });
        });
    }
}
