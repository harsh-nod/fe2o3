use fe2o3_device::{DisjointSlice, kernel, thread};

type Pair = (u32, (), u32);

#[inline(never)]
fn once<F: FnOnce(u32, (), u32) -> Pair>(f: F, a: u32, b: u32) -> Pair {
    f(a, (), b)
}

#[inline(never)]
fn via_fn<F: Fn(u32, (), u32) -> Pair>(f: F, a: u32, b: u32) -> Pair {
    once(f, a, b)
}

#[inline(never)]
fn via_mut<F: FnMut(u32, (), u32) -> Pair>(f: F, a: u32, b: u32) -> Pair {
    once(f, a, b)
}

#[cfg(not(any(
    feature = "rust_call_natural_borrowed",
    feature = "rust_call_natural_env"
)))]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn rust_call_natural_shims(
    seed: u32,
    lhs: u32,
    rhs: u32,
    input: &[u32],
    other: &[u32],
    mut shared_first: DisjointSlice<u32>,
    mut shared_last: DisjointSlice<u32>,
    mut mutable_before: DisjointSlice<u32>,
    mut mutable_after: DisjointSlice<u32>,
) {
    let a = lhs.wrapping_add(thread::thread_idx_x());
    let b = rhs.wrapping_sub(thread::block_idx_x());
    let shared = move |a: u32, (): (), b: u32| {
        let index = a as usize;
        let left = if index < input.len() { input[index] } else { 0 };
        let right = if index < other.len() { other[index] } else { 0 };
        ((seed ^ a).wrapping_add(left), (), b.wrapping_sub(right))
    };
    let shared = via_fn(shared, a, b);

    let mut state = seed;
    let mutable = move |a: u32, (): (), b: u32| {
        let before = state;
        state = state.wrapping_add(a).wrapping_sub(b);
        (before, (), state)
    };
    let mutable = via_mut(mutable, a, b);

    if let Some(value) = shared_first.get_mut(thread::index_1d()) {
        *value = shared.0;
    }
    if let Some(value) = shared_last.get_mut(thread::index_1d()) {
        *value = shared.2;
    }
    if let Some(value) = mutable_before.get_mut(thread::index_1d()) {
        *value = mutable.0;
    }
    if let Some(value) = mutable_after.get_mut(thread::index_1d()) {
        *value = mutable.2;
    }
}

#[inline(never)]
fn borrowed_mut<F: FnMut(u32, (), u32) -> Pair>(f: &mut F, a: u32, b: u32) -> Pair {
    f(a, (), b)
}

#[cfg(feature = "rust_call_natural_borrowed")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn rust_call_natural_borrowed(
    seed: u32,
    lhs: u32,
    rhs: u32,
    mut first_before: DisjointSlice<u32>,
    mut first_after: DisjointSlice<u32>,
    mut second_before: DisjointSlice<u32>,
    mut second_after: DisjointSlice<u32>,
    mut caller_state: DisjointSlice<u32>,
) {
    let a = lhs.wrapping_add(thread::thread_idx_x());
    let b = rhs.wrapping_sub(thread::block_idx_x());
    let mut state = seed;
    let (first, second) = {
        let mut mutable = |a: u32, (): (), b: u32| {
            let before = state;
            state = state.wrapping_add(a).wrapping_sub(b);
            (before, (), state)
        };
        let first = borrowed_mut(&mut mutable, a, b);
        let second = borrowed_mut(&mut mutable, b.wrapping_add(7), a ^ 0x1357_9bdf);
        (first, second)
    };
    let observed = state.wrapping_add(seed ^ 0x2468_ace0);
    if let Some(value) = first_before.get_mut(thread::index_1d()) {
        *value = first.0;
    }
    if let Some(value) = first_after.get_mut(thread::index_1d()) {
        *value = first.2;
    }
    if let Some(value) = second_before.get_mut(thread::index_1d()) {
        *value = second.0;
    }
    if let Some(value) = second_after.get_mut(thread::index_1d()) {
        *value = second.2;
    }
    if let Some(value) = caller_state.get_mut(thread::index_1d()) {
        *value = observed;
    }
}

struct Env<'a> {
    state: u32,
    unit: (),
    input: &'a [u32],
    other: &'a [u32],
}

#[inline(never)]
fn read_env(env: &Env<'_>, alias: &Env<'_>, a: u32, b: u32) -> Pair {
    let index = a as usize;
    let left = if index < env.input.len() {
        env.input[index]
    } else {
        0
    };
    let right = if index < alias.other.len() {
        alias.other[index]
    } else {
        0
    };
    (
        (env.state ^ a).wrapping_add(left),
        env.unit,
        b.wrapping_sub(alias.state).wrapping_sub(right),
    )
}

#[inline(never)]
fn mutate_env(env: &mut Env<'_>, arguments: Pair) -> Pair {
    let before = env.state;
    env.state = before.wrapping_add(arguments.0).wrapping_sub(arguments.2);
    (before, env.unit, env.state)
}

#[cfg(feature = "rust_call_natural_env")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn rust_call_natural_env(
    seed: u32,
    lhs: u32,
    rhs: u32,
    input: &[u32],
    other: &[u32],
    mut shared_first: DisjointSlice<u32>,
    mut shared_last: DisjointSlice<u32>,
    mut first_before: DisjointSlice<u32>,
    mut first_after: DisjointSlice<u32>,
    mut second_before: DisjointSlice<u32>,
    mut second_after: DisjointSlice<u32>,
    mut caller_state: DisjointSlice<u32>,
    mut shared_after_first: DisjointSlice<u32>,
    mut shared_after_last: DisjointSlice<u32>,
) {
    let a = lhs.wrapping_add(thread::thread_idx_x());
    let b = rhs.wrapping_sub(thread::block_idx_x());
    let mut env = Env {
        state: seed,
        unit: (),
        input,
        other,
    };
    let alias = &env;
    let shared = read_env(&env, alias, a, b);
    let first = mutate_env(&mut env, (a, (), b));
    let second = mutate_env(&mut env, (b.wrapping_add(7), (), a ^ 0x1357_9bdf));
    let observed = env.state.wrapping_add(seed ^ 0x2468_ace0);
    let alias = &env;
    let after = read_env(alias, &env, b, a);
    if let Some(value) = shared_first.get_mut(thread::index_1d()) {
        *value = shared.0;
    }
    if let Some(value) = shared_last.get_mut(thread::index_1d()) {
        *value = shared.2;
    }
    if let Some(value) = first_before.get_mut(thread::index_1d()) {
        *value = first.0;
    }
    if let Some(value) = first_after.get_mut(thread::index_1d()) {
        *value = first.2;
    }
    if let Some(value) = second_before.get_mut(thread::index_1d()) {
        *value = second.0;
    }
    if let Some(value) = second_after.get_mut(thread::index_1d()) {
        *value = second.2;
    }
    if let Some(value) = caller_state.get_mut(thread::index_1d()) {
        *value = observed;
    }
    if let Some(value) = shared_after_first.get_mut(thread::index_1d()) {
        *value = after.0;
    }
    if let Some(value) = shared_after_last.get_mut(thread::index_1d()) {
        *value = after.2;
    }
}
