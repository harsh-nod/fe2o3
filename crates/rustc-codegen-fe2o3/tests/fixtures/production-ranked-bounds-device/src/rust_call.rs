use fe2o3_device::{DisjointSlice, kernel, thread};

struct Token(u32, (), u32);
struct SliceToken<'a>(&'a [u32], ((), ()));

#[inline(never)]
fn captured_len(token: SliceToken<'_>) -> u32 {
    #[cfg(not(feature = "rust_call_slice_read"))]
    {
        token.0.len() as u32
    }
    #[cfg(feature = "rust_call_slice_read")]
    {
        if token.0.len() == 0 { 0 } else { token.0[0] }
    }
}

#[inline(never)]
fn parts(seed: u32, a: u32, b: u32, tail: u32) -> (u32, (), u32) {
    ((seed ^ a).wrapping_add(tail), (), b)
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn rust_call(
    seed: u32,
    lhs: u32,
    rhs: u32,
    mut pair: DisjointSlice<u32>,
    mut empty: DisjointSlice<u32>,
    mut zst: DisjointSlice<u32>,
    input: &[u32],
    mut length: DisjointSlice<u32>,
) {
    // Moving the whole non-Copy capture requires an owned FnOnce receiver.
    let token = Token(seed, (), lhs);
    let f = move |a: u32, b: u32| {
        let moved = token;
        parts(moved.0, a, b, moved.2)
    };
    let p = f(lhs, rhs);
    let p = p.0.wrapping_sub(p.2);
    let token = Token(seed, (), lhs);
    let f = move || {
        let moved = token;
        moved.0.wrapping_sub(moved.2)
    };
    let e = f();
    let token = Token(seed, (), lhs);
    let f = move |a: u32, (): (), b: u32| {
        let moved = token;
        a.wrapping_sub(moved.0 ^ b).wrapping_add(moved.2)
    };
    let z = f(lhs, (), rhs);
    let token = SliceToken(input, ((), ()));
    let f = move || {
        let moved = token;
        captured_len(moved)
    };
    let n = f();
    #[cfg(feature = "rust_call_slice_index")]
    let n = {
        let token = SliceToken(input, ((), ()));
        if (lhs as usize) < token.0.len() {
            n ^ token.0[lhs as usize]
        } else {
            n
        }
    };
    if let Some(value) = pair.get_mut(thread::index_1d()) {
        *value = p;
    }
    if let Some(value) = empty.get_mut(thread::index_1d()) {
        *value = e;
    }
    if let Some(value) = zst.get_mut(thread::index_1d()) {
        *value = z;
    }
    if let Some(value) = length.get_mut(thread::index_1d()) {
        *value = n;
    }
}
