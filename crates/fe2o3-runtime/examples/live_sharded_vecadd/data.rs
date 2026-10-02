use super::*;

pub(super) const TOTAL_ELEMENTS: usize = 65_537;
pub(super) const PAGE_BYTES: usize = 4096;
pub(super) const MAX_SNAPSHOT_BYTES: usize = 286_720;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Shard {
    pub offset: usize,
    pub elements: usize,
    pub padded_bytes: usize,
}

impl Shard {
    pub fn logical_bytes(self) -> usize {
        self.elements * 4
    }

    pub fn geometry(self) -> RuntimeLaunchGeometryV1 {
        RuntimeLaunchGeometryV1 {
            grid: [(self.elements.div_ceil(256) * 256) as u32, 1, 1],
            workgroup: [256, 1, 1],
            dynamic_shared_bytes: 0,
        }
    }
}

pub(super) fn unique_ids(arguments: &[String]) -> ResultV1<Vec<u64>> {
    if !(2..=8).contains(&arguments.len()) {
        return Err(USAGE.into());
    }
    let mut ids = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let hex = argument
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        let id = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
        if id == 0 || ids.contains(&id) {
            return Err(USAGE.into());
        }
        ids.push(id);
    }
    Ok(ids)
}

pub(super) fn partition(count: usize) -> ResultV1<Vec<Shard>> {
    if !(2..=8).contains(&count) {
        return Err("two to eight shards required".into());
    }
    let mut offset = 0;
    let mut shards = Vec::with_capacity(count);
    for index in 0..count {
        let elements = TOTAL_ELEMENTS / count + usize::from(index < TOTAL_ELEMENTS % count);
        let padded_bytes = (elements * 4).div_ceil(PAGE_BYTES) * PAGE_BYTES;
        shards.push(Shard {
            offset,
            elements,
            padded_bytes,
        });
        offset += elements;
    }
    if offset != TOTAL_ELEMENTS
        || snapshot_extent(&shards.iter().map(|s| s.padded_bytes).collect::<Vec<_>>()).is_err()
    {
        return Err("invalid fixed-total partition".into());
    }
    Ok(shards)
}

pub(super) fn recipe(count: usize, index: usize, round: usize, shard: Shard) -> ResultV1<Recipe> {
    let recipe = Recipe::new(count, index, round).map_err(|error| failure("recipe", error))?;
    if recipe.count() != count
        || recipe.index() != index
        || recipe.round() != round
        || recipe.global_offset() != shard.offset
        || recipe.elements() != shard.elements
        || recipe.padded_bytes() != shard.padded_bytes
        || recipe.geometry() != shard.geometry()
    {
        return Err(failure("recipe", "independent partition differs"));
    }
    Ok(recipe)
}

fn filled(value: f32, bytes: usize) -> Vec<u8> {
    value.to_bits().to_le_bytes().repeat(bytes / 4)
}

pub(super) fn initial_buffers(shard: Shard, round: usize) -> [Vec<u8>; 3] {
    let mut a = filled(-7.0, shard.padded_bytes);
    let mut b = filled(-11.0, shard.padded_bytes);
    for index in 0..shard.elements {
        let start = index * 4;
        a[start..start + 4]
            .copy_from_slice(&((shard.offset + index + 131_072 * round) as f32).to_le_bytes());
        b[start..start + 4].copy_from_slice(&((3 + round) as f32).to_le_bytes());
    }
    [a, b, filled(-1.0, shard.padded_bytes)]
}

pub(super) fn sentinel(shard: Shard) -> Vec<u8> {
    let mut bytes = filled(-19.0, shard.padded_bytes);
    bytes[..shard.logical_bytes()].copy_from_slice(&filled(-17.0, shard.logical_bytes()));
    bytes
}

fn expected(global: usize, round: usize) -> [u8; 4] {
    ((global + 3 + 131_073 * round) as f32).to_le_bytes()
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("String formatting");
    }
    output
}

