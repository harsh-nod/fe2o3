const SOURCE: &str = concat!(
    include_str!("../../verus/binary32_rne_v1.rs"),
    "\n",
    include_str!("../../verus/binary32_rne_composition_v1.rs"),
);
const TOO_TIGHT: &str = r#"verus! {
proof fn fe2o3_negative_two_result_too_tight_v1() {
    fe2o3_two_rounded_nonzero_example_v1();
    assert(fe2o3_ar_goal_v1(33554432, 33554440, 33554432, 1, 1, 16777216));
}
}
"#;

fn programs() -> [CanonicalGeneratedVerusProofInputV3; 2] {
    [SOURCE.to_owned(), format!("{SOURCE}\n{TOO_TIGHT}")]
        .map(|source| CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).unwrap())
}

#[test]
fn composition_sources_are_distinct_bounded_and_not_proof_authority() {
    // Regression guard for these retained sources, not a parser or admission check.
    let forbidden_form = |source: &str| {
        let compact: String = source
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        [
            "by(compute)",
            "by(compute_only)",
            "spinoff_prover",
            "nonlinear_arith",
            "by(bit_vector)",
            "assume(",
            "admit(",
            "external_body",
        ]
        .into_iter()
        .find(|form| compact.contains(*form))
    };
    assert_eq!(forbidden_form(SOURCE), None);
    let model = include_str!("../../verus/binary32_rne_v1.rs");
    let companion = include_str!("../../verus/binary32_rne_composition_v1.rs");
    for rejected in [
        "assert(true) by(compute);",
        "assert(true) by ( compute_only );",
        "assert(true) by\n(\tcompute );",
        "#[verifier :: spinoff_prover]",
        "assert(true) by ( nonlinear_arith );",
        "assert(true) by ( bit_vector );",
    ] {
        for source in [
            format!("{model}\n{rejected}\n{companion}"),
            format!("{model}\n{companion}\n{rejected}"),
        ] {
            assert!(forbidden_form(&source).is_some(), "{rejected}");
        }
    }
    let [positive, negative] = programs();
    assert_ne!(positive.identity(), negative.identity());
    for source in [&positive, &negative] {
        assert!(!source.authenticates_verus_execution());
        assert!(!source.grants_artifact_or_runtime_authority());
    }
}

// Small exact-integer test oracle only; the universal theorem is Verus source.
fn sufficient(t: i128, d: i128, eg: i128, er: i128, a: i128, rho: i128, k: i128) -> bool {
    d > 0
        && k > 0
        && a >= 0
        && rho >= 0
        && eg >= 0
        && er >= 0
        && (eg + er) * k <= a * d + (t.abs() - er).max(0) * rho
}

fn goal(g: i128, r: i128, d: i128, a: i128, rho: i128, k: i128) -> bool {
    (g - r).abs() * k <= a * d + r.abs() * rho
}

fn units(n: i128, d: i128) -> i128 {
    let q = n / d;
    let rem = n % d;
    q + i128::from(2 * rem > d || (2 * rem == d && q % 2 != 0))
}

fn normal_domain(n: i128, d: i128, e: i128) -> bool {
    d > 0
        && 8_388_608 * d <= n
        && n < 16_777_216 * d
        && (-149..=104).contains(&e)
        && (e != 104 || n <= 16_777_215 * d)
}

#[test]
fn two_independent_inputs_have_nonzero_error_and_mixed_ar_bound() {
    let (ng, nr, d, t) = (16_777_217, 16_777_219, 2, 16_777_218);
    assert!(normal_domain(ng, d, -23) && normal_domain(nr, d, -23));
    assert_eq!((ng - t).abs(), 1);
    assert_eq!((nr - t).abs(), 1);
    let (g, r, target, den) = (
        2 * units(ng, d) * d,
        2 * units(nr, d) * d,
        2 * t,
        4 * (1 << 23),
    );
    assert_eq!(
        (g, r, target, den),
        (33_554_432, 33_554_440, 33_554_436, 33_554_432)
    );
    assert_eq!((g - target).abs(), 4);
    assert_eq!((r - target).abs(), 4);
    assert_ne!(g, r);
    assert!(sufficient(target, den, 4, 4, 1, 1, 1 << 23));
    assert!(goal(g, r, den, 1, 1, 1 << 23));
    assert!(!sufficient(target, den, 4, 4, 1, 1, 1 << 24));
    assert!(!goal(g, r, den, 1, 1, 1 << 24));
}

