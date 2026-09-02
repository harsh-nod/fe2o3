use std::marker::PhantomData;

/// Crate-private proof that a generated argument remains tied to its original
/// storage borrow after the capability wrapper is consumed.
pub(super) struct GeneratedArgumentBorrowV1<'allocation>(PhantomData<&'allocation ()>);

impl GeneratedArgumentBorrowV1<'_> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

#[cfg(all(test, feature = "qualification-legacy-hip-hsa"))]
pub(super) const fn generated_argument_borrow_for_test() -> GeneratedArgumentBorrowV1<'static> {
    GeneratedArgumentBorrowV1::new()
}
