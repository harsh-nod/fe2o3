# Enrollment Restoration And Prerequisite Qualification

This packet records two CPU-only qualifications:

| Scope | Signed source | Result |
| --- | --- | --- |
| Linear output restoration | `2202036a93c2c340536b80f3d2e907b201833789` | 814 unit tests passed, 2 ignored; 27 doctests passed; formatting and Clippy passed |
| Refreshed prerequisite checker | `3278dbb91b77d0417b32241de0c5e41190f44ae4` | Two whole-crate 169/0 positives and seven intended 168/1 controls |

The production change replaces enrollment's final key sort with a linear refill
from canonical entries and the unchanged reversed free suffix. Rejection checks
and the allocation-write/truncate loop are unchanged. The new test covers all
120 permutations of five free slots at all six batch sizes: 720 successful cases,
with an existing allocation and distinct incoming device/extent metadata. It
checks exact output, complete journal contents and storage identity. Existing
rejection tests also pass. Pointer stability is not an allocation-counter proof.

The two ignored tests are existing benchmark-style memory-journal scale checks.
They were not run. The changed restoration step is O(k); the complete enrollment
operation retains its existing asymptotic bound. No measured speedup is claimed.

The standalone checker update accounts for one additional inherited reader-prefix
lemma: 156 inherited obligations plus 13 enrollment-prerequisite obligations.
It still verifies only the admission prefix and conditional commit suffix, not
the intervening replay/search/slot-validation/restoration phase. This is not a
formal equivalence proof for the optimization or full enrollment verification.

`cpu/` and `prerequisites/` retain the original machine-specific drivers, terminal
qualification results and unmodified command streams. All 17 outer commands and
11 nested solver/closure invocations record absent owned process groups. Both
qualifications bracket clean signed source; the prerequisite campaign's 27 input
identities match before and after execution. The only change between the two
source commits is the prerequisite checker's expected counts.

`import.py` is the receipt-only importer, not a complete offline proof validator.
Historical paths in these receipts need not remain present. `SHA256SUMS` covers
all archive files except itself. Run `sha256sum --check SHA256SUMS` here to check
retained bytes; authenticity depends on the signed Git publication.

From a checkout containing the signed changes, rerun the narrow checks with:

```sh
cargo test --offline -p fe2o3-runtime-model -- --test-threads=2
cargo fmt -p fe2o3-runtime-model -- --check
cargo clippy --offline -p fe2o3-runtime-model --all-targets -- -D warnings
python3 -I -B crates/fe2o3-runtime-model/verus/check-journal-enrollment.py \
  --verus "$VERUS" --timeout 180 --output "$NEW_EMPTY_OUTPUT_DIRECTORY"
```

Use the pinned Verus release described by the runtime-model proof runner. The
drivers are provenance, not portable replay scripts. No native backend, GPU,
MI300X process, HIP/HSA comparison or runtime parity claim is part of this packet.
See the [enrollment scope](../../runtime-context-version-enrollment-v1.md).
