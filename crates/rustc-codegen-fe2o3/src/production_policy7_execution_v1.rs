//! Exact in-process schedule observation. No decoder, source or proof authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 as Coordinate;

pub(super) const HEADER: usize = 384;
const ROW: usize = 24;

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
    pub(super) fn retained_storage(&self) -> usize {
        size_of::<Self>() + self.bytes.len()
    }

    pub(super) fn prepare(owner: &Admitted7, budget: &mut Budget<'_>) -> Result7<Self> {
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
            witness.check(owner, budget)?;
            Ok(witness)
        })
    }

    pub(super) fn check(&self, owner: &Admitted7, budget: &mut Budget<'_>) -> Result7<()> {
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

fn extent(owner: &Admitted7) -> Result7<usize> {
    extent_counts(
        owner.continuation().rows().len(),
        owner.continuation().retained_operations().len(),
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
    owner: &Admitted7,
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
    let continuation = owner.continuation();
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(b"F2P7EX1\0");
    header[8..10].copy_from_slice(&1u16.to_le_bytes());
    header[10..12].copy_from_slice(&7u16.to_le_bytes());
    header[12..16].copy_from_slice(&256u32.to_le_bytes());
    header[16..272].copy_from_slice(owner.prefix_record());
    for (offset, graph) in [(272, owner.input()), (312, owner.output())] {
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
