use crate::{
    ArtifactIdentityV1, ContractSetV1, CorrespondenceKindV1, ExactInputIdentityV1,
    ExactModelIdentityV1, ExactToolIdentityV1, PropertyEvidenceV1, PropertyKindV1, TcbEntryKindV1,
    UnsupportedReasonV1,
};

pub const MAX_PROPERTIES_V1: usize = 32;
pub const MAX_OBLIGATIONS_V1: usize = 64;
pub const MAX_TCB_ENTRIES_V1: usize = 32;
pub const MAX_CORRESPONDENCES_V1: usize = 32;
pub const MAX_TCB_REFERENCES_PER_EVIDENCE_V1: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionV1 {
    Properties,
    Obligations,
    TrustedComputingBase,
    Correspondences,
    EvidenceTcbReferences,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityFieldV1 {
    Record,
    Statement,
    Evidence,
    Input,
    Model,
    Tool,
    Artifact,
    Namespace,
    TcbReference,
    Correspondence,
    Rationale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationErrorV1 {
    LimitExceeded {
        section: SectionV1,
        actual: usize,
        maximum: usize,
    },
    EmptySection(SectionV1),
    NonCanonicalOrder {
        section: SectionV1,
        index: usize,
    },
    InvalidIdentity {
        section: SectionV1,
        index: usize,
        field: IdentityFieldV1,
    },
    InvalidExtensionCode {
        section: SectionV1,
        index: usize,
    },
    EvidenceStatusMismatch {
        property_index: usize,
    },
    EvidenceBindingMismatch {
        property_index: usize,
    },
    EvidenceIdentityReused {
        property_index: usize,
    },
    InvalidTcbEntryKind {
        tcb_index: usize,
    },
    UnknownTcbReference {
        property_index: usize,
        reference_index: usize,
    },
    MissingToolTcb {
        property_index: usize,
    },
    UnknownCorrespondence {
        property_index: usize,
    },
    CorrespondenceBindingMismatch {
        property_index: usize,
    },
    VacuousCorrespondence {
        correspondence_index: usize,
    },
    UnreferencedTcb {
        tcb_index: usize,
    },
    UnreferencedCorrespondence {
        correspondence_index: usize,
    },
    UnknownObligationProperty {
        obligation_index: usize,
    },
    ObligationStatementMismatch {
        obligation_index: usize,
    },
    SatisfactionBindingMismatch {
        obligation_index: usize,
    },
    SatisfactionStatusMismatch {
        obligation_index: usize,
    },
    PropertyWithoutExactObligation {
        property_index: usize,
    },
    OpenObligation {
        obligation_index: usize,
    },
}

impl ContractSetV1 {
    /// Checks boundedness, exact bindings, and canonical deterministic order.
    ///
    /// This does not authenticate identities or run any proof/checking tool.
    pub fn validate(&self) -> Result<(), ValidationErrorV1> {
        self.validate_limits_and_order()?;
        self.validate_tcb()?;
        self.validate_correspondences()?;
        self.validate_properties()?;
        self.validate_no_unreferenced_records()?;
        self.validate_obligations()?;
        Ok(())
    }

    /// Applies validate and additionally rejects every open obligation.
    pub fn validate_closed(&self) -> Result<(), ValidationErrorV1> {
        self.validate()?;
        if let Some(index) = self
            .obligations
            .iter()
            .position(|obligation| obligation.satisfaction.is_none())
        {
            return Err(ValidationErrorV1::OpenObligation {
                obligation_index: index,
            });
        }
        Ok(())
    }

    fn validate_limits_and_order(&self) -> Result<(), ValidationErrorV1> {
        limit(
            SectionV1::Properties,
            self.properties.len(),
            MAX_PROPERTIES_V1,
        )?;
        limit(
            SectionV1::Obligations,
            self.obligations.len(),
            MAX_OBLIGATIONS_V1,
        )?;
        limit(
            SectionV1::TrustedComputingBase,
            self.trusted_computing_base.len(),
            MAX_TCB_ENTRIES_V1,
        )?;
        limit(
            SectionV1::Correspondences,
            self.correspondences.len(),
            MAX_CORRESPONDENCES_V1,
        )?;
        if self.properties.is_empty() {
            return Err(ValidationErrorV1::EmptySection(SectionV1::Properties));
        }
        if self.obligations.is_empty() {
            return Err(ValidationErrorV1::EmptySection(SectionV1::Obligations));
        }

        if let Some(index) = self
            .properties
            .windows(2)
            .position(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(ValidationErrorV1::NonCanonicalOrder {
                section: SectionV1::Properties,
                index: index + 1,
            });
        }
        if let Some(index) = self
            .obligations
            .windows(2)
            .position(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(ValidationErrorV1::NonCanonicalOrder {
                section: SectionV1::Obligations,
                index: index + 1,
            });
        }
        if let Some(index) = self
            .trusted_computing_base
            .windows(2)
            .position(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(ValidationErrorV1::NonCanonicalOrder {
                section: SectionV1::TrustedComputingBase,
                index: index + 1,
            });
        }
        if let Some(index) = self
            .correspondences
            .windows(2)
            .position(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(ValidationErrorV1::NonCanonicalOrder {
                section: SectionV1::Correspondences,
                index: index + 1,
            });
        }
        Ok(())
    }

    fn validate_tcb(&self) -> Result<(), ValidationErrorV1> {
        for (index, entry) in self.trusted_computing_base.iter().enumerate() {
            identity(
                entry.identity.is_valid(),
                SectionV1::TrustedComputingBase,
                index,
                IdentityFieldV1::Record,
            )?;
            artifact(
                entry.component,
                SectionV1::TrustedComputingBase,
                index,
                IdentityFieldV1::Artifact,
            )?;
            artifact(
                entry.rationale,
                SectionV1::TrustedComputingBase,
                index,
                IdentityFieldV1::Rationale,
            )?;
            extension(entry.kind, SectionV1::TrustedComputingBase, index)?;
            match (entry.kind, entry.exact_tool) {
                (TcbEntryKindV1::Tool, Some(tool))
                    if tool.is_valid() && entry.component.bytes == tool.executable => {}
                (TcbEntryKindV1::Tool, _) | (_, Some(_)) => {
                    return Err(ValidationErrorV1::InvalidTcbEntryKind { tcb_index: index });
                }
                (_, None) => {}
            }
        }
        Ok(())
    }

    fn validate_correspondences(&self) -> Result<(), ValidationErrorV1> {
        for (index, reference) in self.correspondences.iter().enumerate() {
            identity(
                reference.identity.is_valid(),
                SectionV1::Correspondences,
                index,
                IdentityFieldV1::Record,
            )?;
            identity(
                reference.property.is_valid(),
                SectionV1::Correspondences,
                index,
                IdentityFieldV1::Record,
            )?;
            identity(
                reference.statement.is_valid(),
                SectionV1::Correspondences,
                index,
                IdentityFieldV1::Statement,
            )?;
            exact_input(reference.from, SectionV1::Correspondences, index)?;
            exact_input(reference.to, SectionV1::Correspondences, index)?;
            artifact(
                reference.witness_artifact,
                SectionV1::Correspondences,
                index,
                IdentityFieldV1::Artifact,
            )?;
            correspondence_extension(reference.kind, index)?;
            if reference.from == reference.to {
                return Err(ValidationErrorV1::VacuousCorrespondence {
                    correspondence_index: index,
                });
            }
        }
        Ok(())
    }

    fn validate_properties(&self) -> Result<(), ValidationErrorV1> {
        for (index, property) in self.properties.iter().enumerate() {
            identity(
                property.identity.is_valid(),
                SectionV1::Properties,
                index,
                IdentityFieldV1::Record,
            )?;
            identity(
                property.statement.is_valid(),
                SectionV1::Properties,
                index,
                IdentityFieldV1::Statement,
            )?;
            property_extension(property.kind, index)?;
            if property.status != property.evidence.status() {
                return Err(ValidationErrorV1::EvidenceStatusMismatch {
                    property_index: index,
                });
            }
            let binding = property.evidence.binding();
            identity(
                binding.identity.is_valid(),
                SectionV1::Properties,
                index,
                IdentityFieldV1::Evidence,
            )?;
            if binding.property != property.identity || binding.statement != property.statement {
                return Err(ValidationErrorV1::EvidenceBindingMismatch {
                    property_index: index,
                });
            }
            if self.properties[..index]
                .iter()
                .any(|prior| prior.evidence.binding().identity == binding.identity)
            {
                return Err(ValidationErrorV1::EvidenceIdentityReused {
                    property_index: index,
                });
            }
            self.validate_property_evidence(index, &property.evidence)?;
        }
        Ok(())
    }

    fn validate_property_evidence(
        &self,
        property_index: usize,
        evidence: &PropertyEvidenceV1,
    ) -> Result<(), ValidationErrorV1> {
        match evidence {
            PropertyEvidenceV1::Proved(record) => {
                exact_input(record.input, SectionV1::Properties, property_index)?;
                exact_model(record.model, SectionV1::Properties, property_index)?;
                exact_tool(record.tool, SectionV1::Properties, property_index)?;
                artifact(
                    record.proof_artifact,
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Artifact,
                )?;
                identity(
                    record.correspondence.is_valid(),
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Correspondence,
                )?;
                self.validate_tcb_references(
                    property_index,
                    record.tool,
                    &record.trusted_computing_base,
                )?;
                let Some(reference) = self
                    .correspondences
                    .iter()
                    .find(|reference| reference.identity == record.correspondence)
                else {
                    return Err(ValidationErrorV1::UnknownCorrespondence { property_index });
                };
                if reference.property != record.binding.property
                    || reference.statement != record.binding.statement
                    || reference.from != record.input
                {
                    return Err(ValidationErrorV1::CorrespondenceBindingMismatch {
                        property_index,
                    });
                }
            }
            PropertyEvidenceV1::Validated(record) => {
                exact_input(record.input, SectionV1::Properties, property_index)?;
                exact_model(record.model, SectionV1::Properties, property_index)?;
                exact_tool(record.tool, SectionV1::Properties, property_index)?;
                artifact(
                    record.validation_artifact,
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Artifact,
                )?;
                self.validate_tcb_references(
                    property_index,
                    record.tool,
                    &record.trusted_computing_base,
                )?;
            }
            PropertyEvidenceV1::Contracted(record) => {
                artifact(
                    record.contract_artifact,
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Artifact,
                )?;
            }
            PropertyEvidenceV1::Checked(record) => {
                exact_input(record.input, SectionV1::Properties, property_index)?;
                exact_tool(record.tool, SectionV1::Properties, property_index)?;
                artifact(
                    record.check_artifact,
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Artifact,
                )?;
                self.validate_tcb_references(
                    property_index,
                    record.tool,
                    &record.trusted_computing_base,
                )?;
            }
            PropertyEvidenceV1::Unsupported(record) => {
                unsupported_extension(record.reason, property_index)?;
                artifact(
                    record.rationale_artifact,
                    SectionV1::Properties,
                    property_index,
                    IdentityFieldV1::Rationale,
                )?;
            }
        }
        Ok(())
    }

    fn validate_tcb_references(
        &self,
        property_index: usize,
        tool: ExactToolIdentityV1,
        references: &[crate::TcbEntryIdentityV1],
    ) -> Result<(), ValidationErrorV1> {
        limit(
            SectionV1::EvidenceTcbReferences,
            references.len(),
            MAX_TCB_REFERENCES_PER_EVIDENCE_V1,
        )?;
        if references.is_empty() {
            return Err(ValidationErrorV1::MissingToolTcb { property_index });
        }
        if let Some(index) = references.windows(2).position(|pair| pair[0] >= pair[1]) {
            return Err(ValidationErrorV1::NonCanonicalOrder {
                section: SectionV1::EvidenceTcbReferences,
                index: index + 1,
            });
        }
        for (reference_index, reference) in references.iter().enumerate() {
            identity(
                reference.is_valid(),
                SectionV1::EvidenceTcbReferences,
                reference_index,
                IdentityFieldV1::TcbReference,
            )?;
            if !self
                .trusted_computing_base
                .iter()
                .any(|entry| entry.identity == *reference)
            {
                return Err(ValidationErrorV1::UnknownTcbReference {
                    property_index,
                    reference_index,
                });
            }
        }
        let has_exact_tool = self.trusted_computing_base.iter().any(|entry| {
            references.contains(&entry.identity)
                && entry.kind == TcbEntryKindV1::Tool
                && entry.exact_tool == Some(tool)
        });
        if !has_exact_tool {
            return Err(ValidationErrorV1::MissingToolTcb { property_index });
        }
        Ok(())
    }

    fn validate_no_unreferenced_records(&self) -> Result<(), ValidationErrorV1> {
        for (index, entry) in self.trusted_computing_base.iter().enumerate() {
            if !self.properties.iter().any(|property| {
                property
                    .evidence
                    .trusted_computing_base()
                    .contains(&entry.identity)
            }) {
                return Err(ValidationErrorV1::UnreferencedTcb { tcb_index: index });
            }
        }
        for (index, correspondence) in self.correspondences.iter().enumerate() {
            if !self.properties.iter().any(|property| {
                matches!(
                    &property.evidence,
                    PropertyEvidenceV1::Proved(record)
                        if record.correspondence == correspondence.identity
                )
            }) {
                return Err(ValidationErrorV1::UnreferencedCorrespondence {
                    correspondence_index: index,
                });
            }
        }
        Ok(())
    }

    fn validate_obligations(&self) -> Result<(), ValidationErrorV1> {
        for (index, obligation) in self.obligations.iter().enumerate() {
            identity(
                obligation.identity.is_valid(),
                SectionV1::Obligations,
                index,
                IdentityFieldV1::Record,
            )?;
            identity(
                obligation.property.is_valid(),
                SectionV1::Obligations,
                index,
                IdentityFieldV1::Record,
            )?;
            identity(
                obligation.statement.is_valid(),
                SectionV1::Obligations,
                index,
                IdentityFieldV1::Statement,
            )?;
            let Some(property) = self
                .properties
                .iter()
                .find(|property| property.identity == obligation.property)
            else {
                return Err(ValidationErrorV1::UnknownObligationProperty {
                    obligation_index: index,
                });
            };
            if property.statement != obligation.statement {
                return Err(ValidationErrorV1::ObligationStatementMismatch {
                    obligation_index: index,
                });
            }
            if let Some(satisfaction) = obligation.satisfaction {
                if satisfaction.property != obligation.property
                    || satisfaction.statement != obligation.statement
                    || satisfaction.evidence != property.evidence.binding().identity
                {
                    return Err(ValidationErrorV1::SatisfactionBindingMismatch {
                        obligation_index: index,
                    });
                }
                if satisfaction.status != obligation.required_status
                    || satisfaction.status != property.status
                {
                    return Err(ValidationErrorV1::SatisfactionStatusMismatch {
                        obligation_index: index,
                    });
                }
            }
        }

        for (property_index, property) in self.properties.iter().enumerate() {
            let has_exact_obligation = self.obligations.iter().any(|obligation| {
                obligation.property == property.identity
                    && obligation.statement == property.statement
                    && obligation.required_status == property.status
                    && obligation.satisfaction.is_some_and(|satisfaction| {
                        satisfaction.evidence == property.evidence.binding().identity
                            && satisfaction.property == property.identity
                            && satisfaction.statement == property.statement
                            && satisfaction.status == property.status
                    })
            });
            if !has_exact_obligation {
                return Err(ValidationErrorV1::PropertyWithoutExactObligation { property_index });
            }
        }
        Ok(())
    }
}

fn limit(section: SectionV1, actual: usize, maximum: usize) -> Result<(), ValidationErrorV1> {
    if actual > maximum {
        return Err(ValidationErrorV1::LimitExceeded {
            section,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn identity(
    valid: bool,
    section: SectionV1,
    index: usize,
    field: IdentityFieldV1,
) -> Result<(), ValidationErrorV1> {
    if !valid {
        return Err(ValidationErrorV1::InvalidIdentity {
            section,
            index,
            field,
        });
    }
    Ok(())
}

fn exact_input(
    value: ExactInputIdentityV1,
    section: SectionV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    identity(value.is_valid(), section, index, IdentityFieldV1::Input)
}

fn exact_model(
    value: ExactModelIdentityV1,
    section: SectionV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    identity(value.is_valid(), section, index, IdentityFieldV1::Model)
}

fn exact_tool(
    value: ExactToolIdentityV1,
    section: SectionV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    identity(value.is_valid(), section, index, IdentityFieldV1::Tool)
}

fn artifact(
    value: ArtifactIdentityV1,
    section: SectionV1,
    index: usize,
    field: IdentityFieldV1,
) -> Result<(), ValidationErrorV1> {
    identity(value.is_valid(), section, index, field)
}

fn property_extension(kind: PropertyKindV1, index: usize) -> Result<(), ValidationErrorV1> {
    if let PropertyKindV1::Extension { namespace, code } = kind
        && (namespace.is_zero() || code == 0)
    {
        return Err(ValidationErrorV1::InvalidExtensionCode {
            section: SectionV1::Properties,
            index,
        });
    }
    Ok(())
}

fn unsupported_extension(
    reason: UnsupportedReasonV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    if let UnsupportedReasonV1::Extension { namespace, code } = reason
        && (namespace.is_zero() || code == 0)
    {
        return Err(ValidationErrorV1::InvalidExtensionCode {
            section: SectionV1::Properties,
            index,
        });
    }
    Ok(())
}

fn extension(
    kind: TcbEntryKindV1,
    section: SectionV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    if let TcbEntryKindV1::Extension { namespace, code } = kind
        && (namespace.is_zero() || code == 0)
    {
        return Err(ValidationErrorV1::InvalidExtensionCode { section, index });
    }
    Ok(())
}

fn correspondence_extension(
    kind: CorrespondenceKindV1,
    index: usize,
) -> Result<(), ValidationErrorV1> {
    if let CorrespondenceKindV1::Extension { namespace, code } = kind
        && (namespace.is_zero() || code == 0)
    {
        return Err(ValidationErrorV1::InvalidExtensionCode {
            section: SectionV1::Correspondences,
            index,
        });
    }
    Ok(())
}
