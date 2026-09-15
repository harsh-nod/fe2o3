// The authenticated Issue method is defined only on Gfx950Subgroup, the
// Wave16 view of a Wave64 subgroup. The tile's Wave64 brand is not a claim
// that its borrowed source partition also has width 64.
fn transpose_partition_source_types_match(
    types: &[SemanticTypeDeclV1],
    issue: fe2o3_mir_model::semantic_mir_v1::SemanticGfx950TransposeContractV1,
    derive: fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1,
    path: &[SemanticProjectionV1],
    target: SemanticTypeIdV1,
) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticGfx950LdsTransposeFormatV1 as Format,
        SemanticGfx950TransposeOperationV1 as Transpose,
        SemanticSubgroupPartitionOperationV1 as Partition,
    };
    let Transpose::Issue {
        partition_reference,
        partition: expected,
        ..
    } = issue.operation()
    else {
        return false;
    };
    let Partition::Derive {
        subgroup_reference,
        subgroup,
        partition,
        width,
        partition_width,
        ..
    } = derive
    else {
        return false;
    };
    // These are properties of the two closed source Issue formats, not values
    // inferred from a launch or accepted from arbitrary partition geometry.
    let expected_geometry = match issue.format() {
        Format::Fp4E2M1 | Format::Fp8E4M3 => (64, 16),
    };
    path.is_empty()
        && target == expected
        && partition == expected
        && (width, partition_width) == expected_geometry
        && shared_pointee(types, partition_reference) == Some(expected)
        && shared_pointee(types, subgroup_reference) == Some(subgroup)
}

#[cfg(test)]
mod transpose_partition_geometry_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;
    include!("partition_geometry_tests.rs");
}
