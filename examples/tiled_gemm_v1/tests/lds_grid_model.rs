use std::collections::BTreeSet;

use fe2o3_tiled_gemm_v1::{ARegisterLayoutV1, AccumulatorRegisterLayoutV1, BRegisterLayoutV1};

const TILE: u64 = 16;
const LANES: u64 = 64;
const COMPONENTS: u64 = 4;

#[derive(Clone, Copy, Debug)]
struct CheckedGrid {
    m: u64,
    n: u64,
    lda: u64,
    ldb: u64,
    ldc: u64,
    tiles_x: u64,
    tiles_y: u64,
    a_len: u64,
    b_len: u64,
    c_len: u64,
}

fn strided_footprint(rows: u64, columns: u64, stride: u64) -> Option<u64> {
    rows.checked_sub(1)?
        .checked_mul(stride)?
        .checked_add(columns)
}

impl CheckedGrid {
    fn derive(m: u64, n: u64, lda: u64, ldb: u64, ldc: u64) -> Option<Self> {
        if m == 0
            || n == 0
            || m > u32::MAX.into()
            || n > u32::MAX.into()
            || !m.is_multiple_of(TILE)
            || !n.is_multiple_of(TILE)
            || lda < TILE
            || ldb < n
            || ldc < n
            || lda > u32::MAX.into()
            || ldb > u32::MAX.into()
            || ldc > u32::MAX.into()
        {
            return None;
        }

        let tiles_x = n / TILE;
        let tiles_y = m / TILE;
        let threads_x = tiles_x.checked_mul(LANES)?;
        let workgroups = tiles_x.checked_mul(tiles_y)?;
        if threads_x > u32::MAX.into() || tiles_y > u32::MAX.into() || workgroups > u32::MAX.into()
        {
            return None;
        }

        let a_len = strided_footprint(m, TILE, lda)?;
        let b_len = strided_footprint(TILE, n, ldb)?;
        let c_len = strided_footprint(m, n, ldc)?;
        a_len.checked_mul(2)?;
        b_len.checked_mul(2)?;
        c_len.checked_mul(4)?;

        Some(Self {
            m,
            n,
            lda,
            ldb,
            ldc,
            tiles_x,
            tiles_y,
            a_len,
            b_len,
            c_len,
        })
    }

    fn a_index(self, group_y: u64, lane: u64, component: u64) -> u64 {
        let coordinate = ARegisterLayoutV1::coordinate(lane as usize, component as usize)
            .expect("bounded A register coordinate");
        (group_y * TILE + coordinate.row as u64) * self.lda + coordinate.depth as u64
    }

    fn b_index(self, group_x: u64, lane: u64, component: u64) -> u64 {
        let coordinate = BRegisterLayoutV1::coordinate(lane as usize, component as usize)
            .expect("bounded B register coordinate");
        coordinate.depth as u64 * self.ldb + group_x * TILE + coordinate.column as u64
    }

    fn c_index(self, group_x: u64, group_y: u64, lane: u64, component: u64) -> u64 {
        let coordinate = AccumulatorRegisterLayoutV1::coordinate(lane as usize, component as usize)
            .expect("bounded C register coordinate");
        (group_y * TILE + coordinate.row as u64) * self.ldc
            + group_x * TILE
            + coordinate.column as u64
    }
}

