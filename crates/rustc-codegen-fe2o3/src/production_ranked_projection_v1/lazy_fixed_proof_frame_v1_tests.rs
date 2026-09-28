//! Independent typed-row checks; no source/factory/admission promotion.
use super::*;
// Source-typed locals for the two reviewed owner vertices. References are
// measured as references; copied source/ledger inputs remain separate fields.
// These are layout expressions only: no tuple is constructed.
type ExpectedVisit = (
    &'static mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
    usize,
    &'static SemanticFunctionDeclV1,
    Option<&'static mir::SemanticBasicBlockV1>,
    &'static mir::SemanticBasicBlockV1,
    &'static mir::SemanticTerminatorV1,
    &'static SemanticTerminatorKindV1,
    &'static SemanticOperandV1,
    &'static SemanticOperandV1,
    &'static SemanticOperandV1,
    &'static bool,
    &'static SemanticUnwindActionV1,
    FixedGuardInputsV1<'static, 'static>,
    &'static mut PreparedFixedGuardSessionV1<'static>,
    Result<FixedGuardDataV1>,
    SemanticLocalIdV1,
    u64,
    LazyFixedEventV1,
    Option<usize>,
);
type ExpectedInitialize = (
    FixedGuardInputsV1<'static, 'static>,
    FixedGuardLedgerV1,
    &'static mut Building<'static>,
    &'static mut AssertionResourcesV1<'static>,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    std::iter::Enumerate<std::slice::Iter<'static, mir::SemanticBasicBlockV1>>,
    Option<(usize, &'static mir::SemanticBasicBlockV1)>,
    &'static mir::SemanticBasicBlockV1,
    &'static mir::SemanticTerminatorV1,
    &'static SemanticTerminatorKindV1,
    &'static SemanticOperandV1,
    Option<SemanticLocalIdV1>,
    SemanticLocalIdV1,
    Option<&'static mut Vec<usize>>,
    &'static mut Vec<usize>,
);
type ExpectedConstantOperand = (
    &'static SemanticOperandV1,
    &'static [Option<u64>],
    ConstantDefinitionV1,
    SemanticLocalIdV1,
    usize,
    Option<&'static Option<u64>>,
    Option<Option<u64>>,
    Option<u64>,
    u64,
);
type ExpectedConstantDefinition = (
    &'static SemanticOperandV1,
    &'static mir::SemanticConstantV1,
    &'static SemanticConstantValueV1,
    &'static mir::SemanticScalarValueV1,
    &'static SemanticPlaceV1,
    u128,
    std::result::Result<u64, std::num::TryFromIntError>,
    std::result::Result<ConstantDefinitionV1, std::num::TryFromIntError>,
    u64,
    SemanticLocalIdV1,
);

fn expected<R>(locals: usize) -> usize {
    locals
        .checked_add(size_of::<R>())
        .unwrap()
        .checked_add(size_of::<R>())
        .unwrap()
        .checked_add(size_of::<Result<R>>())
        .unwrap()
        .checked_add(size_of::<Result<R>>())
        .unwrap()
}
fn compact_source(source: &str) -> String {
    source
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect()
}
// The sole optional punctuation is rustfmt's trailing comma after this
// one complete size_of expression. Never strip or rewrite other punctuation.
fn typed_row_count(source: &str, row: &str) -> usize {
    let prefix = row.strip_suffix(")?").expect("closed frame row");
    let with_comma = format!("{prefix},)?");
    source.matches(row).count() + source.matches(with_comma.as_str()).count()
}
fn owner_source() -> String {
    let source = compact_source(include_str!("lazy_fixed_proof_owner_v1.rs"));
    source
        .split("fnowner_frame()->Result<usize>{")
        .nth(1)
        .unwrap()
        .split("fnfacet_frame<T>()->Result<usize>{")
        .next()
        .unwrap()
        .to_owned()
}
#[test]
fn typed_equation_has_no_opaque_fixed_allowance() {
    assert_eq!(frame::<u128>(17).unwrap(), expected::<u128>(17));
    assert_eq!(frame::<()>(0).unwrap(), expected::<()>(0));
    assert_eq!(
        frame::<FixedGuardDataV1>(size_of::<ExpectedVisit>()).unwrap(),
        expected::<FixedGuardDataV1>(size_of::<ExpectedVisit>())
    );
    let source = compact_source(include_str!("lazy_fixed_proof_owner_v1.rs"));
    let body = source
        .split("fnframe<R>(locals:usize)->Result<usize>{")
        .nth(1)
        .unwrap()
        .split("fnsum(")
        .next()
        .unwrap();
    assert!(!body.contains("4096"));
}
#[test]
fn typed_equation_refuses_overflow() {
    assert!(frame::<()>(usize::MAX).is_err());
    assert!(frame::<u128>(usize::MAX - size_of::<u128>()).is_err());
}
#[test]
fn visit_row_names_actual_copied_and_borrowed_inputs() {
    assert_eq!(
        size_of::<VisitInnerFrameLocalsV1>(),
        size_of::<ExpectedVisit>()
    );
    assert_eq!(
        frame::<LazyFixedEventV1>(size_of::<VisitInnerFrameLocalsV1>()).unwrap(),
        expected::<LazyFixedEventV1>(size_of::<ExpectedVisit>())
    );
    assert_eq!(
        typed_row_count(
            &owner_source(),
            "frame::<LazyFixedEventV1>(size_of::<VisitInnerFrameLocalsV1>())?"
        ),
        1
    );
}
#[test]
fn initialization_row_names_actual_ledger_and_resource_borrow() {
    assert_eq!(
        size_of::<InitializeBuildingFrameLocalsV1>(),
        size_of::<ExpectedInitialize>()
    );
    assert_eq!(
        frame::<()>(size_of::<InitializeBuildingFrameLocalsV1>()).unwrap(),
        expected::<()>(size_of::<ExpectedInitialize>())
    );
    assert_eq!(
        typed_row_count(
            &owner_source(),
            "frame::<()>(size_of::<InitializeBuildingFrameLocalsV1>())?"
        ),
        1
    );
}
#[test]
fn constant_rows_keep_distinct_callers_and_result_transfers() {
    assert_eq!(
        size_of::<ConstantOperandFrameLocalsV1>(),
        size_of::<ExpectedConstantOperand>()
    );
    assert_eq!(
        size_of::<ConstantDefinitionFrameLocalsV1>(),
        size_of::<ExpectedConstantDefinition>()
    );
    assert_eq!(
        frame::<Option<u64>>(size_of::<ConstantOperandFrameLocalsV1>()).unwrap(),
        expected::<Option<u64>>(size_of::<ExpectedConstantOperand>())
    );
    assert_eq!(
        frame::<ConstantDefinitionV1>(size_of::<ConstantDefinitionFrameLocalsV1>()).unwrap(),
        expected::<ConstantDefinitionV1>(size_of::<ExpectedConstantDefinition>())
    );
    for row in [
        "frame::<Option<u64>>(size_of::<ConstantOperandFrameLocalsV1>())?",
        "frame::<ConstantDefinitionV1>(size_of::<ConstantDefinitionFrameLocalsV1>())?",
    ] {
        assert_eq!(typed_row_count(&owner_source(), row), 1);
    }
}
#[test]
fn every_nested_accessor_has_its_own_typed_row() {
    let owner = owner_source();
    let rows: [(&str, usize); 24] = [
        (
            "frame::<&'static NominalRootSourceTablesV1<'static>>(size_of::<&NominalRootCfgSourceV1<'static>,>())?",
            expected::<&'static NominalRootSourceTablesV1<'static>>(size_of::<
                &NominalRootCfgSourceV1<'static>,
            >()),
        ),
        (
            "frame::<&'static SemanticFunctionDeclV1>(size_of::<&NominalRootCfgSourceV1<'static>>())?",
            expected::<&'static SemanticFunctionDeclV1>(
                size_of::<&NominalRootCfgSourceV1<'static>>(),
            ),
        ),
        (
            "frame::<&'static ProjectedLoopCfgV1>(size_of::<&NominalRootCfgSourceV1<'static>>())?",
            expected::<&'static ProjectedLoopCfgV1>(size_of::<&NominalRootCfgSourceV1<'static>>()),
        ),
        (
            "frame::<&'static [SemanticTypeDeclV1]>(size_of::<&NominalRootCfgSourceV1<'static>>())?",
            expected::<&'static [SemanticTypeDeclV1]>(size_of::<&NominalRootCfgSourceV1<'static>>()),
        ),
        (
            "frame::<&'static RichNominalSourceTablesV1<'static>>(size_of::<&NominalRootSourceTablesV1<'static>,>())?",
            expected::<&'static RichNominalSourceTablesV1<'static>>(size_of::<
                &NominalRootSourceTablesV1<'static>,
            >()),
        ),
        (
            "frame::<&'static SemanticFunctionDeclV1>(size_of::<&RichNominalSourceTablesV1<'static>>())?",
            expected::<&'static SemanticFunctionDeclV1>(size_of::<
                &RichNominalSourceTablesV1<'static>,
            >()),
        ),
        (
            "frame::<bool>(size_of::<(&RichNominalSourceTablesV1<'static>, FixedGuardLedgerV1,)>())?",
            expected::<bool>(size_of::<(
                &RichNominalSourceTablesV1<'static>,
                FixedGuardLedgerV1,
            )>()),
        ),
        (
            "frame::<&'static [Option<u64>]>(size_of::<&RichNominalSourceTablesV1<'static>>())?",
            expected::<&'static [Option<u64>]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        ),
        (
            "frame::<&'static [u8]>(size_of::<&RichNominalSourceTablesV1<'static>>())?",
            expected::<&'static [u8]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        ),
        (
            "frame::<&'static [Vec<usize>]>(size_of::<&RichNominalSourceTablesV1<'static>>())?",
            expected::<&'static [Vec<usize>]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        ),
        (
            "frame::<&'static [Option<ScalarAssignmentSiteV1>]>(size_of::<&RichNominalSourceTablesV1<'static>,>())?",
            expected::<&'static [Option<ScalarAssignmentSiteV1>]>(size_of::<
                &RichNominalSourceTablesV1<'static>,
            >()),
        ),
        (
            "frame::<&'static [bool]>(size_of::<&RichNominalSourceTablesV1<'static>>())?",
            expected::<&'static [bool]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        ),
        (
            "frame::<&'static [mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>())?",
            expected::<&'static [mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>()),
        ),
        (
            "frame::<&'static [mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>())?",
            expected::<&'static [mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>()),
        ),
        (
            "frame::<SemanticBlockIdV1>(size_of::<&SemanticFunctionDeclV1>())?",
            expected::<SemanticBlockIdV1>(size_of::<&SemanticFunctionDeclV1>()),
        ),
        (
            "frame::<&'static mir::SemanticTerminatorV1>(size_of::<&mir::SemanticBasicBlockV1>())?",
            expected::<&'static mir::SemanticTerminatorV1>(size_of::<&mir::SemanticBasicBlockV1>()),
        ),
        (
            "frame::<&'static SemanticTerminatorKindV1>(size_of::<&mir::SemanticTerminatorV1>())?",
            expected::<&'static SemanticTerminatorKindV1>(size_of::<&mir::SemanticTerminatorV1>()),
        ),
        (
            "frame::<&'static SemanticConstantValueV1>(size_of::<&mir::SemanticConstantV1>())?",
            expected::<&'static SemanticConstantValueV1>(size_of::<&mir::SemanticConstantV1>()),
        ),
        (
            "frame::<u128>(size_of::<mir::SemanticScalarValueV1>())?",
            expected::<u128>(size_of::<mir::SemanticScalarValueV1>()),
        ),
        (
            "frame::<&'static [mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>())?",
            expected::<&'static [mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>()),
        ),
        (
            "frame::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>())?",
            expected::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>()),
        ),
        (
            "frame::<SemanticProjectionKindV1>(size_of::<mir::SemanticProjectionV1>())?",
            expected::<SemanticProjectionKindV1>(size_of::<mir::SemanticProjectionV1>()),
        ),
        (
            "frame::<u32>(size_of::<SemanticLocalIdV1>())?",
            expected::<u32>(size_of::<SemanticLocalIdV1>()),
        ),
        (
            "frame::<u32>(size_of::<SemanticBlockIdV1>())?",
            expected::<u32>(size_of::<SemanticBlockIdV1>()),
        ),
    ];
    let mut total = 0usize;
    for (source, bytes) in rows {
        let compact = compact_source(source);
        assert_eq!(typed_row_count(&owner, compact.as_str()), 1, "{source}");
        total = total.checked_add(bytes).unwrap();
    }
    assert!(total > 0);
    assert_eq!(
        owner
            .matches("distinctreceiverandcaller/calleereturntransfer")
            .count(),
        24
    );
}
#[test]
fn complete_owner_equation_includes_each_selected_row_once() {
    type O = LazyFixedProofOwnerV1<'static, 'static, 'static>;
    let rows = [
        expected::<O>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            &mut Prep<'static, 'static>,
            FixedGuardLedgerV1,
            usize,
        )>()),
        expected::<O>(size_of::<(
            &NominalRootCfgSourceV1<'static>,
            &mut Prep<'static, 'static>,
        )>()),
        expected::<()>(size_of::<(&mut O, bool)>()),
        expected::<()>(size_of::<(
            &mut O,
            usize,
            &mut Prep<'static, 'static>,
            &mut AssertionResourcesV1<'static>,
        )>()),
        expected::<LazyFixedEventV1>(size_of::<(&mut O, usize, Result<LazyFixedEventV1>)>()),
        expected::<LazyFixedEventV1>(size_of::<ExpectedVisit>()),
        expected::<Option<u64>>(size_of::<ExpectedConstantOperand>()),
        expected::<ConstantDefinitionV1>(size_of::<ExpectedConstantDefinition>()),
        expected::<()>(size_of::<(
            &mut O,
            Option<&mut Prep<'static, 'static>>,
            Result<AssertionResourcesV1<'static>>,
            Phase<'static, 'static, 'static>,
        )>()),
        expected::<()>(size_of::<ExpectedInitialize>()),
        expected::<PreparedFixedGuardSessionV1<'static>>(size_of::<(
            Building<'static>,
            SemanticAssertProofsV1<'static>,
            FixedGuardInputsV1<'static, 'static>,
            FixedGuardLedgerV1,
        )>()),
        expected::<FixedGuardDataV1>(size_of::<(
            &mut PreparedFixedGuardSessionV1<'static>,
            FixedGuardInputsV1<'static, 'static>,
            usize,
        )>()),
        expected::<Option<SemanticLocalIdV1>>(size_of::<(
            &SemanticOperandV1,
            u32,
            &SemanticPlaceV1,
            &[mir::SemanticProjectionV1],
            &mir::SemanticProjectionV1,
            SemanticProjectionKindV1,
            bool,
            SemanticLocalIdV1,
        )>()),
        expected::<()>(size_of::<(
            &mut O,
            Option<&mut Prep<'static, 'static>>,
            Result<AssertionResourcesV1<'static>>,
            Building<'static>,
        )>()),
        expected::<()>(size_of::<(
            &mut Building<'static>,
            Result<AssertionCacheV1<'static>>,
        )>()),
        expected::<usize>(size_of::<(usize, Option<usize>)>()),
        expected::<&'static NominalRootSourceTablesV1<'static>>(size_of::<
            &NominalRootCfgSourceV1<'static>,
        >()),
        expected::<&'static SemanticFunctionDeclV1>(size_of::<&NominalRootCfgSourceV1<'static>>()),
        expected::<&'static ProjectedLoopCfgV1>(size_of::<&NominalRootCfgSourceV1<'static>>()),
        expected::<&'static [SemanticTypeDeclV1]>(size_of::<&NominalRootCfgSourceV1<'static>>()),
        expected::<&'static RichNominalSourceTablesV1<'static>>(size_of::<
            &NominalRootSourceTablesV1<'static>,
        >()),
        expected::<&'static SemanticFunctionDeclV1>(
            size_of::<&RichNominalSourceTablesV1<'static>>(),
        ),
        expected::<bool>(size_of::<(
            &RichNominalSourceTablesV1<'static>,
            FixedGuardLedgerV1,
        )>()),
        expected::<&'static [Option<u64>]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        expected::<&'static [u8]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        expected::<&'static [Vec<usize>]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        expected::<&'static [Option<ScalarAssignmentSiteV1>]>(size_of::<
            &RichNominalSourceTablesV1<'static>,
        >()),
        expected::<&'static [bool]>(size_of::<&RichNominalSourceTablesV1<'static>>()),
        expected::<&'static [mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>()),
        expected::<&'static [mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>()),
        expected::<SemanticBlockIdV1>(size_of::<&SemanticFunctionDeclV1>()),
        expected::<&'static mir::SemanticTerminatorV1>(size_of::<&mir::SemanticBasicBlockV1>()),
        expected::<&'static SemanticTerminatorKindV1>(size_of::<&mir::SemanticTerminatorV1>()),
        expected::<&'static SemanticConstantValueV1>(size_of::<&mir::SemanticConstantV1>()),
        expected::<u128>(size_of::<mir::SemanticScalarValueV1>()),
        expected::<&'static [mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>()),
        expected::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>()),
        expected::<SemanticProjectionKindV1>(size_of::<mir::SemanticProjectionV1>()),
        expected::<u32>(size_of::<SemanticLocalIdV1>()),
        expected::<u32>(size_of::<SemanticBlockIdV1>()),
        expected::<usize>(size_of::<[usize; 44]>()),
        expected::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>()),
        expected::<Error>(size_of::<Resource>()),
        expected::<Error>(size_of::<Resource>()),
    ];
    assert_eq!(rows.len(), 44);
    let want = rows
        .iter()
        .try_fold(0usize, |n, x| n.checked_add(*x))
        .unwrap();
    assert_eq!(owner_frame().unwrap(), want);
    assert_eq!(owner_source().matches("frame::<").count(), 44);
    assert_eq!(owner_source().matches("size_of::<[usize;44]>()").count(), 1);
}

#[test]
fn typed_row_selector_allows_only_the_closed_trailing_comma() {
    let row = "frame::<u8>(size_of::<u8>())?";
    let comma = "frame::<u8>(size_of::<u8>(),)?";
    assert_eq!(typed_row_count(row, row), 1);
    assert_eq!(typed_row_count(comma, row), 1);
    for bad in [
        "frame::<u16>(size_of::<u8>())?",
        "frame::<u8>(size_of::<u16>())?",
        "frame::<u8>(,size_of::<u8>())?",
        "frame::<u8>(size_of::<u8>(),,)?",
        "frame::<u8>(size_of::<u8>();)?",
        "frame::<u8>(size_of::<u8>(),0)?",
    ] {
        assert_eq!(typed_row_count(bad, row), 0, "{bad}");
    }
    assert_eq!(typed_row_count(&format!("{row}{comma}"), row), 2);
}
