use super::*;

/// Operand-independent Execution identity. SSA occurrences, not these fields,
/// establish source, scope and producer authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticExecutionInstancePayloadV3 {
    ContextIssue,
    WorkgroupDerive,
    ScopeEnd { discard_count: u32 },
    MaskedTileLoadU32 { lanes: u16, elements: u16 },
    TileIntoFragmentU32 { lanes: u16, elements: u16 },
    FragmentIntoPartsU32 { lanes: u16, elements: u16 },
}

impl SemanticExecutionInstancePayloadV3 {
    pub const fn kind(self) -> SemanticOperationKind {
        match self {
            Self::ContextIssue => SemanticOperationKind::ExecutionContextIssue,
            Self::WorkgroupDerive => SemanticOperationKind::ExecutionWorkgroupDerive,
            Self::ScopeEnd { .. } => SemanticOperationKind::ExecutionScopeEnd,
            Self::MaskedTileLoadU32 { .. } => SemanticOperationKind::ExecutionMaskedTileLoadU32,
            Self::TileIntoFragmentU32 { .. } => SemanticOperationKind::ExecutionTileIntoFragmentU32,
            Self::FragmentIntoPartsU32 { .. } => {
                SemanticOperationKind::ExecutionFragmentIntoPartsU32
            }
        }
    }

    pub(super) const fn encoded_len(self) -> usize {
        match self {
            Self::ContextIssue | Self::WorkgroupDerive => 0,
            _ => 4,
        }
    }

    fn validate(self) -> Result<(), ExecutionOperationErrorV15> {
        match self {
            Self::ContextIssue | Self::WorkgroupDerive => Ok(()),
            Self::ScopeEnd { discard_count } => {
                let max = crate::MAX_VALUE_ARGUMENTS_V1 - 1;
                if u64::from(discard_count) > max as u64 {
                    return Err(ExecutionOperationErrorV15::DiscardLimitExceeded {
                        actual: discard_count as usize,
                        max,
                    });
                }
                Ok(())
            }
            Self::MaskedTileLoadU32 { lanes, elements }
            | Self::TileIntoFragmentU32 { lanes, elements }
            | Self::FragmentIntoPartsU32 { lanes, elements } => {
                ExecutionRoleV15::MaskedTileU32 { lanes, elements }.validate()
            }
        }
    }
}

impl SemanticOperationInstanceId {
    /// Constructs only a bounded SO3 identity; it does not authenticate a graph.
    pub fn execution_v3(
        payload: SemanticExecutionInstancePayloadV3,
    ) -> Result<Self, ExecutionOperationErrorV15> {
        payload.validate()?;
        Ok(Self {
            schema: SemanticOperationSchema {
                version: SEMANTIC_OPERATION_VERSION_V3,
                kind: payload.kind(),
            },
            payload: SemanticOperationInstancePayloadV1::Execution(payload),
        })
    }
}

impl ExecutionOperationV15 {
    /// Invalid raw IR returns an error before descriptor construction.
    pub fn semantic_instance_id_v3(
        &self,
    ) -> Result<SemanticOperationInstanceId, ExecutionOperationErrorV15> {
        self.validate_payload()?;
        let payload = match self {
            Self::ContextIssue => SemanticExecutionInstancePayloadV3::ContextIssue,
            Self::WorkgroupDerive { .. } => SemanticExecutionInstancePayloadV3::WorkgroupDerive,
            Self::ScopeEnd { discarded, .. } => SemanticExecutionInstancePayloadV3::ScopeEnd {
                discard_count: u32::try_from(discarded.len()).map_err(|_| {
                    ExecutionOperationErrorV15::DiscardLimitExceeded {
                        actual: discarded.len(),
                        max: crate::MAX_VALUE_ARGUMENTS_V1 - 1,
                    }
                })?,
            },
            Self::MaskedTileLoadU32 {
                lanes, elements, ..
            } => SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 {
                lanes: *lanes,
                elements: *elements,
            },
            Self::TileIntoFragmentU32 {
                lanes, elements, ..
            } => SemanticExecutionInstancePayloadV3::TileIntoFragmentU32 {
                lanes: *lanes,
                elements: *elements,
            },
            Self::FragmentIntoPartsU32 {
                lanes, elements, ..
            } => SemanticExecutionInstancePayloadV3::FragmentIntoPartsU32 {
                lanes: *lanes,
                elements: *elements,
            },
        };
        SemanticOperationInstanceId::execution_v3(payload)
    }