fn exercise_checked_grid(grid: CheckedGrid) {
    assert_eq!(grid.tiles_x * TILE, grid.n);
    assert_eq!(grid.tiles_y * TILE, grid.m);
    assert!(grid.tiles_x * LANES <= u32::MAX.into());
    assert!(grid.tiles_x * grid.tiles_y <= u32::MAX.into());

    let mut tile_ids = BTreeSet::new();
    let mut c_owners = BTreeSet::new();
    for group_y in 0..grid.tiles_y {
        for group_x in 0..grid.tiles_x {
            assert!(tile_ids.insert(group_y * grid.tiles_x + group_x));
            for lane in 0..LANES {
                for component in 0..COMPONENTS {
                    let a_index = grid.a_index(group_y, lane, component);
                    let b_index = grid.b_index(group_x, lane, component);
                    let c_index = grid.c_index(group_x, group_y, lane, component);

                    assert!(a_index < grid.a_len);
                    assert!(b_index < grid.b_len);
                    assert!(c_index < grid.c_len);
                    assert!((a_index + 1).checked_mul(2).is_some());
                    assert!((b_index + 1).checked_mul(2).is_some());
                    assert!((c_index + 1).checked_mul(4).is_some());
                    assert!(c_index % grid.ldc < grid.n, "store reached C row padding");
                    assert!(c_owners.insert(c_index), "colliding C owner at {c_index}");
                }
            }
        }
    }

    assert_eq!(tile_ids.len() as u64, grid.tiles_x * grid.tiles_y);
    assert_eq!(c_owners.len() as u64, grid.m * grid.n);
    let logical_c: BTreeSet<_> = (0..grid.m)
        .flat_map(|row| (0..grid.n).map(move |column| row * grid.ldc + column))
        .collect();
    assert_eq!(c_owners, logical_c);
}

#[test]
fn exhaustive_grid_ownership_and_bounds_cover_representative_padded_strides() {
    for m_tiles in 1..=3 {
        for n_tiles in 1..=3 {
            let m = m_tiles * TILE;
            let n = n_tiles * TILE;
            for lda_padding in 0..=2 {
                for ldb_padding in 0..=2 {
                    for ldc_padding in 0..=2 {
                        let grid = CheckedGrid::derive(
                            m,
                            n,
                            TILE + lda_padding,
                            n + ldb_padding,
                            n + ldc_padding,
                        )
                        .expect("representative grid is checked");
                        exercise_checked_grid(grid);
                    }
                }
            }
        }
    }

    exercise_checked_grid(
        CheckedGrid::derive(64, 48, 33, 79, 96).expect("larger padded grid is checked"),
    );
}

#[test]
fn checked_grid_rejects_tails_undersized_strides_and_u32_grid_overflow() {
    assert!(CheckedGrid::derive(0, 16, 16, 16, 16).is_none());
    assert!(CheckedGrid::derive(16, 0, 16, 16, 16).is_none());
    assert!(CheckedGrid::derive(17, 16, 16, 16, 16).is_none());
    assert!(CheckedGrid::derive(16, 17, 16, 17, 17).is_none());
    assert!(CheckedGrid::derive(32, 16, 15, 16, 16).is_none());
    assert!(CheckedGrid::derive(16, 32, 16, 31, 32).is_none());
    assert!(CheckedGrid::derive(16, 32, 16, 32, 31).is_none());

    let too_many_x_threads = (u32::MAX as u64 / LANES + 1) * TILE;
    assert!(too_many_x_threads <= u32::MAX.into());
    assert!(
        CheckedGrid::derive(
            16,
            too_many_x_threads,
            16,
            too_many_x_threads,
            too_many_x_threads,
        )
        .is_none()
    );
}

#[test]
fn rejected_mutations_have_concrete_collisions_or_out_of_bounds_accesses() {
    let collapsed_tile_col = |_group_x: u64| 0_u64;
    assert_eq!(collapsed_tile_col(0), collapsed_tile_col(1));

    let undersized_lda = 15_u64;
    let undersized_a_len = 32 * undersized_lda;
    let last_a_index = (16 + 15) * undersized_lda + 15;
    assert_eq!(last_a_index, undersized_a_len);
    assert!(last_a_index >= undersized_a_len);

    let c_without_group_x =
        |_group_x: u64, group_y: u64, row: u64, column: u64| (group_y * TILE + row) * 32 + column;
    assert_eq!(c_without_group_x(0, 0, 0, 0), c_without_group_x(1, 0, 0, 0));
}
