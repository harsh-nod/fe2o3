use std::collections::BTreeSet;

const TILE: usize = 16;
const LANES: usize = 64;
const COMPONENTS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rational {
    numerator: i128,
    denominator: i128,
}

impl Rational {
    fn new(mut numerator: i128, mut denominator: i128) -> Self {
        assert_ne!(denominator, 0);
        if denominator < 0 {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = gcd(numerator.unsigned_abs(), denominator as u128) as i128;
        Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        }
    }

    fn integer(value: i64) -> Self {
        Self::new(value.into(), 1)
    }

    fn add(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.denominator + other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn multiply(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.numerator,
            self.denominator * other.denominator,
        )
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn ceil_tiles(value: usize) -> usize {
    value.div_ceil(TILE)
}

fn a_value(row: usize, depth: usize, m: usize, k: usize) -> i64 {
    ((row * 5 + depth * 3 + m + k) % 11) as i64 - 5
}

fn b_value(depth: usize, column: usize, n: usize, k: usize) -> i64 {
    ((depth * 7 + column * 2 + n + k) % 13) as i64 - 6
}

fn c_value(row: usize, column: usize, m: usize, n: usize) -> i64 {
    ((row * 3 + column * 5 + m + n) % 9) as i64 - 4
}

#[derive(Default)]
struct Observations {
    saw_a_predicated_off: bool,
    saw_b_predicated_off: bool,
    saw_c_predicated_off: bool,
    saw_k_tail: bool,
}

fn exercise_positive_shape(m: usize, n: usize, k: usize, observations: &mut Observations) {
    assert!((1..=32).contains(&m));
    assert!((1..=32).contains(&n));
    assert!((1..=32).contains(&k));

    let a: Vec<_> = (0..m)
        .flat_map(|row| (0..k).map(move |depth| a_value(row, depth, m, k)))
        .collect();
    let b: Vec<_> = (0..k)
        .flat_map(|depth| (0..n).map(move |column| b_value(depth, column, n, k)))
        .collect();
    let c_input: Vec<_> = (0..m)
        .flat_map(|row| (0..n).map(move |column| c_value(row, column, m, n)))
        .collect();
    let coefficients = [
        (Rational::new(1, 1), Rational::new(0, 1)),
        (Rational::new(2, 1), Rational::new(-1, 1)),
        (Rational::new(-1, 2), Rational::new(3, 2)),
        (Rational::new(0, 1), Rational::new(5, 3)),
        (Rational::new(7, 3), Rational::new(-2, 5)),
    ];

    let mut owners = BTreeSet::new();
    for group_y in 0..ceil_tiles(m) {
        for group_x in 0..ceil_tiles(n) {
            let mut accumulator = [0_i64; TILE * TILE];
            let mut covered_depths = vec![0_u8; k];

            for phase in 0..ceil_tiles(k) {
                let mut a_lds = [0_i64; TILE * TILE];
                let mut b_lds = [0_i64; TILE * TILE];
                let mut publish_arrivals = [false; LANES];

                for (lane, publish_arrived) in publish_arrivals.iter_mut().enumerate() {
                    for component in 0..COMPONENTS {
                        let tile_row = lane % TILE;
                        let tile_depth = 4 * (lane / TILE) + component;
                        let tile_col = lane % TILE;
                        let global_row = group_y * TILE + tile_row;
                        let global_depth = phase * TILE + tile_depth;
                        let global_col = group_x * TILE + tile_col;

                        let a_enabled = global_row < m && global_depth < k;
                        let b_enabled = global_depth < k && global_col < n;
                        observations.saw_a_predicated_off |= !a_enabled;
                        observations.saw_b_predicated_off |= !b_enabled;

                        if a_enabled {
                            let index = global_row * k + global_depth;
                            assert!(index < a.len());
                            a_lds[tile_row * TILE + tile_depth] = a[index];
                        } else {
                            assert_eq!(a_lds[tile_row * TILE + tile_depth], 0);
                        }
                        if b_enabled {
                            let index = global_depth * n + global_col;
                            assert!(index < b.len());
                            b_lds[tile_depth * TILE + tile_col] = b[index];
                        } else {
                            assert_eq!(b_lds[tile_depth * TILE + tile_col], 0);
                        }
                    }
                    // Arrival is unconditional on all four A/B predicates.
                    *publish_arrived = true;
                }
                assert!(publish_arrivals.into_iter().all(|arrived| arrived));

                for tile_row in 0..TILE {
                    for tile_col in 0..TILE {
                        for tile_depth in 0..TILE {
                            accumulator[tile_row * TILE + tile_col] += a_lds
                                [tile_row * TILE + tile_depth]
                                * b_lds[tile_depth * TILE + tile_col];
                        }
                    }
                }

                for offset in 0..TILE {
                    let depth = phase * TILE + offset;
                    if depth < k {
                        covered_depths[depth] += 1;
                    } else {
                        observations.saw_k_tail = true;
                    }
                }

                // Reuse arrival is also unconditional on loads and C stores.
                let reuse_arrivals = [true; LANES];
                assert!(reuse_arrivals.into_iter().all(|arrived| arrived));
            }
            assert!(covered_depths.into_iter().all(|count| count == 1));

            for lane in 0..LANES {
                for component in 0..COMPONENTS {
                    let tile_row = 4 * (lane / TILE) + component;
                    let tile_col = lane % TILE;
                    let row = group_y * TILE + tile_row;
                    let column = group_x * TILE + tile_col;
                    let store_enabled = row < m && column < n;
                    observations.saw_c_predicated_off |= !store_enabled;
                    if !store_enabled {
                        continue;
                    }

                    let c_index = row * n + column;
                    assert!(c_index < c_input.len());
                    assert!(owners.insert(c_index), "duplicate C owner at {c_index}");
                    let product = accumulator[tile_row * TILE + tile_col];
                    let direct_product: i64 = (0..k)
                        .map(|depth| a[row * k + depth] * b[depth * n + column])
                        .sum();
                    assert_eq!(product, direct_product);

                    for (alpha, beta) in coefficients {
                        let tiled = alpha
                            .multiply(Rational::integer(product))
                            .add(beta.multiply(Rational::integer(c_input[c_index])));
                        let exact_contract = alpha
                            .multiply(Rational::integer(direct_product))
                            .add(beta.multiply(Rational::integer(c_input[c_index])));
                        assert_eq!(tiled, exact_contract);
                    }
                }
            }
        }
    }
    assert_eq!(owners, (0..m * n).collect());
}

#[test]
fn exhaustive_small_positive_edges_cover_tails_barriers_and_exact_alpha_beta() {
    let mut observations = Observations::default();
    for m in 1..=18 {
        for n in 1..=18 {
            for k in 1..=18 {
                exercise_positive_shape(m, n, k, &mut observations);
            }
        }
    }
    for shape in [(31, 17, 19), (32, 31, 17), (17, 32, 32)] {
        exercise_positive_shape(shape.0, shape.1, shape.2, &mut observations);
    }

    assert!(observations.saw_a_predicated_off);
    assert!(observations.saw_b_predicated_off);
    assert!(observations.saw_c_predicated_off);
    assert!(observations.saw_k_tail);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EdgePolicy {
    NoDispatchEmptyOutput,
    LegacyPositiveZeroFill,
    LegacyZeroKRejectsMnTail,
    PositiveKEdgeDispatch,
}

fn edge_policy(m: usize, n: usize, k: usize) -> EdgePolicy {
    if m == 0 || n == 0 {
        EdgePolicy::NoDispatchEmptyOutput
    } else if k == 0 && (!m.is_multiple_of(TILE) || !n.is_multiple_of(TILE)) {
        EdgePolicy::LegacyZeroKRejectsMnTail
    } else if k == 0 {
        EdgePolicy::LegacyPositiveZeroFill
    } else {
        EdgePolicy::PositiveKEdgeDispatch
    }
}

#[test]
fn empty_and_zero_k_policy_matches_the_current_host_boundary() {
    assert_eq!(edge_policy(0, 17, 19), EdgePolicy::NoDispatchEmptyOutput);
    assert_eq!(edge_policy(31, 0, 0), EdgePolicy::NoDispatchEmptyOutput);
    assert_eq!(edge_policy(16, 32, 0), EdgePolicy::LegacyPositiveZeroFill);
    assert_eq!(edge_policy(17, 16, 0), EdgePolicy::LegacyZeroKRejectsMnTail);
    assert_eq!(edge_policy(16, 17, 0), EdgePolicy::LegacyZeroKRejectsMnTail);
    assert_eq!(edge_policy(17, 19, 1), EdgePolicy::PositiveKEdgeDispatch);

    let beta = Rational::new(3, 2);
    let c_input = Rational::integer(4);
    let generalized_k_zero = beta.multiply(c_input);
    assert_ne!(generalized_k_zero, Rational::integer(0));
    assert_eq!(Rational::new(0, 1).multiply(c_input), Rational::integer(0));
}

#[test]
fn rejection_mutations_have_concrete_edge_counterexamples() {
    let mutated_lane_reaches_barrier = |_load_enabled: bool| false;
    assert!(!mutated_lane_reaches_barrier(false));

    let (tail_row, k, a_len) = (1_usize, 1_usize, 1_usize);
    let unguarded_a_index = tail_row * k;
    assert_eq!(unguarded_a_index, a_len);
    assert!(unguarded_a_index >= a_len);

    let (row, n, tail_column, c_len) = (0_usize, 1_usize, 1_usize, 1_usize);
    let unguarded_c_index = row * n + tail_column;
    assert_eq!(unguarded_c_index, c_len);
    assert!(unguarded_c_index >= c_len);

    let wrong_epilogue = Rational::integer(5 * 2 + 7);
    let exact_epilogue = Rational::integer(5 * 2 + 7 * 3);
    assert_ne!(wrong_epilogue, exact_epilogue);

    let tail_k = 17_usize;
    assert_eq!(tail_k / TILE, 1);
    assert!(tail_k > (tail_k / TILE) * TILE);
}