pub(super) fn snapshot_extent(lengths: &[usize]) -> ResultV1<usize> {
    if !(2..=8).contains(&lengths.len()) || lengths.contains(&0) {
        return Err("invalid snapshot roster".into());
    }
    lengths.iter().try_fold(0_usize, |sum, length| {
        sum.checked_add(*length)
            .filter(|total| *total <= MAX_SNAPSHOT_BYTES)
            .ok_or_else(|| "snapshot byte bound exceeded".into())
    })
}

// Partial bytes never escape when one of the settled reads fails.
pub(super) fn snapshot(
    lengths: &[usize],
    mut read: impl FnMut(usize, &mut [u8]) -> ResultV1<()>,
) -> ResultV1<Vec<u8>> {
    let mut bytes = vec![0; snapshot_extent(lengths)?];
    let mut cursor = 0;
    for (index, length) in lengths.iter().enumerate() {
        read(index, &mut bytes[cursor..cursor + length])?;
        cursor += length;
    }
    Ok(bytes)
}

pub(super) fn verify_snapshot(
    bytes: &[u8],
    shards: &[Shard],
    round: usize,
) -> ResultV1<(String, String)> {
    let lengths: Vec<_> = shards.iter().map(|shard| shard.padded_bytes).collect();
    if bytes.len() != snapshot_extent(&lengths)? || round > 1 {
        return Err("snapshot extent or round differs".into());
    }
    let mut logical = Vec::with_capacity(TOTAL_ELEMENTS * 4);
    let mut cursor = 0;
    for shard in shards {
        let observed = &bytes[cursor..cursor + shard.padded_bytes];
        for (index, word) in observed.chunks_exact(4).enumerate() {
            let wanted = if index < shard.elements {
                expected(shard.offset + index, round)
            } else {
                (-1.0_f32).to_le_bytes()
            };
            if word != wanted {
                return Err(format!(
                    "full output differs at global shard {} word {index}",
                    shard.offset
                ));
            }
        }
        logical.extend_from_slice(&observed[..shard.logical_bytes()]);
        cursor += shard.padded_bytes;
    }
    let reference: Vec<_> = (0..TOTAL_ELEMENTS)
        .flat_map(|global| expected(global, round))
        .collect();
    if logical != reference {
        return Err("global gathered coverage differs".into());
    }
    Ok((hex_digest(&logical), hex_digest(bytes)))
}

pub(super) struct Receipts<I = RuntimeSubmissionIdV1> {
    observations: Vec<Option<(I, RuntimeCompletionStatusV1)>>,
    deliveries: Vec<usize>,
}

impl<I: Copy + Eq> Receipts<I> {
    pub fn new(count: usize) -> Self {
        Self {
            observations: vec![None; count],
            deliveries: vec![0; count],
        }
    }

    pub fn record(&mut self, index: usize, id: I, status: RuntimeCompletionStatusV1) {
        self.deliveries[index] = self.deliveries[index].saturating_add(1);
        self.observations[index] = Some((id, status));
    }

