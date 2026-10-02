//! Independent finite-width specification for the mixed tile CPU example.

/// Diagnostic distribution of three input positions across 64 logical lanes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedTileOrderV1 {
    /// Lane l receives positions 3*l, 3*l+1 and 3*l+2.
    Blocked,
    /// Lane l receives positions l, 64+l and 128+l.
    Striped,
}

/// Input offset and complete-workgroup launch for this example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedTileConfigV1 {
    /// Finite-width 64-bit element offset of the input tile.
    pub base: u64,
    /// Number of complete 64-lane workgroups in the launch.
    pub workgroups: u32,
    /// Per-lane element distribution, not a schedule-equivalence assertion.
    pub order: MixedTileOrderV1,
}

/// Invalid host oracle arguments, rejected before any output mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedTileErrorV1 {
    /// A zero or unrepresentable invocation count was requested.
    Launch,
    /// The per-element initialization view does not match the output length.
    Initialization,
}

/// Applies the mathematical weighted sum modulo 2^32 to active output lanes.
///
/// Uses u128 arithmetic and ordinary slices, without device types, intrinsics,
/// lowering helpers or the kernel body. Values outside finite-width input bounds
/// contribute nothing. Workgroups revisit the same input tile but have distinct
/// global output indices. Short outputs, inactive lanes and output beyond the
/// launch retain both their original bytes and initialization state.
pub fn mixed_tile_oracle_v1(
    values: &[u32],
    config: MixedTileConfigV1,
    output: &mut [u32],
    initialized: &mut [bool],
) -> Result<(), MixedTileErrorV1> {
    let invocations = config
        .workgroups
        .checked_mul(64)
        .filter(|&count| count != 0)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(MixedTileErrorV1::Launch)?;
    if initialized.len() != output.len() {
        return Err(MixedTileErrorV1::Initialization);
    }
    for (global_lane, (slot, ready)) in output
        .iter_mut()
        .zip(initialized.iter_mut())
        .take(invocations)
        .enumerate()
    {
        let lane = (global_lane % 64) as u128;
        let mut sum = 0_u128;
        let mut active = false;
        for (element, (weight, bias)) in [(3_u128, 11_u128), (5, 13), (7, 17)]
            .into_iter()
            .enumerate()
        {
            let position = match config.order {
                MixedTileOrderV1::Blocked => lane * 3 + element as u128,
                MixedTileOrderV1::Striped => element as u128 * 64 + lane,
            };
            let index = u128::from(config.base) + position;
            if index <= u128::from(u64::MAX) && index < values.len() as u128 {
                sum += u128::from(values[index as usize]) * weight + bias;
                active = true;
            }
        }
        if active {
            *slot = (sum & u128::from(u32::MAX)) as u32;
            *ready = true;
        }
    }
    Ok(())
}
