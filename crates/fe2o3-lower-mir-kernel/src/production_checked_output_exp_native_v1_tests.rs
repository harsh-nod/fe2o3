use super::*;
use dialect_amdgcn::{
    bind_production_llvm22_worker_layout_v1, check_native_v12_text_descriptor_relation_v1 as check,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
};
use fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 as Catalog;

pub(super) fn check_actual(
    owner: &crate::ProductionCheckedOutputOwnerPolicy4V1,
    profile: Profile,
    budget: &mut AssertOriginBudgetV1<'_>,
) {
    let entry = budget.storage();
    const FIXTURE: usize = 1 << 20;
    budget.reserve_storage(FIXTURE).unwrap();
    let sha = *owner
        .source_semantic_kir()
        .semantic()
        .semantic()
        .semantic_sha256()
        .as_bytes();
    let (catalog, receipt) = Catalog::from_rows_with_budget(sha, &[], &[], budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let output = owner.output();
    let native = match profile {
        Profile::Gfx942 => lower_942(output),
        Profile::Gfx950 => lower_950(output),
    }
    .unwrap();
    let prefix = bind_production_llvm22_worker_layout_v1(&native).unwrap();
    assert!(
        prefix.contains("call float @__ocml_exp_f32(float "),
        "{prefix}"
    );
    assert!(
        prefix.contains("declare float @__ocml_exp_f32(float)"),
        "{prefix}"
    );
    for attribute in [
        "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
        "\"unsafe-fp-math\"=\"false\"",
        "\"no-infs-fp-math\"=\"false\"",
        "\"no-nans-fp-math\"=\"false\"",
        "\"no-signed-zeros-fp-math\"=\"false\"",
        "\"fp-contract\"=\"off\"",
    ] {
        assert!(
            prefix.contains(attribute),
            "missing strict attribute: {attribute}"
        );
    }
    for flag in [
        " fast ",
        " reassoc ",
        " arcp ",
        " afn ",
        " nnan ",
        " ninf ",
        " nsz ",
        " contract ",
    ] {
        assert!(!prefix.contains(flag), "unexpected fast-math flag: {flag}");
    }
    let descriptors = super::super::native::table(profile);
    let text = super::super::native::final_text(&prefix, &descriptors);
    assert!(native.len() + prefix.len() + text.len() * 6 < FIXTURE);
    let floor = budget.storage();
    let retained = {
        let relation = check(
            output,
            &catalog,
            output.canonical().canonical_bytes(),
            profile,
            &descriptors,
            &text,
            budget,
        )
        .unwrap();
        let retained = relation.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        assert!(std::ptr::eq(relation.output(), output));
        assert!(!relation.grants_authority());
        retained
    };
    budget.release_storage(retained).unwrap();
    for altered in [
        text.replacen(
            "call float @__ocml_exp_f32",
            "call fast float @__ocml_exp_f32",
            1,
        ),
        text.replace("@__ocml_exp_f32", "@__ocml_exp2_f32"),
        text.replace("@__ocml_exp_f32", "@unapproved_exp_f32"),
        text.replace(
            "\"unsafe-fp-math\"=\"false\"",
            "\"unsafe-fp-math\"=\"true\"",
        ),
    ] {
        assert_ne!(altered, text);
        assert!(
            check(
                output,
                &catalog,
                output.canonical().canonical_bytes(),
                profile,
                &descriptors,
                &altered,
                budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
    drop((catalog, native, prefix, descriptors, text));
    budget.release_storage(receipt.retained_storage()).unwrap();
    budget.release_storage(FIXTURE).unwrap();
    assert_eq!(budget.storage(), entry);
}
