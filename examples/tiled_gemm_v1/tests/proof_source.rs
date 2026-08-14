mod support;

use std::process::Command;

use fe2o3_device::RowMajorXor4;
use fe2o3_tiled_gemm_v1::{
    ARegisterLayoutV1, AccumulatorRegisterLayoutV1, BRegisterLayoutV1, LdsLogicalCoordinateV1,
    RowMajorXor4StagingV1,
};

const PROOF_SOURCE: &[u8] = include_bytes!("../verus/tiled_gemm_host_contract.rs");
const PINNED_PROOF_BYTES: usize = 34_733;
const PINNED_PROOF_SHA256: &str =
    "fcb0bb8d86430fce8dafcd8a049864111952e49b13e0a68997aa424729db336c";
const PINNED_VERUS_VERSION: &str = "0.2026.08.02.b677dd5";
const PINNED_VERUS_SHA256: &str =
    "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";
const OFFICIAL_XOR2_TABLE: [[usize; 4]; 4] =
    [[0, 1, 2, 3], [1, 0, 3, 2], [2, 3, 0, 1], [3, 2, 1, 0]];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParsedFormula {
    LaneModulo16,
    FourTimesLaneQuotientPlusComponent,
    Xor4Column,
    Xor4Index,
    AStagingIndex,
    BTransposedStagingIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Xor2Argument {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Xor2Atom {
    Argument(Xor2Argument),
    Literal(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Xor2Equality {
    left: Xor2Atom,
    right: Xor2Atom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Xor2Expression {
    Atom(Xor2Atom),
    IfElse {
        condition: Xor2Equality,
        when_true: Box<Xor2Expression>,
        when_false: Box<Xor2Expression>,
    },
}

impl Xor2Atom {
    fn evaluate(self, left: usize, right: usize) -> usize {
        match self {
            Self::Argument(Xor2Argument::Left) => left,
            Self::Argument(Xor2Argument::Right) => right,
            Self::Literal(value) => value,
        }
    }
}

impl Xor2Expression {
    fn evaluate(&self, left: usize, right: usize) -> usize {
        match self {
            Self::Atom(atom) => atom.evaluate(left, right),
            Self::IfElse {
                condition,
                when_true,
                when_false,
            } => {
                let branch = if condition.left.evaluate(left, right)
                    == condition.right.evaluate(left, right)
                {
                    when_true
                } else {
                    when_false
                };
                branch.evaluate(left, right)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Xor2Token {
    If,
    Else,
    Left,
    Right,
    Literal(usize),
    EqualEqual,
    LeftBrace,
    RightBrace,
}

fn tokenize_xor2_body(body: &str) -> Vec<Xor2Token> {
    let bytes = body.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte.is_ascii_whitespace() => cursor += 1,
            b'{' => {
                tokens.push(Xor2Token::LeftBrace);
                cursor += 1;
            }
            b'}' => {
                tokens.push(Xor2Token::RightBrace);
                cursor += 1;
            }
            b'=' if bytes.get(cursor + 1) == Some(&b'=') => {
                tokens.push(Xor2Token::EqualEqual);
                cursor += 2;
            }
            byte if byte.is_ascii_digit() => {
                let start = cursor;
                while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                    cursor += 1;
                }
                let value = body[start..cursor].parse().unwrap();
                tokens.push(Xor2Token::Literal(value));
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = cursor;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    cursor += 1;
                }
                tokens.push(match &body[start..cursor] {
                    "if" => Xor2Token::If,
                    "else" => Xor2Token::Else,
                    "left" => Xor2Token::Left,
                    "right" => Xor2Token::Right,
                    token => panic!("unsupported token in Verus xor2_v1 body: {token}"),
                });
            }
            byte => panic!("unsupported byte in Verus xor2_v1 body: {byte:#x}"),
        }
    }
    tokens
}

struct Xor2Parser {
    tokens: Vec<Xor2Token>,
    cursor: usize,
}

impl Xor2Parser {
    fn parse(body: &str) -> Xor2Expression {
        let mut parser = Self {
            tokens: tokenize_xor2_body(body),
            cursor: 0,
        };
        let expression = parser.parse_expression();
        assert_eq!(
            parser.cursor,
            parser.tokens.len(),
            "trailing syntax in Verus xor2_v1 body"
        );
        expression
    }

    fn parse_expression(&mut self) -> Xor2Expression {
        if self.peek() == Some(Xor2Token::If) {
            self.parse_if_else()
        } else {
            Xor2Expression::Atom(self.parse_atom())
        }
    }

    fn parse_if_else(&mut self) -> Xor2Expression {
        self.expect(Xor2Token::If);
        let condition = Xor2Equality {
            left: self.parse_atom(),
            right: {
                self.expect(Xor2Token::EqualEqual);
                self.parse_atom()
            },
        };
        self.expect(Xor2Token::LeftBrace);
        let when_true = self.parse_expression();
        self.expect(Xor2Token::RightBrace);
        self.expect(Xor2Token::Else);
        let when_false = if self.peek() == Some(Xor2Token::If) {
            self.parse_if_else()
        } else {
            self.expect(Xor2Token::LeftBrace);
            let expression = self.parse_expression();
            self.expect(Xor2Token::RightBrace);
            expression
        };
        Xor2Expression::IfElse {
            condition,
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        }
    }

    fn parse_atom(&mut self) -> Xor2Atom {
        match self.next() {
            Some(Xor2Token::Left) => Xor2Atom::Argument(Xor2Argument::Left),
            Some(Xor2Token::Right) => Xor2Atom::Argument(Xor2Argument::Right),
            Some(Xor2Token::Literal(value)) => Xor2Atom::Literal(value),
            token => panic!("expected atom in Verus xor2_v1 body, found {token:?}"),
        }
    }

    fn peek(&self) -> Option<Xor2Token> {
        self.tokens.get(self.cursor).copied()
    }

    fn next(&mut self) -> Option<Xor2Token> {
        let token = self.peek();
        self.cursor += usize::from(token.is_some());
        token
    }

    fn expect(&mut self, expected: Xor2Token) {
        let actual = self.next();
        assert_eq!(actual, Some(expected), "unexpected Verus xor2_v1 syntax");
    }
}

fn normalized_spec_body(source: &str, function: &str) -> String {
    let marker = format!("pub open spec fn {function}");
    let declaration = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing Verus formula {function}"));
    let body_start = source[declaration..]
        .find('{')
        .map(|offset| declaration + offset)
        .unwrap();
    let mut depth = 0_u32;
    let mut body_end = None;
    for (offset, byte) in source.as_bytes()[body_start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = Some(body_start + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    source[body_start + 1..body_end.unwrap()]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_formula(source: &str, function: &str) -> ParsedFormula {
    match normalized_spec_body(source, function).as_str() {
        "lane % 16" => ParsedFormula::LaneModulo16,
        "4 * (lane / 16) + component" => ParsedFormula::FourTimesLaneQuotientPlusComponent,
        "xor2_v1(row % 4, col / 4) * 4 + col % 4" => ParsedFormula::Xor4Column,
        "row * 16 + xor4_lds_col_v1(row, col)" => ParsedFormula::Xor4Index,
        "xor4_lds_index_v1( a_register_row_v1(lane), a_register_depth_v1(lane, component), )" => {
            ParsedFormula::AStagingIndex
        }
        "xor4_lds_index_v1( b_register_col_v1(lane), b_register_depth_v1(lane, component), )" => {
            ParsedFormula::BTransposedStagingIndex
        }
        body => panic!("unrecognized Verus formula for {function}: {body}"),
    }
}

fn evaluate_staging_formula(
    formula: ParsedFormula,
    xor2: &Xor2Expression,
    lane: usize,
    component: usize,
) -> usize {
    let (row, column) = match formula {
        ParsedFormula::AStagingIndex => (lane % 16, 4 * (lane / 16) + component),
        ParsedFormula::BTransposedStagingIndex => (lane % 16, 4 * (lane / 16) + component),
        other => panic!("{other:?} is not a staging formula"),
    };
    evaluate_lds_formula(ParsedFormula::Xor4Index, xor2, row, column)
}

fn evaluate_lane_formula(formula: ParsedFormula, lane: usize, component: usize) -> usize {
    match formula {
        ParsedFormula::LaneModulo16 => lane % 16,
        ParsedFormula::FourTimesLaneQuotientPlusComponent => 4 * (lane / 16) + component,
        other => panic!("{other:?} is not a lane/component formula"),
    }
}

fn evaluate_lds_formula(
    formula: ParsedFormula,
    xor2: &Xor2Expression,
    row: usize,
    column: usize,
) -> usize {
    match formula {
        ParsedFormula::Xor4Column => xor2.evaluate(row % 4, column / 4) * 4 + column % 4,
        ParsedFormula::Xor4Index => {
            let physical_column =
                evaluate_lds_formula(ParsedFormula::Xor4Column, xor2, row, column);
            row * 16 + physical_column
        }
        other => panic!("{other:?} is not an LDS formula"),
    }
}

#[test]
fn local_sha256_matches_the_standard_abc_vector() {
    assert_eq!(
        support::hex(support::sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn exact_tiled_gemm_proof_and_verus_identity_are_pinned() {
    assert_eq!(PROOF_SOURCE.len(), PINNED_PROOF_BYTES);
    assert_eq!(
        support::hex(support::sha256(PROOF_SOURCE)),
        PINNED_PROOF_SHA256
    );
    assert_eq!(
        include_str!("../verus/VERUS_VERSION"),
        format!("{PINNED_VERUS_VERSION}\n")
    );
    assert_eq!(
        include_str!("../verus/VERUS_SHA256"),
        format!("{PINNED_VERUS_SHA256}\n")
    );
}

#[test]
fn pinned_source_contains_the_complete_public_theorem_set_without_shortcuts() {
    let source = std::str::from_utf8(PROOF_SOURCE).unwrap();
    let theorem_markers = [
        "pub proof fn a_register_coordinates_are_bounded_v1",
        "pub proof fn b_register_coordinates_are_bounded_v1",
        "pub proof fn accumulator_coordinates_are_bounded_v1",
        "pub proof fn lane_component_register_maps_are_injective_v1",
        "pub proof fn xor4_physical_index_is_bounded_v1",
        "pub proof fn xor4_column_round_trips_v1",
        "pub proof fn xor4_logical_coordinate_round_trips_v1",
        "pub proof fn xor4_physical_index_is_injective_v1",
        "pub proof fn xor4_physical_layout_is_permutation_v1",
        "pub proof fn a_and_b_staging_are_bounded_v1",
        "pub proof fn distinct_lane_components_have_disjoint_a_lds_v1",
        "pub proof fn distinct_lane_components_have_disjoint_b_lds_v1",
        "pub proof fn checked_workgroup_origin_stays_in_bounds_v1",
        "pub proof fn distinct_logical_coordinates_have_distinct_row_major_v1",
        "pub proof fn checked_accumulator_output_is_in_bounds_v1",
        "pub proof fn all_unequal_invocations_own_disjoint_global_c_v1",
        "pub proof fn a_phase_load_is_in_bounds_v1",
        "pub proof fn b_phase_load_is_in_bounds_v1",
        "pub proof fn k_phases_partition_every_depth_v1",
        "pub proof fn distinct_k_phase_offsets_are_disjoint_v1",
        "pub proof fn checked_matrix_addresses_fit_u64_v1",
        "pub proof fn empty_output_no_dispatch_reads_no_operands_v1",
        "pub proof fn zero_k_host_fill_reads_no_operands_v1",
    ];

    assert_eq!(
        source.matches("pub proof fn ").count(),
        theorem_markers.len()
    );
    for marker in theorem_markers {
        assert!(source.contains(marker), "missing theorem marker {marker}");
    }
    for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
        assert!(!source.contains(shortcut), "forbidden shortcut {shortcut}");
    }
}

#[test]
fn executable_rust_and_recognized_verus_formulas_correspond_exhaustively() {
    let source = std::str::from_utf8(PROOF_SOURCE).unwrap();
    let xor2 = Xor2Parser::parse(&normalized_spec_body(source, "xor2_v1"));
    let a_row = parse_formula(source, "a_register_row_v1");
    let a_depth = parse_formula(source, "a_register_depth_v1");
    let b_depth = parse_formula(source, "b_register_depth_v1");
    let b_col = parse_formula(source, "b_register_col_v1");
    let accumulator_row = parse_formula(source, "accumulator_row_v1");
    let accumulator_col = parse_formula(source, "accumulator_col_v1");
    let xor4_col = parse_formula(source, "xor4_lds_col_v1");
    let xor4_index = parse_formula(source, "xor4_lds_index_v1");
    let a_staging = parse_formula(source, "a_lds_index_v1");
    let b_staging = parse_formula(source, "b_transposed_lds_index_v1");

    for (left, row) in OFFICIAL_XOR2_TABLE.iter().enumerate() {
        for (right, expected) in row.iter().enumerate() {
            assert_eq!(xor2.evaluate(left, right), *expected);
        }
    }

    for lane in 0..64 {
        for component in 0..4 {
            let a = ARegisterLayoutV1::coordinate(lane, component).unwrap();
            let b = BRegisterLayoutV1::coordinate(lane, component).unwrap();
            let accumulator = AccumulatorRegisterLayoutV1::coordinate(lane, component).unwrap();
            assert_eq!(a.row, evaluate_lane_formula(a_row, lane, component));
            assert_eq!(a.depth, evaluate_lane_formula(a_depth, lane, component));
            assert_eq!(b.depth, evaluate_lane_formula(b_depth, lane, component));
            assert_eq!(b.column, evaluate_lane_formula(b_col, lane, component));
            assert_eq!(
                accumulator.row,
                evaluate_lane_formula(accumulator_row, lane, component)
            );
            assert_eq!(
                accumulator.column,
                evaluate_lane_formula(accumulator_col, lane, component)
            );
            assert_eq!(
                RowMajorXor4StagingV1::a_coordinate(lane, component)
                    .unwrap()
                    .physical_index,
                evaluate_staging_formula(a_staging, &xor2, lane, component)
            );
            assert_eq!(
                RowMajorXor4StagingV1::b_transposed_coordinate(lane, component)
                    .unwrap()
                    .physical_index,
                evaluate_staging_formula(b_staging, &xor2, lane, component)
            );
        }
    }

    for row in 0..16 {
        for column in 0..16 {
            let expected_column = evaluate_lds_formula(xor4_col, &xor2, row, column);
            let expected_index = evaluate_lds_formula(xor4_index, &xor2, row, column);
            assert_eq!(expected_index, row * 16 + expected_column);
            assert_eq!(
                RowMajorXor4::physical_index(row, column),
                Some(expected_index)
            );
            assert_eq!(
                RowMajorXor4StagingV1::physical(LdsLogicalCoordinateV1 { row, column })
                    .unwrap()
                    .physical_index,
                expected_index
            );
        }
    }
}

#[test]
fn matching_version_fake_verus_is_rejected_by_executable_digest() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fake = manifest.join("tests/fixtures/fake-verus-matching-version.sh");
    let runner = manifest.join("run-verus.sh");
    let fake_source = std::fs::read_to_string(&fake).unwrap();
    assert!(fake_source.contains(PINNED_VERUS_VERSION));

    let output = Command::new(runner).env("VERUS", fake).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Verus executable SHA-256"));
    assert!(stderr.contains("does not match pinned"));
    assert!(!stderr.contains("fake verifier must never reach proof execution"));
}
