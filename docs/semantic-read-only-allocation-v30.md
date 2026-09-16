# Consuming Read-Only Allocations in Semantic MIR V30

V30 adds three compiler-terminal records for the reviewed
`fe2o3_device::ReadOnlyAllocation<T>` API. It adds no kernel argument kind, host
descriptor, allocation identity, or runtime authority. The original
`DisjointSlice<T, Index1D>` argument retains its exclusive lease and read/write
physical ABI.

## Source Contract

`DisjointSlice<T, Index1D>::into_read_only(self)` consumes the source value and
returns a private local view. Initially `T` is exactly `u16` or `f32`. The view
has no public constructor, `Copy`, `Clone`, mutable accessor, or pointer escape.
Its `len(&self)` retains the actual allocation extent. Its
`load_or(&self, index, fallback)` reads only when `index < len`; otherwise it
returns the fallback without a memory effect.

A move in one invocation does not prove that another invocation cannot write
the allocation. Production admission therefore additionally requires the
ranked compiler's exact-root, whole-kernel no-write/no-escape audit, including
uses before the conversion. The source API and its structural layout alone
never discharge that obligation. This capability does not establish shared
tensor publication or an inter-workgroup happens-before relation.

Optimized Rust MIR can spell the genuine consuming call's original argument
as `Copy` after Rust has checked source moves. The checked lowering accepts
that spelling only for an unprojected original argument with the exact
authenticated exclusive read/write slice ABI, and consumes its local binding
at this constructor. The retained request keeps the original operand for
replay. This does not authorize copying a capability local, borrowing a
lookalike aggregate, or reusing a consumed root.

## Retained Records

| Tag | Operation | Retained Types | Source Arity |
| --- | --- | --- | --- |
| 69 | `DisjointSliceIntoReadOnly` | `slice`, `view`, `element` | 1 |
| 70 | `ReadOnlyAllocationLen` | `view` | 1 |
| 71 | `ReadOnlyAllocationLoadOr` | `view`, `element` | 3 |

The constructor returns the view directly, not a `Result`. The two borrowed
operations require an immutable reference to that exact view type. The view's
reviewed `repr(C)` layout is a thin immutable raw pointer and a `usize`, at
offsets 0 and 8, size 16 and alignment 8 on the admitted 64-bit target. Both
scalar fields and their retained layout are checked. The constructor's source
slice also participates in the existing request-wide `Index1D` mapping check.

The compiler importer authenticates the actual diagnostic-item `DefId`, exact
generic element/index-space arguments, canonical definition path, and complete
reviewed device-provider source closure. A matching name, signature, or memory
layout does not authenticate a source producer. The serialized semantic model
itself remains inert and caller-asserted, including these terminal identities.

## Compatibility and Validation

V30 uses existing type/layout records; there is no new nominal type marker.
Intrinsic tags 0 through 68 retain their prior encodings, including the V15
grammar used by V28 and V29. Requests without the new terminals retain their
previous minimum version. V29 and earlier reject tags 69 through 71; exact
version decoders reject a mismatched envelope. V16 through V26 remain retired
and V27 remains independently reserved.

Focused tests cover exact terminal records, truncation and unknown tags,
legacy encoding preservation, exact-version rejection, consuming and borrowed
signatures, pointer mutability/address space, scalar identity, field order,
size, alignment, and stale or lookalike provider identities. Actual source
tests and downstream proof/lowering checks are separate from these inert
schema checks. No GPU execution or protected runtime qualification follows
from admitting or decoding V30 bytes.

## Verification Snapshot

The 2026-09-16 source checks compile genuine `u16` and `f32` consuming views
through the pinned Rust frontend to checked AMDGPU LLVM. All 256 invocations
read input cell zero, while each invocation performs a System-scope Release
store and Acquire load on its own atomic channel. These are compiler checks,
not GPU executions or protected runtime admission.

| Check | Result |
| --- | --- |
| Semantic model, all targets | 458 passed; 1 existing ignored test |
| Compiler library | 696 passed, including 14 consuming-view tests |
| Genuine many-reader source | Both element types passed |
| New source rejection cases | 3 tests passed, covering 6 compilations |
| Existing indexed-atomic source regressions | 3 passed |
| Device consuming-view host tests | 2 passed |
| Unsafe inventory policy | 5 passed; 1 maintenance-only test ignored |

The source negatives retain Rust use-after-move rejection, the sealed element
boundary, nominal and index-space rejection, and rejection of writes before
conversion or on another control-flow path. The existing ordinary shared
input plus shared atomic channel still fails alias analysis; the exclusive
input and two-argument atomic controls retain their exact System ordering.

Source tests use `production_extraction_driver_v1` with the
`consumed_read_only_` and `indexed_atomic_slice_` filters. Test executables
are built with `--no-run` first, then the CLI and matching backend/extractor
are rebuilt together before invoking the test executable directly. No Cargo
rebuild of that compiler target intervenes; source and binary hashes are
checked before and after. This avoids confusing an ABI-mismatched Rust
compiler pair with a source-admission failure.

Workspace and new include-file formatting checks pass. Compiler Clippy still
reports its 38 pre-existing diagnostic locations; normalized messages and
source paths match the prior baseline, with no new diagnostic source lines.
Neither these checks nor total-read lowering prove tensor publication,
cross-workgroup synchronization, or native floating-point numerical behavior.
