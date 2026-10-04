# Owned Conditional Fill Refinement

Parent: `ea555e726e04a9508b48c9d7a2bf02b08bf50467`.
This closes the owned-refinement prerequisite for multi-GPU application admission,
not the pending host artifact, native launch or protected compiler-deployment join.

## Implementation

`execute_owned_conditional_fill_refinement_v1` consumes the original validated
conditional compiler inputs, target lineage and authenticated machine-analysis
execution. Temporary checked views run the existing generated proof once; only
its owned source, obligation and strictly imported receipt are moved out of the
borrowed result. The original three owners then move into the lifetime-free result.
There is no self-reference, receipt reconstruction, public from-parts constructor,
mutable owner access or conversion to executable authority. Failure consumes the
inputs as documented. The borrowed producer remains available and unchanged.

The result can outlive the proof-runtime lease. Future artifact admission can
retain this original evidence without running Verus at each launch. Boundary 5
and all conditional storage, geometry, schedule and completion premises remain
unchanged. Immutable compiler-input access does not authenticate compiler origin.

## Qualification

The first protected native positive passed in 31.70 seconds. The final campaign
passed both tests in 42.77 seconds using the same real Worker, pinned protected
runtime and genuine finalized fill payload as the
[parent checkpoint](../dev-fill-refinement-2026-10-03/README.md).

The positive compares six original heap-buffer addresses before and after moves:
semantic bytes, target KIR, analyzer payload, request, bundle and receipt. It checks
the original analyzer challenge/identity, identical generated source and obligation,
strict receipt import and false launch authority after the construction scope and
runtime lease have ended. Reconstructing temporary views from the retained owners
also reproduces the exact obligation without rerunning Verus.

The negative changes two genuine kernel-descriptor fields while retaining the
instruction stream, obtains a fresh analysis from the real Worker, and requires
`Machine(Kernel(EntryLayout))`. A zero proof timeout distinguishes this rejection
from proof execution: the profile check must fail before runtime timeout checking.

Ownership doctests cover a lifetime-free result, clone rejection and use-after-move
rejection. A second distinct genuine compiler handoff was not available; compiler
cross-owner substitution through the new consuming API remains a test gap. Existing
program association and source mutation tests are retained. No GPU was used.

Final regression results: 163 verifier tests passed with 10 default ignores;
211 host tests passed with one default ignore; functional-proof legacy/V2 suites
passed 10 and 8 tests plus two doctests. Conditional-fill ownership doctests passed
one positive and five compile-fail tests. Strict verifier-library Clippy, targeted
rustfmt and whitespace checks passed. Default ignores outside the explicit
protected campaign are not newly claimed as tested.

## Next Admission Work

Construct the distinct pending host artifact under retained publication currentness,
bind one fresh inherited compiler-service audit to its exact subject/carriage, and
retain the original owners. The inherited compiler client is one-use; two GPU
invocations must not independently consume the same connection. Preserve original
artifact evidence across invocation-specific checks. Do not use the deterministic
host request identity as the fresh current-record challenge.

The existing signed audit does not establish protected signing-key custody or
independently administered anchor deployment. Keep that production trust join
separate from the actual KFD pointer/backing, full64 coverage, selected-device and
completion discharges. No additional routing or opcode abstraction is needed first.

## Reproduction

The qualification archive contains the code-only patch, protected execution scripts,
exact proof/analyzer captures and CPU qualification logs. Worker build sources and
runtime prerequisites are in the parent checkpoint's archive. The `.key` capture
is the public receipt-verification key, not signing material.

The [qualification archive](qualification.tar.gz) contains 24 entries with SHA-256
`755beb483e1c70c56487ee14fa7f29dc7ebab1e902a4788f295936a3be4f179c`.
Its extracted code-only patch SHA-256 is
`ad17a56f1fc77b2571007b9da4f8990c5ce82243943d5de54b9189d7001382eb`,
matching the qualified working tree.
