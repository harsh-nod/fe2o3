//! Logical ownership conversion. Its sole operand is the allocation; the
//! canonical payload never substitutes for the SSA edge or source replay.
use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReusableLdsConversionV1 {
    pub input: ExecutionTypeIdentityV1,
    pub output: ExecutionTypeIdentityV1,
    pub element: ExecutionTypeIdentityV1,
    pub layout: ExecutionElementLayoutV1,
    pub elements: u64,
    pub defined_function: [u8; 32],
    pub defined_abi: [u8; 32],
    pub defined_body: [u8; 32],
    pub source_binding: [u8; 32],
}

impl ReusableLdsConversionV1 {
    pub fn is_complete(self) -> bool {
        let types = [self.input, self.output, self.element];
        types
            .iter()
            .enumerate()
            .all(|(i, ty)| ty.is_complete() && !types[..i].contains(ty))
            && self
                .layout
                .checked_footprint(self.elements)
                .is_some_and(|n| n != 0)
            && [
                self.defined_function,
                self.defined_abi,
                self.defined_body,
                self.source_binding,
            ]
            .into_iter()
            .all(|id| id != [0; 32])
    }
    pub fn type_references(self) -> Vec<ExecutionTypeIdentityV1> {
        vec![self.input, self.output, self.element]
    }
    pub fn signature_matches(self, signature: ExecutionCapabilitySignatureV1) -> bool {
        signature.arguments().eq([self.input]) && signature.output() == self.output
    }
    pub fn input_role(self) -> ExecutionCapabilityRoleV1 {
        ExecutionCapabilityRoleV1::Lds {
            element: self.element,
            layout: self.layout,
            elements: self.elements,
            state: ExecutionLdsStateV1::Uninitialized,
        }
    }
    pub fn output_role(self) -> ExecutionCapabilityRoleV1 {
        ExecutionCapabilityRoleV1::ReusableLds {
            element: self.element,
            layout: self.layout,
            elements: self.elements,
        }
    }
    pub fn output_type(
        self,
        input: &ExecutionCapabilityTypeV1,
    ) -> Option<ExecutionCapabilityTypeV1> {
        (self.is_complete()
            && input.is_complete()
            && input.source_type == self.input
            && input.role == self.input_role()
            && input.workgroup_brand.is_some()
            && input.epoch.is_some())
        .then_some(())?;
        Some(ExecutionCapabilityTypeV1 {
            source_type: self.output,
            provenance: input.provenance.clone(),
            workgroup_brand: input.workgroup_brand,
            epoch: input.epoch,
            role: self.output_role(),
        })
    }
    pub(super) fn encode(self, writer: &mut ContractWriter) {
        writer.u8(29);
        for ty in self.type_references() {
            writer.identity(ty);
        }
        put_layout(writer, self.layout);
        writer.u64(self.elements);
        for id in [
            self.defined_function,
            self.defined_abi,
            self.defined_body,
            self.source_binding,
        ] {
            writer.digest(id);
        }
    }
    pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<Self> {
        let value = Self {
            input: reader.identity()?,
            output: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
            defined_function: reader.digest()?,
            defined_abi: reader.digest()?,
            defined_body: reader.digest()?,
            source_binding: reader.digest()?,
        };
        value.is_complete().then_some(value)
    }
}

pub(super) const fn obligations() -> u32 {
    ExecutionSafetyObligationsV1::TARGET_SUPPORT
        | ExecutionSafetyObligationsV1::DYNAMIC_WORKGROUP_IDENTITY
        | ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
        | ExecutionSafetyObligationsV1::ALIASING_VALIDITY
        | ExecutionSafetyObligationsV1::DISJOINT_LDS_ALLOCATION
}

pub(super) fn encode_role(
    writer: &mut ContractWriter,
    element: ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
) {
    writer.u8(16);
    writer.identity(element);
    put_layout(writer, layout);
    writer.u64(elements);
}
pub(super) fn decode_role(reader: &mut ContractReader<'_>) -> Option<ExecutionCapabilityRoleV1> {
    Some(ExecutionCapabilityRoleV1::ReusableLds {
        element: reader.identity()?,
        layout: get_layout(reader)?,
        elements: reader.u64()?,
    })
}

#[cfg(test)]
mod tests;
