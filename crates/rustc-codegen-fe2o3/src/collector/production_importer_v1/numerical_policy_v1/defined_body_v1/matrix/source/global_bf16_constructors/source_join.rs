//! Private native-provider to full-Session source join. Exact Result and error
//! types are retained; neither successful Result flow nor extent is assumed.
use super::*;
use fe2o3_lower_mir_kernel::{ProductionGlobalBf16SourceRowV1, ProductionSemanticKirErrorV1};

type JoinResult<T> = Result<T, ProductionSemanticKirErrorV1>;

impl ConstructorRosterV1 {
    pub(in crate::collector::production_importer_v1) fn matches_subject(
        &self,
        subject: InertSemanticMirSha256V1,
    ) -> bool {
        self.subject == subject
    }

    pub(in crate::collector::production_importer_v1) fn join(
        &self,
        source: ProductionGlobalBf16SourceRowV1<'_, '_>,
        charge: &mut dyn FnMut(usize) -> JoinResult<()>,
    ) -> JoinResult<&ConstructorRowV1> {
        charge(1)?;
        if source.owner().source_semantic().semantic_sha256() != self.subject {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // One bounded index for both lookup and complete-source reconciliation.
        // Do not rebuild or linearly scan the provider roster for every read.
        let (mut start, mut end) = (0, self.rows.len());
        let wrapper = source.wrapper_function();
        while start < end {
            charge(1)?;
            let middle = start + (end - start) / 2;
            if self.rows[middle].wrapper.index() < wrapper.index() {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        let provider = self
            .rows
            .get(start)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        charge(
            1 + provider.inputs.len() + provider.bound_types.len() + provider.output_types.len(),
        )?;
        if !provider.matches(source) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(provider)
    }
}

impl ConstructorRowV1 {
    fn matches(&self, source: ProductionGlobalBf16SourceRowV1<'_, '_>) -> bool {
        let contract = source.contract();
        let types = contract.types();
        self.root == source.view().root()
            && self.wrapper == source.wrapper_function()
            && self.checked == source.checked_function()
            && self.role == contract.operand().role
            && self.identity.provenance() == contract.provenance()
            && self.identity.matrix_brand() == contract.matrix_brand()
            && self.identity.kernel_brand() == contract.global_brand()
            && self.output_types[0] == types.global
            && self.output_types[1] == types.matrix
            && self.output_types[1] != self.output_types[3]
            && self.inputs[0] == source.receiver().operand().ty()
            && self.inputs[2..] == [types.index; 4]
            && source
                .geometry()
                .iter()
                .all(|input| input.operand().ty() == types.index)
            && source
                .bases()
                .iter()
                .all(|input| input.operand().ty() == types.index)
    }

    pub(in crate::collector::production_importer_v1) fn identity(
        &self,
    ) -> SemanticDefinedMatrixIdentityV1 {
        self.identity
    }

    pub(in crate::collector::production_importer_v1) fn bound_types(
        &self,
    ) -> SemanticPolicyMatrixBindTypesV1 {
        SemanticPolicyMatrixBindTypesV1::new(self.bound_types)
    }

    pub(in crate::collector::production_importer_v1) fn result_types(
        &self,
    ) -> [SemanticTypeIdV1; 3] {
        [
            self.output_types[1],
            self.output_types[2],
            self.output_types[3],
        ]
    }
}
