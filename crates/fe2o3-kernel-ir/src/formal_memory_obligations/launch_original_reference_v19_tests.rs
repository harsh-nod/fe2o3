fn original_resolve_invocations_v19(
    domain: &LaunchDomain,
    launch_extent: ExplicitLaunchExtent,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
    interpretation: PhysicalLaunchInterpretationV2,
) -> Result<Option<InvocationRange1d>, FormalMemoryObligationError> {
    let ExplicitLaunchExtent::Exact { rank, extents } = launch_extent else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentUnknown);
        return Ok(None);
    };
    if !(1..=3).contains(&rank) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchRankUnsupported { rank });
        return Ok(None);
    }
    if domain.rank() != rank {
        reasons.insert(FormalMemoryIncompleteReason::LaunchRankMismatch {
            domain_rank: domain.rank(),
            extent_rank: rank,
        });
        return Ok(None);
    }
    if (rank < 2 && extents[1] != 1) || (rank < 3 && extents[2] != 1) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentShapeMismatch { rank, extents });
        return Ok(None);
    }
    if extents.contains(&0) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentZero);
        return Ok(None);
    }
    for (index, expected) in domain.extents().enumerate() {
        let LaunchExtent::Static(expected) = expected else {
            continue;
        };
        let actual = extents[index];
        let covered = match interpretation {
            PhysicalLaunchInterpretationV2::Exact => u64::from(expected) == actual,
            PhysicalLaunchInterpretationV2::Envelope => u64::from(expected) <= actual,
        };
        if covered {
            continue;
        }
        if rank == 1 {
            reasons.insert(FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
                expected,
                actual,
            });
        } else {
            reasons.insert(
                FormalMemoryIncompleteReason::StaticLaunchAxisExtentMismatch {
                    axis: [Axis::X, Axis::Y, Axis::Z][index],
                    expected,
                    actual,
                },
            );
        }
        return Ok(None);
    }
    let Some(count) = extents[..usize::from(rank)]
        .iter()
        .try_fold(1_u64, |count, extent| count.checked_mul(*extent))
    else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentOverflow { rank, extents });
        return Ok(None);
    };
    InvocationRange1d::from_count(count)
        .map(Some)
        .map_err(FormalMemoryObligationError::InvalidInvocationRange)
}
