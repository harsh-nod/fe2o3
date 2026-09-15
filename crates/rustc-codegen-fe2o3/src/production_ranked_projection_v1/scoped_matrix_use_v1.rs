//! The use relation supplies custody only. Existing typed load, layout, memory,
//! numerical and final-reference checks remain in their existing consumers.
use super::*;
pub(super) use fe2o3_lower_mir_kernel::{
    ProductionScopedMatrixUseRelationV1 as Relation, ProductionScopedMatrixUseV1 as Use,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Issuer {
    Legacy(u64),
    Scoped(DigestV1),
}

impl Issuer {
    pub(super) fn digest(self, kind: u8) -> DigestV1 {
        match self {
            Self::Legacy(root) => tensor_capability_root_v1(kind, &[root]),
            Self::Scoped(root) => root,
        }
    }
}

pub(super) fn at(
    relation: Option<&Relation<'_>>,
    function: &SemanticFunctionDeclV1,
    block: usize,
    call: &SemanticDirectCallV1,
) -> Result<Option<Use>, ProductionRankedProjectionErrorV1> {
    relation
        .map(|r| r.use_at(function, block as u32, call))
        .transpose()
        .map(Option::flatten)
        .map_err(ProductionRankedProjectionErrorV1::StructuralValidation)
}

pub(super) fn lane(
    call: &SemanticDirectCallV1,
    state: &ProjectedCapabilityStateV1,
    operand: usize,
    scoped: Option<Use>,
) -> Option<(Issuer, u32)> {
    if let Some(scoped) = scoped {
        return (scoped.operand() == operand && scoped.matrix_identity().is_none()).then(|| {
            (
                Issuer::Scoped(DigestV1::from_untrusted_bytes(scoped.lane_identity())),
                64,
            )
        });
    }
    match capability_known_origin_v1(state, call.arguments().get(operand)?)? {
        ProjectedCapabilityOriginV1::Lane { root, wave_width } => {
            Some((Issuer::Legacy(root), wave_width))
        }
        _ => None,
    }
}

pub(super) fn context(
    call: &SemanticDirectCallV1,
    state: &ProjectedCapabilityStateV1,
    scoped: Option<Use>,
) -> Option<(Issuer, Option<Issuer>)> {
    if let Some(scoped) = scoped {
        if scoped.operand() != 0 {
            return None;
        }
        return Some((
            Issuer::Scoped(DigestV1::from_untrusted_bytes(scoped.matrix_identity()?)),
            Some(Issuer::Scoped(DigestV1::from_untrusted_bytes(
                scoped.lane_identity(),
            ))),
        ));
    }
    match capability_known_origin_v1(state, call.arguments().first()?)? {
        ProjectedCapabilityOriginV1::MatrixContext { root } => Some((Issuer::Legacy(root), None)),
        _ => None,
    }
}

pub(super) fn operand_root(operand: ProjectedMfmaOperandV1) -> DigestV1 {
    let kind = match operand.contract.role {
        SemanticMfmaOperandRoleV1::A => 3,
        SemanticMfmaOperandRoleV1::B => 4,
    };
    let fields = [
        operand.allocation.allocation_origin,
        operand.allocation.noalias_class,
        u64::from(operand.allocation.writable),
        match operand.storage_layout {
            SemanticMfmaStorageLayoutV1::RowMajor => 1,
            SemanticMfmaStorageLayoutV1::LdsXor4 => 2,
        },
    ];
    match operand.lane_root {
        Issuer::Legacy(lane) => {
            tensor_capability_root_v1(kind, &[lane, fields[0], fields[1], fields[2], fields[3]])
        }
        Issuer::Scoped(lane) => {
            let mut digest = Sha256::new();
            digest.update(b"fe2o3.scoped-matrix-operand.v1");
            digest.update([kind]);
            digest.update(lane.as_bytes());
            for field in fields {
                digest.update(field.to_le_bytes());
            }
            DigestV1::from_untrusted_bytes(digest.finalize().into())
        }
    }
}