#[test]
fn identical_exact_inputs_are_not_a_nonzero_example() {
    for d in 1..=16 {
        for delta in 0..=32 {
            let n = 8_388_608 * d + delta;
            assert_eq!(units(n, d), units(n * 3, d * 3));
        }
    }
}

#[test]
fn common_target_lower_bound_cannot_be_replaced_by_target_magnitude() {
    assert!(!sufficient(1, 1, 0, 1, 0, 1, 1));
    assert!(!goal(1, 0, 1, 0, 1, 1));
    // Using |target| instead of max(0, |target|-reference_error) would pass.
    assert!((0 + 1) <= 1);
}

#[test]
fn invalid_denominators_and_negative_budgets_do_not_satisfy_the_contract() {
    for (d, eg, er, a, rho, k) in [
        (0, 1, 1, 2, 1, 1),
        (-1, 1, 1, 2, 1, 1),
        (1, 1, 1, 2, 1, 0),
        (1, 1, 1, 2, 1, -1),
        (1, -1, 1, 2, 1, 1),
        (1, 1, -1, 2, 1, 1),
        (1, 1, 1, -1, 1, 1),
        (1, 1, 1, 2, -1, 1),
    ] {
        assert!(!sufficient(1, d, eg, er, a, rho, k));
    }
}

#[test]
fn signed_and_zero_target_composition_matches_bounded_integer_oracle() {
    let mut accepted = 0;
    for g in -4i128..=4 {
        for r in -4i128..=4 {
            for t in -4i128..=4 {
                for gs in 0..=1 {
                    for rs in 0..=1 {
                        for k in 1..=4 {
                            for a in 0..=4 {
                                for rho in 0..=4 {
                                    if sufficient(
                                        t,
                                        3,
                                        (g - t).abs() + gs,
                                        (r - t).abs() + rs,
                                        a,
                                        rho,
                                        k,
                                    ) {
                                        assert!(goal(g, r, 3, a, rho, k));
                                        accepted += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(accepted > 0);
}

#[test]
fn independent_rounding_errors_compose_before_ar_selection() {
    for d in 1i128..=16 {
        for tg in 0..=8 {
            for tr in 0..=8 {
                let t = 8_388_612 * d;
                let ng = t + tg - 4;
                let nr = t + tr - 4;
                assert!(normal_domain(ng, d, -23) && normal_domain(nr, d, -23));
                let eg = d + 2 * (ng - t).abs();
                let er = d + 2 * (nr - t).abs();
                let g = 2 * units(ng, d) * d;
                let r = 2 * units(nr, d) * d;
                assert!((g - 2 * t).abs() <= eg && (r - 2 * t).abs() <= er);
                assert!((g - r).abs() <= eg + er);
            }
        }
    }
}

#[test]
fn binary32_domain_endpoints_carry_and_exclusions_remain_explicit() {
    assert!(normal_domain(8_388_608, 1, -149));
    assert!(normal_domain(16_777_215, 1, 104));
    assert!(normal_domain(33_554_431, 2, 103));
    assert_eq!(units(33_554_431, 2), 16_777_216);
    assert!(!normal_domain(33_554_431, 2, 104));
    assert!(!normal_domain(8_388_607, 1, -149));
    assert!(!normal_domain(0, 1, -149));
    assert!(!normal_domain(-8_388_608, 1, -149));
    assert!(!normal_domain(8_388_608, 0, -149));
    assert!(!normal_domain(8_388_608, 1, -150));
    assert!(!normal_domain(8_388_608, 1, 105));
}
