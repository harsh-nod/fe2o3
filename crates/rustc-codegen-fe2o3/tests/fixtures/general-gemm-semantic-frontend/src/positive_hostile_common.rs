#![allow(dead_code)]

use fe2o3_device::trap;

static SUBSTITUTE: u16 = 0x3f80;

pub fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    (u64::from(rows - 1) * u64::from(stride) + u64::from(columns)) as usize
}

pub fn canonical_load(
    values: &[u16],
    row: u64,
    column: u64,
    rows: u32,
    columns: u32,
    stride: u32,
) -> u16 {
    if row >= u64::from(rows) || column >= u64::from(columns) {
        return 0;
    }
    let index = row * u64::from(stride) + column;
    let Some(value) = values.get(index as usize) else {
        trap();
    };
    *value
}

pub fn split_provenance_load(
    values: &[u16],
    row: u64,
    column: u64,
    rows: u32,
    columns: u32,
    stride: u32,
) -> u16 {
    if row >= u64::from(rows) || column >= u64::from(columns) {
        return 0;
    }
    let index = row * u64::from(stride) + column;
    let Some(value) = values.get(index as usize) else {
        trap();
    };
    let selected = if row == 0 { &SUBSTITUTE } else { value };
    *selected
}
