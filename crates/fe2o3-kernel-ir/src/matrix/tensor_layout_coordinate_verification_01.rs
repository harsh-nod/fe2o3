use super::*;

pub(super) fn try_verify_tensor_coordinates_with_sink_v1<S: TensorLayoutFindingSinkV1>(
    fragment: &TensorFragmentLayoutV1,
    expected: &TensorFragmentLayoutV1,
    position: TensorOperandRoleV1,
    subgroup_width: u16,
    sink: &mut S,
) -> Result<(), S::Error> {
    if subgroup_width.min(64) == 64
        && fragment.shape == expected.shape
        && fragment.fragment_elements == expected.fragment_elements
        && fragment.mapping == expected.mapping
        && fragment.multiplicity == expected.multiplicity
    {
        return Ok(());
    }

    let lanes = usize::from(subgroup_width.min(64));
    let components = usize::from(fragment.fragment_elements);
    let samples = lanes.saturating_mul(components);
    sink.charge_work(lanes.saturating_add(samples.saturating_mul(3)))?;
    for lane in 0..lanes {
        for component in 0..components {
            let Some(coordinate) = fragment.logical_coordinate(lane as u16, component as u8) else {
                sink.emit(TensorLayoutFindingV1::MalformedSymbolicMap { role: position })?;
                return Ok(());
            };
            if coordinate[0] >= u64::from(fragment.shape[0])
                || coordinate[1] >= u64::from(fragment.shape[1])
            {
                sink.emit(TensorLayoutFindingV1::CoordinateOutOfBounds { role: position })?;
                return Ok(());
            }
        }
    }

    // Validate first so malformed/out-of-bounds precedence is unchanged. The
    // second traversal stores one exact coordinate per sample, not per domain
    // point; hostile shape dimensions cannot inflate scratch allocation.
    let mut coordinates = sink.allocate_coordinates(samples)?;
    let result = (|| {
        sink.charge_work(samples.saturating_mul(3))?;
        for lane in 0..lanes {
            for component in 0..components {
                let Some(coordinate) = fragment.logical_coordinate(lane as u16, component as u8)
                else {
                    sink.emit(TensorLayoutFindingV1::MalformedSymbolicMap { role: position })?;
                    return Ok(());
                };
                coordinates.push(coordinate);
            }
        }
        sort_coordinates_v1(&mut coordinates, sink)?;
        sink.charge_work(samples.saturating_mul(4).saturating_add(1))?;
        let mut unique_coordinates = 0_usize;
        let mut has_duplicate = false;
        let mut bad_count = false;
        let mut index = 0_usize;
        while index < coordinates.len() {
            let mut end = index + 1;
            while end < coordinates.len() && coordinates[end] == coordinates[index] {
                end += 1;
            }
            let count = end - index;
            unique_coordinates += 1;
            has_duplicate |= count != 1;
            if let TensorMultiplicityV1::Broadcast { factor } = fragment.multiplicity {
                bad_count |= count != usize::from(factor);
            }
            index = end;
        }
        let expected_coordinates =
            usize::from(fragment.shape[0]).saturating_mul(usize::from(fragment.shape[1]));
        match fragment.multiplicity {
            TensorMultiplicityV1::Unique => {
                if has_duplicate {
                    sink.emit(TensorLayoutFindingV1::DuplicateCoordinate { role: position })?;
                }
                if unique_coordinates != expected_coordinates {
                    sink.emit(TensorLayoutFindingV1::IncompleteCoverage { role: position })?;
                }
            }
            TensorMultiplicityV1::Broadcast { factor } => {
                if factor == 0 || unique_coordinates != expected_coordinates || bad_count {
                    sink.emit(TensorLayoutFindingV1::BroadcastContractMismatch { role: position })?;
                }
            }
        }
        Ok(())
    })();
    let released = sink.release_coordinates(coordinates, samples);
    result.and(released)
}

fn sort_coordinates_v1<S: TensorLayoutFindingSinkV1>(
    coordinates: &mut [[u64; 2]],
    sink: &mut S,
) -> Result<(), S::Error> {
    let count = coordinates.len();
    if count < 2 {
        return Ok(());
    }
    let height = usize::BITS as usize - (count - 1).leading_zeros() as usize;
    // Fewer than 3*n*height/2 sift levels, each with two two-word comparisons
    // and a four-cell swap, plus fewer than n root swaps. 16*n*height
    // bounds those scalar comparisons/publications before sort mutation.
    sink.charge_work(count.saturating_mul(height).saturating_mul(16))?;
    for start in (0..count / 2).rev() {
        sift_coordinates_v1(coordinates, start, count);
    }
    for end in (1..count).rev() {
        coordinates.swap(0, end);
        sift_coordinates_v1(coordinates, 0, end);
    }
    Ok(())
}

fn sift_coordinates_v1(coordinates: &mut [[u64; 2]], mut root: usize, end: usize) {
    loop {
        let child = root * 2 + 1;
        if child >= end {
            return;
        }
        let right = child + 1;
        let selected = if right < end && coordinates[child] < coordinates[right] {
            right
        } else {
            child
        };
        if coordinates[root] >= coordinates[selected] {
            return;
        }
        coordinates.swap(root, selected);
        root = selected;
    }
}

#[cfg(test)]
#[path = "tensor_layout_coordinate_verification_02_tests.rs"]
mod tests;
