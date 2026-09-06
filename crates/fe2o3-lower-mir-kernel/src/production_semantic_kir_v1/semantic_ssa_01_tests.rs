mod semantic_ssa_transport_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAtomicAccessV1, SemanticBorrowKindV1, SemanticMemoryStoreV1,
        SemanticPointerTypeV1,
    };

    include!("semantic_ssa_capability_01_tests.rs");
    include!("semantic_ssa_transport_01_tests.rs");
    include!("semantic_ssa_enum_01_tests.rs");
}
