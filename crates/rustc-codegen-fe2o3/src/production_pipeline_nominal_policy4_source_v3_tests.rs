use super::*;
use fe2o3_kernel_ir::{
    AddressSpace as Space, Function, FunctionRole as Role, Module, Operation, OperationKind as Op,
};

fn operations(module: &Module) -> impl Iterator<Item = (&Function, &Operation)> {
    module.functions.iter().flat_map(|function| {
        function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .map(move |operation| (function, operation))
    })
}

impl PreparedNominalPolicy4AbiV3 {
    pub(crate) fn source_test_formal_views_v3(
        &self,
    ) -> (
        Option<&fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1>,
        &[fe2o3_kernel_ir::FormalMemoryObligations],
    ) {
        match &self.admitted {
            Output::Direct(owner) => (Some(owner.source_semantic_kir()), owner.kernels()),
            Output::Erased(owner) => (None, owner.kernels()),
        }
    }

    pub(crate) fn source_test_erasure_v3(&self) -> [[u8; 32]; 2] {
        let Output::Erased(owner) = &self.admitted else {
            panic!("actual Erased P4 required")
        };
        assert!(matches!(
            owner.original_source().helper_source_policy_v1(),
            Policy::UnitLocal
        ));
        let (original, erased) = (owner.original_source().executable(), owner.erased());
        assert_ne!(
            original.canonical().identity(),
            erased.canonical().identity()
        );
        assert_eq!(
            (
                owner.erased_source().deleted_function_count(),
                owner.erased_source().deleted_call_count()
            ),
            (1, 1)
        );
        let mut helpers = original
            .module()
            .functions
            .iter()
            .filter(|function| function.role == Role::InternalHelper);
        let helper = helpers.next().expect("retained private helper");
        assert!(helpers.next().is_none());
        assert!(helper.signature.parameters.is_empty() && helper.signature.results.is_empty());
        assert!(operations(original.module()).any(|(function, operation)| {
            function.id == helper.id
                && matches!(&operation.kind,
                Op::Load { access, .. } | Op::Store { access, .. }
                    | Op::GuardedLoad { access, .. } | Op::GuardedStore { access, .. }
                    if access.address_space == Space::Private)
        }));
        let [root] = original.module().kernels.as_slice() else {
            panic!("one actual N root")
        };
        let mut calls = 0;
        for (function, operation) in operations(original.module()) {
            if let Op::Call { callee, arguments } = &operation.kind
                && callee == &helper.id
            {
                assert_eq!(function.id, root.entry);
                assert!(arguments.is_empty() && operation.results.is_empty());
                calls += 1;
            }
        }
        assert_eq!(calls, 1);
        for module in [
            erased.module(),
            owner.bound().module(),
            owner.output().module(),
        ] {
            assert!(module.function(&helper.id).is_none());
            assert!(
                module
                    .functions
                    .iter()
                    .all(|function| function.role != Role::InternalHelper)
            );
            assert!(
                !operations(module).any(|(_, operation)| matches!(&operation.kind,
                Op::Call { callee, .. } if callee == &helper.id))
            );
        }
        [
            *original.canonical().identity().digest(),
            *erased.canonical().identity().digest(),
        ]
    }
}
