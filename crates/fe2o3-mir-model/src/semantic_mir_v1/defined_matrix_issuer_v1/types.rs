#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKernelMatrixDeriveTypesV1 {
    pub context_reference: SemanticTypeIdV1,
    pub context: SemanticTypeIdV1,
    pub matrix: SemanticTypeIdV1,
    pub unbranded_matrix: SemanticTypeIdV1,
    pub brand_marker: SemanticTypeIdV1,
    pub thread_marker: SemanticTypeIdV1,
}

impl SemanticKernelMatrixDeriveTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 6]) -> Self {
        let [
            context_reference,
            context,
            matrix,
            unbranded_matrix,
            brand_marker,
            thread_marker,
        ] = ids;
        Self {
            context_reference,
            context,
            matrix,
            unbranded_matrix,
            brand_marker,
            thread_marker,
        }
    }
    pub const fn all(self) -> [SemanticTypeIdV1; 6] {
        [
            self.context_reference,
            self.context,
            self.matrix,
            self.unbranded_matrix,
            self.brand_marker,
            self.thread_marker,
        ]
    }
}
