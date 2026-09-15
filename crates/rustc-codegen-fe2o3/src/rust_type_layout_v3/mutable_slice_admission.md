# Mutable-Slice Admission

Raw `&mut [T]` now has a distinct source, layout, descriptor, and generated
host-packing contract. This is general scalar-slice architecture; it has no
tutorial-name, argument-position, or engineering-artifact exceptions.

## Implemented Chain

- `fe2o3-artifacts/src/rust_layout.rs` adds
  `RustSourceTypeShapeV1::MutableSlice` with canonical source tag 5. It requires
  a mutable pointer to the exact scalar followed by a pointer-width usize.
  Existing tags, disjoint mappings, and golden identities are unchanged.
- `fe2o3-artifacts/src/abi.rs` documents the existing `UniqueBorrow` contract:
  host exclusivity does not establish disjoint device invocations.
- `fe2o3-kernel-descriptor/src/{model,encode,decode}.rs` adds descriptor tag 5,
  distinct source/layout identities, and `LogicalArgumentV1::mutable_slice`.
  Validation requires `UniqueBorrow`, exclusive read/write access, and the
  exact pointer/length ABI. Older readers reject the unknown tag.
- `rust_type_layout_v3.rs` reconstructs raw mutable-slice evidence from rustc.
  `rust_type_layout_general.rs` already preserves the necessary observations
  and is unchanged.
- `compiler_descriptor.rs` propagates the distinct kind through records,
  logical arguments, semantic source identity, layout/FnAbi checks, KIR
  correspondence, and formal allocation matching. Its semantic ownership check
  requires `UniqueBorrow` for mutable references and still requires
  `ExclusiveOwner` for genuine device wrappers.
- `fe2o3-macros/src/lib.rs` parses raw mutable slices independently of genuine
  disjoint wrappers, derives the new host identity, and generates
  `bind_mutable_argument` for initialized read/write host buffers.
- `fe2o3-host/src/generated_argument_plan.rs` derives the new identity,
  validates exact descriptor/source/element/effect correspondence, rejects
  disjoint index-space substitutions, and binds the retained borrow and extent.
- `fe2o3-host/src/generated_kfd_arguments.rs` adds the explicit mutable-slice
  binding method and shares the existing initialized-buffer/writeback path.
  Existing ordinary and mapped disjoint binding methods retain their contracts.

The ngram gather and reverse-probe output lower to `&mut [i32]` at physical
argument 6; gradient-shard staging lowers to `&mut [f32]` at physical argument 2.
Their context global capability APIs therefore use this path naturally.

## Proof Boundary

Host exclusivity only discharges alias requirements between distinct logical
arguments. Same-argument conflicts between device invocations remain subject to
the existing ranked/global bounds, race, and output-coverage proof gates.
Mutable-slice descriptors never receive a disjoint index-space identity or
enter the genuine-wrapper mapping checks. Engineering artifacts grant no
qualification authority.

No production importer, SSA transport, ranked projection, or proof-gate code is
changed by this work. The remaining integration boundary is the parent's full
compiler/host build and the three tutorial runs on the integrated tree, including
the importer/SSA work owned by the parent.

## Validation

Local bounded test runs:

- `cargo test -p fe2o3-artifacts --test rust_layout --offline`: 14 passed.
- `cargo test -p fe2o3-kernel-descriptor --lib --offline`: 34 passed.
- `cargo test -p fe2o3-macros --lib --offline`: 83 passed.

Added compiler and host tests, pending the parent's build:

- `cargo test -p rustc-codegen-fe2o3 --lib rust_type_layout_v3::mutable_slice_tests`
- `cargo test -p rustc-codegen-fe2o3 --lib compiler_descriptor::tests`
- `cargo test -p fe2o3-host --lib mutable_slice`

The compiler tests observe real Rust mutable references and aliases, distinguish
same-size source types, and reject root/ABI substitutions. Schema tests cover
round trips and malformed ownership/access/extent records. Macro and host tests
check distinct identities, exact binding calls, and mutable/disjoint
substitutions in both directions. Existing disjoint golden tests still pass.
