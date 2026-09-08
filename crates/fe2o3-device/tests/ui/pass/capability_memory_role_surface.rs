#![allow(dead_code)]

use fe2o3_device::DisjointIndex;
use fe2o3_device::capability_memory::{DisjointWrite, Global, Private, ReadOnly, Workgroup};

enum KernelBrand {}
enum OutputMapping {}

fn global_contract<'kernel>(
    input: &Global<'kernel, u32, ReadOnly, KernelBrand>,
    output: &mut Global<'kernel, u32, DisjointWrite<OutputMapping>, KernelBrand>,
    index: DisjointIndex<OutputMapping, KernelBrand>,
) {
    let _: usize = input.len();
    let _: bool = input.is_empty();
    let _: Option<u32> = input.load(0);
    let _: bool = output.store(index, 7);
}

fn address_spaces_are_explicit<'kernel>(
    workgroup: &Workgroup<'kernel, u32, ReadOnly, KernelBrand>,
    private: &Private<'kernel, u32, ReadOnly, KernelBrand>,
) {
    let _: usize = workgroup.len();
    let _: bool = private.is_empty();
}

fn main() {}
