// Trusted std contents/cardinality contracts. These do not constrain allocation,
// addresses, allocator failure/panic behavior, cost or termination.
use std::alloc::Allocator;

verus! {
pub assume_specification<T, A: Allocator>[Vec::<T, A>::into_boxed_slice](vec: Vec<T, A>)
    -> (out: Box<[T], A>)
    ensures out@ == vec@,
;
pub assume_specification<T, const N: usize>[<Box<[T; N]> as TryFrom<Box<[T]>>>::try_from](slice: Box<[T]>)
    -> (out: Result<Box<[T; N]>, <Box<[T; N]> as TryFrom<Box<[T]>>>::Error>)
    ensures match out {
        Ok(array) => slice@.len() == N && array@ == slice@,
        Err(returned) => slice@.len() != N && returned == slice,
    },
;
}
