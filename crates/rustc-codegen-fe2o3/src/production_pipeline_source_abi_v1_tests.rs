//! One rustc capture and one genuine checked owner; repeated queries do not
//! recapture MIR or construct an optimizer owner from a diagnostic clone.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::nominal_policy4_v3::OwnerRef;
use crate::compiler_descriptor::nominal_v3::{NominalDescriptorErrorV3 as E, scoped};
use crate::compiler_descriptor::source_abi_v1::tests::{Qualification, qualify};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

impl RankedVerifiedProductionCompilation {
    pub(crate) fn entry_packing_scalar_qualification_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        crate::compiler_descriptor::source_abi_v1::entry_packing::scalar_tests::Qualification,
        E,
    > {
        use crate::compiler_descriptor::source_abi_v1::entry_packing::scalar_tests;
        scoped(budget, |budget| {
            use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1 as Policy;
            let observed = match self.ranked.materialized().helper_source_policy_v1() {
                Policy::RawEmpty => {
                    let stage = self
                        .prepare_admitted_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    scalar_tests::qualify(
                        OwnerRef::Direct(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::UnitLocal => {
                    let stage = self
                        .prepare_admitted_erased_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    scalar_tests::qualify(
                        OwnerRef::Erased(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::Bf16Nominal => {
                    return Err(E::Mismatch("scalar packing requires ordinary checked P4"));
                }
            };
            observed.map_err(|error| {
                E::Descriptor(
                    crate::compiler_descriptor::CompilerDescriptorError::from_source_abi(error),
                )
            })
        })
    }

    pub(crate) fn entry_packing_multi_qualification_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        crate::compiler_descriptor::source_abi_v1::entry_packing::multi_tests::Qualification,
        E,
    > {
        use crate::compiler_descriptor::source_abi_v1::entry_packing::multi_tests;
        scoped(budget, |budget| {
            use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1 as Policy;
            let observed = match self.ranked.materialized().helper_source_policy_v1() {
                Policy::RawEmpty => {
                    let stage = self
                        .prepare_admitted_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    multi_tests::qualify(
                        OwnerRef::Direct(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::UnitLocal => {
                    let stage = self
                        .prepare_admitted_erased_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    multi_tests::qualify(
                        OwnerRef::Erased(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::Bf16Nominal => {
                    return Err(E::Mismatch(
                        "packing multi-root requires ordinary checked P4",
                    ));
                }
            };
            observed.map_err(|error| {
                E::Descriptor(
                    crate::compiler_descriptor::CompilerDescriptorError::from_source_abi(error),
                )
            })
        })
    }

    pub(crate) fn source_abi_qualification_v1(
        self,
        expected: &str,
        budget: &mut Budget<'_>,
    ) -> Result<Qualification, E> {
        scoped(budget, |budget| {
            use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1 as Policy;
            let observed = match self.ranked.materialized().helper_source_policy_v1() {
                Policy::RawEmpty => {
                    let stage = self
                        .prepare_admitted_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    assert_eq!(
                        stage.bindings.typed_descriptor_roots[0].logical_name(),
                        expected
                    );
                    qualify(
                        OwnerRef::Direct(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::UnitLocal => {
                    let stage = self
                        .prepare_admitted_erased_policy4_v1(budget)
                        .map_err(E::Pipeline)?;
                    assert_eq!(
                        stage.bindings.typed_descriptor_roots[0].logical_name(),
                        expected
                    );
                    qualify(
                        OwnerRef::Erased(&stage.admitted),
                        &stage.bindings.typed_descriptor_roots,
                        stage.bindings.rustc_target.profile(),
                        budget,
                    )
                }
                Policy::Bf16Nominal => {
                    return Err(E::Mismatch(
                        "source ABI qualification requires ordinary checked P4",
                    ));
                }
            };
            observed.map_err(|error| {
                E::Descriptor(
                    crate::compiler_descriptor::CompilerDescriptorError::from_source_abi(error),
                )
            })
        })
    }
}
