use super::*;

#[path = "fixture.rs"]
mod fixture;
use fixture::*;

pub(crate) fn record_fixture() -> SemanticReusableLdsConversionV1 {
    fixture().observe().unwrap()
}

#[test]
fn reusable_lds_requires_exact_existing_allocation() {
    let f = fixture();
    let r = f.observe().unwrap();
    assert_eq!(r.source().allocation_local.index(), 2);
    assert_eq!(r.epoch(), f.epoch);
    assert_eq!(r.brand(), f.brand);
    assert_eq!(r.elements(), 256);
    for local in [0, 1, 3, u32::MAX] {
        let mut f = fixture();
        f.source.allocation_local = SemanticLocalIdV1::from_index(local);
        assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}

#[test]
fn reusable_lds_rejects_brand_epoch_source_and_layout_substitution() {
    for change in 0..8 {
        let mut f = fixture();
        match change {
            0 => f.brand = SemanticTypeIdentityV1::from_sha256([240; 32]),
            1 => f.epoch = SemanticTypeIdentityV1::from_sha256([241; 32]),
            2 => f.source.caller_identity = SemanticFunctionIdentityV1::from_sha256([242; 32]),
            3 => f.source.allocation_identity = SemanticFunctionIdentityV1::from_sha256([243; 32]),
            4 => f.source.allocation_abi = SemanticAbiIdentityV1::from_sha256([244; 32]),
            5 => f.source.caller_abi = SemanticAbiIdentityV1::from_sha256([245; 32]),
            6 => f.source.source_binding = [0; 32],
            7 => f.types.element = f.types.storage_marker,
            _ => unreachable!(),
        }
        assert_eq!(
            f.observe(),
            Err(SemanticMirErrorV1::InvalidFunctionAbi),
            "mutation {change}"
        );
    }
}

#[test]
fn reusable_lds_accepts_exact_retained_move_but_not_copy() {
    for retained in [false, true] {
        let mut f = fixture();
        let p = place(2, f.types.input);
        f.caller_with_argument(if retained {
            SemanticOperandV1::Move(p)
        } else {
            SemanticOperandV1::Copy(p)
        });
        assert_eq!(f.observe().is_ok(), retained);
    }
}

#[test]
fn reusable_lds_rejects_bypass_edge_and_wrong_normal_target() {
    let mut f = fixture();
    let mut blocks = f.functions[0].blocks().to_vec();
    blocks.push(block(
        4,
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        )),
    ));
    f.replace_caller(blocks);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    let mut f = fixture();
    f.source.conversion_block = SemanticBlockIdV1::from_index(2);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn reusable_lds_canonical_roles_do_not_assume_return_local_zero() {
    let mut f = fixture();
    let abi = f.functions[1].abi().clone();
    f.functions[1] = function(
        31,
        abi,
        &[
            (f.types.input, SemanticLocalRoleV1::Argument(0)),
            (f.types.output, SemanticLocalRoleV1::Return),
        ],
        vec![block(1, SemanticTerminatorKindV1::Return)],
    );
    let record = f.observe().unwrap();
    assert!(body_matches(&f.functions[1], record.types()));
    assert!(
        f.functions[1]
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)
            )
            .is_ok()
    );
}

#[test]
fn reusable_lds_empty_shape_does_not_validate_unrelated_or_mutated_body() {
    let f = fixture();
    let record = f.observe().unwrap();
    let mut changed = fixture();
    changed.functions[1] = function(
        31,
        f.functions[1].abi().clone(),
        &[
            (f.types.output, SemanticLocalRoleV1::Return),
            (f.types.input, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(1, SemanticTerminatorKindV1::Unreachable)],
    );
    assert_eq!(
        validate_attachment(&changed.functions[1], record),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(
        validate_attachment(&f.functions[0], record),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}