    pub fn succeeded(&self, start: usize, expected: &[I]) -> bool {
        start
            .checked_add(expected.len())
            .is_some_and(|end| end <= self.observations.len())
            && expected.iter().enumerate().all(|(index, &id)| {
                self.deliveries[start + index] == 1
                    && self.observations[start + index]
                        == Some((id, RuntimeCompletionStatusV1::Succeeded))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(shards: &[Shard], round: usize) -> Vec<u8> {
        shards
            .iter()
            .flat_map(|shard| {
                let mut bytes: Vec<_> = (shard.offset..shard.offset + shard.elements)
                    .flat_map(|global| expected(global, round))
                    .collect();
                bytes.extend(filled(-1.0, shard.padded_bytes - bytes.len()));
                bytes
            })
            .collect()
    }

    #[test]
    fn cli_requires_two_to_eight_explicit_distinct_nonzero_ids() {
        for count in [2, 3, 5, 8] {
            let arguments: Vec<_> = (1..=count).map(|id| format!("0x{id:x}")).collect();
            assert_eq!(
                unique_ids(&arguments).unwrap(),
                (1..=count as u64).collect::<Vec<_>>()
            );
        }
        for arguments in [
            vec![],
            vec!["0x1"],
            vec!["0x0", "0x1"],
            vec!["0x1", "0x01"],
            vec!["1", "0x2"],
            vec!["0x", "0x2"],
            vec!["--round", "0"],
            vec!["0x10000000000000000", "0x2"],
        ] {
            assert!(
                unique_ids(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
        assert!(unique_ids(&(1..=9).map(|id| format!("0x{id:x}")).collect::<Vec<_>>()).is_err());
    }

    #[test]
    fn both_batches_preserve_exact_uneven_partition_and_finite_recipes() {
        for count in 2..=8 {
            let shards = partition(count).unwrap();
            assert_eq!(
                shards.iter().map(|shard| shard.elements).sum::<usize>(),
                TOTAL_ELEMENTS
            );
            assert!(
                shards
                    .windows(2)
                    .all(|pair| pair[0].offset + pair[0].elements == pair[1].offset)
            );
            for (index, &shard) in shards.iter().enumerate() {
                for round in 0..2 {
                    let recipe = recipe(count, index, round, shard).unwrap();
                    let buffers = recipe.host_buffers().unwrap();
                    let [a, b, c] = initial_buffers(shard, round);
                    assert_eq!(a, buffers.a());
                    assert_eq!(b, buffers.b());
                    assert_eq!(c, buffers.c_initial());
                }
            }
        }
        assert!(partition(1).is_err());
        assert!(partition(9).is_err());
    }

    #[test]
    fn independent_full_outputs_and_padding_change_across_batches() {
        for count in [2, 3, 5, 8] {
            let shards = partition(count).unwrap();
            let first = verify_snapshot(&output(&shards, 0), &shards, 0).unwrap();
            let second = verify_snapshot(&output(&shards, 1), &shards, 1).unwrap();
            assert_eq!(
                first.0,
                "d394636ce117fedd8452e72f433c276198314fa52eb1c464449a894505a4103c"
            );
            assert_eq!(
                second.0,
                "297de3086d3bd8894cf3c46925e48b5e6e1d3a16f183c6682ea96f4f3772b866"
            );
            assert_ne!(first, second);
            assert!(verify_snapshot(&output(&shards, 0), &shards, 1).is_err());
            let mut wrong = output(&shards, 0);
            wrong[shards[0].logical_bytes()] ^= 1;
            assert!(verify_snapshot(&wrong, &shards, 0).is_err());
        }
    }

    #[test]
    fn snapshot_preflights_bound_and_discards_partial_failure() {
        let mut calls = 0;
        assert!(
            snapshot(&[MAX_SNAPSHOT_BYTES, 1], |_, _| {
                calls += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert!(snapshot_extent(&[usize::MAX, 1]).is_err());
        assert!(snapshot_extent(&[0, 8]).is_err());
        let result = snapshot(&[8, 8, 8], |index, bytes| {
            calls += 1;
            bytes.fill(31);
            if index == 1 {
                Err("second read failed".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("second read failed".into()));
        assert_eq!(calls, 2);
    }

    #[test]
    fn callbacks_bind_six_n_exact_ids_once_across_both_batches() {
        let mut receipts = Receipts::new(12);
        let ids: Vec<u64> = (100..112).collect();
        for (index, &id) in ids.iter().enumerate() {
            receipts.record(index, id, RuntimeCompletionStatusV1::Succeeded);
        }
        assert!(receipts.succeeded(0, &ids[..6]));
        assert!(receipts.succeeded(6, &ids[6..]));
        assert!(!receipts.succeeded(6, &ids[..6]));
        receipts.observations[7] =
            Some((ids[7], RuntimeCompletionStatusV1::QuiescentWithoutResult));
        assert!(!receipts.succeeded(6, &ids[6..]));
        receipts.observations[7] = Some((ids[7], RuntimeCompletionStatusV1::Succeeded));
        receipts.record(6, ids[6], RuntimeCompletionStatusV1::Succeeded);
        assert!(!receipts.succeeded(6, &ids[6..]));
        assert!(!receipts.succeeded(usize::MAX, &ids));
    }
}