    /// Physical shape/effects only; optimization must also retain compiler order.
    /// The metered verifier checks borrowed types without constructing this Vec.
    pub fn try_contract_v3(&self) -> Result<SemanticOperationContract, ExecutionOperationErrorV15> {
        let instance = self.semantic_instance_id_v3()?;
        let (operand_count, results) = match self {
            Self::ContextIssue => (0, vec![Type::Execution(ExecutionRoleV15::Context)]),
            Self::WorkgroupDerive { .. } => (1, vec![Type::Execution(ExecutionRoleV15::Workgroup)]),
            Self::ScopeEnd { discarded, .. } => (1 + discarded.len(), Vec::new()),
            Self::MaskedTileLoadU32 {
                lanes, elements, ..
            } => (
                3,
                vec![Type::Execution(ExecutionRoleV15::MaskedTileU32 {
                    lanes: *lanes,
                    elements: *elements,
                })],
            ),
            Self::TileIntoFragmentU32 {
                lanes, elements, ..
            } => (
                1,
                vec![Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
                    lanes: *lanes,
                    elements: *elements,
                })],
            ),
            Self::FragmentIntoPartsU32 { elements, .. } => {
                let elements = usize::from(*elements);
                let mut results = vec![Type::Scalar(ScalarType::U32); elements];
                results.extend(std::iter::repeat_n(Type::BOOL, elements));
                (1, results)
            }
        };
        let memory = if matches!(self, Self::MaskedTileLoadU32 { .. }) {
            vec![MemoryEffect::Read(AddressSpace::Global)]
        } else {
            Vec::new()
        };
        Ok(SemanticOperationContract::new(
            instance,
            operand_count,
            results,
            memory,
            BTreeSet::new(),
        ))
    }
}

pub(super) fn encode_execution_payload_v3(payload: SemanticExecutionInstancePayloadV3) -> Vec<u8> {
    match payload {
        SemanticExecutionInstancePayloadV3::ContextIssue
        | SemanticExecutionInstancePayloadV3::WorkgroupDerive => Vec::new(),
        SemanticExecutionInstancePayloadV3::ScopeEnd { discard_count } => {
            discard_count.to_le_bytes().to_vec()
        }
        SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 { lanes, elements }
        | SemanticExecutionInstancePayloadV3::TileIntoFragmentU32 { lanes, elements }
        | SemanticExecutionInstancePayloadV3::FragmentIntoPartsU32 { lanes, elements } => {
            let mut bytes = Vec::with_capacity(4);
            bytes.extend_from_slice(&lanes.to_le_bytes());
            bytes.extend_from_slice(&elements.to_le_bytes());
            bytes
        }
    }
}

pub(super) fn decode_execution_payload_v3(
    kind: SemanticOperationKind,
    bytes: &[u8],
) -> Result<SemanticOperationInstanceId, SemanticOperationInstanceDecodeError> {
    let invalid = || SemanticOperationInstanceDecodeError::InvalidContract { kind };
    let payload = match kind {
        SemanticOperationKind::ExecutionContextIssue => {
            SemanticExecutionInstancePayloadV3::ContextIssue
        }
        SemanticOperationKind::ExecutionWorkgroupDerive => {
            SemanticExecutionInstancePayloadV3::WorkgroupDerive
        }
        SemanticOperationKind::ExecutionScopeEnd => SemanticExecutionInstancePayloadV3::ScopeEnd {
            discard_count: u32::from_le_bytes(bytes.try_into().map_err(|_| invalid())?),
        },
        SemanticOperationKind::ExecutionMaskedTileLoadU32
        | SemanticOperationKind::ExecutionTileIntoFragmentU32
        | SemanticOperationKind::ExecutionFragmentIntoPartsU32 => {
            let [l0, l1, e0, e1] = <[u8; 4]>::try_from(bytes).map_err(|_| invalid())?;
            let lanes = u16::from_le_bytes([l0, l1]);
            let elements = u16::from_le_bytes([e0, e1]);
            match kind {
                SemanticOperationKind::ExecutionMaskedTileLoadU32 => {
                    SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 { lanes, elements }
                }
                SemanticOperationKind::ExecutionTileIntoFragmentU32 => {
                    SemanticExecutionInstancePayloadV3::TileIntoFragmentU32 { lanes, elements }
                }
                _ => SemanticExecutionInstancePayloadV3::FragmentIntoPartsU32 { lanes, elements },
            }
        }
        _ => return Err(invalid()),
    };
    SemanticOperationInstanceId::execution_v3(payload).map_err(|_| invalid())
}
