//! The same value grammar for recipe operands and detached source coordinates.
use super::*;

/// Largest encoded ranked value: one tag and two little-endian u32 coordinates.
pub const MAX_PRODUCTION_RANKED_VALUE_BYTES_V1: usize = 9;

/// Encodes the recipe's V1 value leaf into caller-owned storage, returning its length.
/// Bytes after that length are unchanged. Work is charged; no storage is allocated.
/// This transports a coordinate, not evidence that the referenced value exists.
pub fn encode_production_ranked_value_v1(
    value: ProductionRankedValueV1,
    output: &mut [u8; MAX_PRODUCTION_RANKED_VALUE_BYTES_V1],
    budget: &mut Budget<'_>,
) -> Result<usize, WireError> {
    let mut writer = Encoder::new(Mode::Fill(output), budget);
    value.emit(&mut writer)?;
    Ok(writer.offset)
}

/// Reads exactly one V1 value prefix and returns the consumed byte count.
/// The enclosing schema must check its own frame's end. No storage is allocated
/// and no reference resolution or proof import occurs.
pub fn decode_production_ranked_value_prefix_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<(ProductionRankedValueV1, usize), WireError> {
    let mut resolver = NoProof;
    let mut reader = Decoder::new(bytes, &mut resolver, budget);
    match ProductionRankedValueV1::read(&mut reader) {
        Ok(value) => Ok((value, reader.offset)),
        Err(DecodeError::Wire(error) | DecodeError::Resolver(error)) => Err(error),
    }
}

struct NoProof;
impl Resolver for NoProof {
    type Error = WireError;
    fn resolve(
        &mut self,
        _: ProductionRankedRecipeProofClaimV1,
        _: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<ProductionReferenceProofV2, Self::Error> {
        Err(WireError::Invalid("proof in value leaf"))
    }
    fn finish(
        &mut self,
        _: u32,
        _: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn exact_value_leaf_bytes_prefix_lengths_and_work() {
        for (value, golden, fee) in [
            (
                ProductionRankedValueV1::Argument(u32::MAX),
                vec![1, 255, 255, 255, 255],
                7,
            ),
            (
                ProductionRankedValueV1::BlockArgument {
                    block: 0x04030201,
                    argument: 5,
                },
                vec![2, 1, 2, 3, 4, 5, 0, 0, 0],
                12,
            ),
            (
                ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(6)),
                vec![3, 6, 0, 0, 0],
                7,
            ),
        ] {
            for limit in [fee - 1, fee] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 0);
                let mut output = [99; 9];
                let encoded = encode_production_ranked_value_v1(value, &mut output, &mut budget);
                assert_eq!(encoded.is_ok(), limit == fee);
                if let Ok(length) = encoded {
                    assert_eq!(output[..length], golden);
                    assert!(output[length..].iter().all(|byte| *byte == 99));
                    assert_eq!(budget.work(), fee);
                }
                assert_eq!(budget.peak_storage(), 0);
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 0);
                let mut input = golden.clone();
                input.push(255);
                let decoded = decode_production_ranked_value_prefix_v1(&input, &mut budget);
                assert_eq!(decoded.is_ok(), limit == fee);
                if let Ok(decoded) = decoded {
                    assert_eq!(decoded, (value, golden.len()));
                }
            }
            for end in 0..golden.len() {
                let mut work = Work::new(100);
                let mut budget = Budget::new(&mut work, 0);
                assert!(
                    decode_production_ranked_value_prefix_v1(&golden[..end], &mut budget).is_err()
                );
            }
        }
        for tag in [0, 4, 255] {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 0);
            assert!(
                matches!(decode_production_ranked_value_prefix_v1(&[tag], &mut budget), Err(WireError::UnknownTag { field: "ranked value", tag: actual }) if actual == tag)
            );
        }
    }
}
