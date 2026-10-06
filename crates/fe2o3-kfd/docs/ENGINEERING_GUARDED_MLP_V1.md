# Engineering Guarded MLP Runtime

The gfx950 engineering API now owns paired reduction/MLP state through opaque
allocation, binding and dispatch objects. Allocation is separate from binding
so the complete model allocation roster can be established before kernel
arguments capture their identities. Binding consumes the unbound object;
failed dispatch or retirement cannot silently reset a pair for reuse.

The combined allocation contains a 548-word MLP prefix and four guard words.
Mixed-bank retirement checks every retained Prefix284 and Combined552 owner
before changing any state, then establishes the final fence before making
the bank ready. The caller's unsafe retirement contract still requires that
no old prefix dispatch can resume. This is an opt-in engineering boundary,
not production execution authority.

## Validation

These 29 cumulative source/test files are byte-identical to the source set
qualified on MI350 in Ferric's
[guarded model-interface checkpoint](https://github.com/harsh-nod/ferric/tree/47b5165af48e34fdbd6b4e31f5ec645261bf7f5a/qualification/guarded-mlp-model-interface-v1).
The qualification records 1,088 ordinary test passes, eight intentional
ignores, nine compile-fail API tests, a default-feature check and clean source
postchecks. Its single native component attempt verifies 557,056 computed
values bit-for-bit through the public allocation/binding/dispatch facade.

That GPU fixture uses the existing private pair rearm. It does not qualify
the new mixed-bank transaction on a real model. The source promotion retains
the repository's original Cargo manifests and lockfile; the test run used
the retained focused workspace, not the entire repository workspace. No
whole-workspace, sustained decode, model-numerical or throughput claim follows.

Ferric's standalone engineering worker builds against a sibling `fe2o3`
checkout using these exact runtime sources. Its parent process has a separate
locked dependency graph and launches the qualified worker as a child.
