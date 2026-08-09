#[inline(never)]
fn panic_sink() -> u32 {
    panic!("live monomorphization branch reached panic")
}

#[inline(never)]
fn identity(value: u32) -> u32 {
    value
}

#[inline(never)]
fn invoke_indirect(function: fn(u32) -> u32, value: u32) -> u32 {
    function(value)
}

#[inline(never)]
fn address_space_violation(value: u32) -> u32 {
    let pointer = value as usize as *const u32;
    unsafe { core::ptr::read_volatile(pointer) }
}

#[inline(never)]
fn local_const_panic<const DEAD: bool>(value: u32) -> u32 {
    if DEAD { panic_sink() } else { value }
}

#[inline(never)]
fn local_const_unsupported<const DEAD: bool>(value: u32) -> u32 {
    if DEAD {
        invoke_indirect(identity, value)
    } else {
        value
    }
}

#[inline(never)]
fn alias_panic(value: u32) -> u32 {
    let mut select = 0_u32;
    let reference = &mut select;
    *reference = 1;
    match select {
        0 => value,
        _ => panic_sink(),
    }
}

#[inline(never)]
fn alias_unsupported(value: u32) -> u32 {
    let mut select = 0_u32;
    let reference = &mut select;
    *reference = 1;
    match select {
        0 => value,
        _ => invoke_indirect(identity, value),
    }
}

#[inline(never)]
fn target_size(value: u32) -> u32 {
    const SELECT: u32 = core::mem::size_of::<usize>() as u32;
    match SELECT {
        8 => invoke_indirect(identity, value),
        _ => value,
    }
}

#[inline(never)]
fn local_const_address<const DEAD: bool>(value: u32) -> u32 {
    if DEAD {
        address_space_violation(value)
    } else {
        value
    }
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_dead_branches(seed: u32) -> u32 {
    #[cfg(local_const_panic)]
    return local_const_panic::<false>(seed);
    #[cfg(local_const_unsupported)]
    return local_const_unsupported::<false>(seed);
    #[cfg(alias_panic)]
    return alias_panic(seed);
    #[cfg(alias_unsupported)]
    return alias_unsupported(seed);
    #[cfg(target_size)]
    return target_size(seed);
    #[cfg(local_const_address)]
    return local_const_address::<false>(seed);
    seed
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_dead_branches: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(u32) -> u32,
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "dead_branches",
    "dead_branches",
    fe2o3_kernel_dead_branches,
);

fn main() {}
