//! Nominal classification only: the exact producer, brands and scope need separate evidence.

use fe2o3_mir_model::semantic_mir_v1::SemanticExecutionRoleV29;
use rustc_middle::ty::{GenericArgKind, Ty, TyCtxt, TyKind, UintTy};

use crate::trusted_device_items::TrustedDeviceItem;

pub(super) fn role<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Result<SemanticExecutionRoleV29, &'static str> {
    let TyKind::Adt(definition, arguments) = ty.kind() else {
        return Err("execution role requires an authenticated struct");
    };
    if !definition.is_struct()
        || !arguments
            .first()
            .is_some_and(|argument| matches!(argument.kind(), GenericArgKind::Lifetime(_)))
    {
        return Err("execution role has an invalid nominal carrier");
    }
    let type_arguments = |start: usize| {
        arguments[start..]
            .iter()
            .all(|argument| matches!(argument.kind(), GenericArgKind::Type(_)))
    };
    match item {
        TrustedDeviceItem::KernelContext if arguments.len() == 4 && type_arguments(1) => {
            Ok(SemanticExecutionRoleV29::KernelContext)
        }
        TrustedDeviceItem::ExecutionWorkgroupCapability
            if arguments.len() == 3 && type_arguments(1) =>
        {
            Ok(SemanticExecutionRoleV29::Workgroup)
        }
        TrustedDeviceItem::MaskedTile1D | TrustedDeviceItem::LaneFragment1D
            if arguments.len() == 6 && type_arguments(4) =>
        {
            if !arguments[1]
                .as_type()
                .is_some_and(|element| matches!(element.kind(), TyKind::Uint(UintTy::U32)))
            {
                return Err("execution tile role requires the u32 element profile");
            }
            let lanes = arguments[2]
                .as_const()
                .and_then(|value| value.try_to_target_usize(tcx));
            let elements = arguments[3]
                .as_const()
                .and_then(|value| value.try_to_target_usize(tcx));
            let (lanes, elements) = geometry(lanes, elements)?;
            Ok(if item == TrustedDeviceItem::MaskedTile1D {
                SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements }
            } else {
                SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements }
            })
        }
        _ => Err("execution role has unsupported generic argument kinds"),
    }
}

fn geometry(lanes: Option<u64>, elements: Option<u64>) -> Result<(u16, u16), &'static str> {
    match (lanes, elements) {
        (Some(lanes @ 1..=256), Some(elements @ 1..=125)) => Ok((lanes as u16, elements as u16)),
        _ => Err("execution tile role requires 1..=256 lanes and 1..=125 elements"),
    }
}

#[cfg(test)]
mod tests {
    use super::geometry;

    #[test]
    fn geometry_preserves_exact_limits_without_truncation() {
        assert_eq!(geometry(Some(1), Some(1)), Ok((1, 1)));
        assert_eq!(geometry(Some(3), Some(2)), Ok((3, 2)));
        assert_eq!(geometry(Some(256), Some(125)), Ok((256, 125)));
        for (lanes, elements) in [
            (None, Some(1)),
            (Some(1), None),
            (Some(0), Some(1)),
            (Some(1), Some(0)),
            (Some(257), Some(1)),
            (Some(1), Some(126)),
            (Some(u16::MAX as u64 + 2), Some(1)),
            (Some(1), Some(u16::MAX as u64 + 2)),
            (Some(u64::MAX), Some(u64::MAX)),
        ] {
            assert!(
                geometry(lanes, elements).is_err(),
                "{lanes:?}, {elements:?}"
            );
        }
    }
}
