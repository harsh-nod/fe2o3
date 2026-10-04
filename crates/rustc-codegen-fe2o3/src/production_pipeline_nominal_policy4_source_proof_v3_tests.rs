//! Genuine unsigned source refusal, not simulated signed-source success.
use super::*;
use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::any::Any;

impl Input {
    pub(crate) fn source_test_source_proof_quota_failures_v3(
        &self,
        mut fresh: impl FnMut(&mut Budget<'_>) -> (super::super::Input, usize),
        work_limit: usize,
        storage_limit: usize,
    ) {
        for mode in 0..3 {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(53).unwrap();
            let (abi, abi_storage) = fresh(&mut budget);
            assert_eq!(budget.storage(), 53);
            budget.reserve_storage(abi_storage).unwrap();
            let (mut native, receipt) = abi.into_nominal_native_transport_v3(&mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                native.output().canonical().canonical_bytes(),
                self.output().canonical().canonical_bytes()
            );
            let retained = abi_storage + receipt.retained_storage();
            let mut ballast = 0;
            match mode {
                0 => {
                    let guard = 2 * size_of::<usize>()
                        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
                        + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
                        + size_of::<Result<(Output, NominalPolicy4SourceProofStorageV3), E>>();
                    ballast = storage_limit - guard + 1 - budget.storage();
                    budget.reserve_storage(ballast).unwrap();
                }
                1 => budget.charge_work(work_limit - budget.work() - 3).unwrap(),
                _ => {
                    let first = native.module.llvm_ir().as_bytes()[0];
                    native
                        .module
                        .replace_first_ascii_byte_for_test_v3(if first == b'#' {
                            b'!'
                        } else {
                            b'#'
                        });
                }
            }
            let before = budget.work();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let error = match native.into_nominal_source_proof_v3(&mut budget) {
                Ok(_) => panic!("invalid consuming source transition accepted"),
                Err(error) => error,
            };
            match (mode, error) {
                (0, E::Resource(Resource::Storage(error))) => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (storage_limit + 1, storage_limit)
                    );
                    assert_eq!(budget.work(), before);
                }
                (1, E::Resource(Resource::Work(error))) => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (work_limit + 1, work_limit)
                    );
                    assert_eq!(budget.work(), before);
                }
                (
                    2,
                    E::Catalog(
                        fe2o3_lower_mir_kernel::SourcePipelineCatalogCallbackErrorV1::Callback(
                            dialect_amdgcn::NativeV12TextDescriptorReplayErrorV3::Invalid(
                                "exact native/descriptor text",
                            ),
                        ),
                    ),
                ) => {
                    assert!(budget.work() > before);
                }
                (_, other) => panic!("wrong consuming source failure: {other:?}"),
            }
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            budget.release_storage(retained + ballast).unwrap();
            assert_eq!(budget.storage(), 53);
        }
    }

    pub(crate) fn source_test_unsigned_source_proof_refusal_v3(
        self,
        erased: bool,
        budget: &mut Budget<'_>,
    ) {
        assert_eq!(matches!(&self.input.admitted, Admitted::Erased(_)), erased);
        let ranked = &self.input.ranked_verification;
        assert!(!ranked.roots().is_empty());
        assert!(
            ranked
                .roots()
                .iter()
                .all(|root| root.verification().aggregate_verus_execution().is_none())
        );
        let expected_root = ranked.roots()[0].semantic_root().index();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let before = budget.work();
        assert!(floor >= self.retained_storage_floor_v3());
        let result = self.into_nominal_source_proof_v3(budget);
        assert!(matches!(result,
            Err(E::SourceProof(NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root }))
                if root == expected_root));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > before);
    }
}
