# Defined Math Origins

Status: proposal for parent/Pauli agreement. No API, schema, collector parent,
lowerer parent or backend hooks changed. Only this directory was added during
the source-origin task. `body.rs` contains closed structural predicates;
`probe.rs` exercises them against original cached rustc MIR. Structural matches
do not authenticate a provider or establish SSA custody.

## Actual Rustc Observations

The standalone probe was compiled directly with nightly-2026-04-03/rustc-dev.
It queried existing host and gfx942 AMD metadata with no Cargo, dependency
build, code generation, SSH or network. Both runs exited successfully.

Raw observations: `/tmp/fe2o3-policy-defined-body-host.log` and
`/tmp/fe2o3-policy-defined-body-amdgpu.log`. Metadata SHA256:

```text
host libfe2o3_device-94f7230dae5d07a7.rmeta
3e3063cd7c879b2e1f4a161ee087a896f1079f781fb63777cb93856873b26470
AMD libfe2o3_device-c1806ab98746e656.rmeta
4ead8ae1a03ee23909ca305b1c737bd63c96f618efb2999132c13f4a4680b4e6
```

Both targets expose these Runtime(Optimized) original bodies:

```text
KernelContext::math: 1 argument, 2 locals, 2 blocks
  _1: &KernelContext<K, T, L>
  _0: DeviceMath<KernelCapabilityBrand<K, T, L>>
  bb0: _0 = DeviceMath::<Brand>::current_branded() -> bb1
  bb1: return

DeviceMath::current_branded: 0 arguments, 2 locals, 2 blocks
  _0: DeviceMath<Brand>
  _1: DeviceMath<UnbrandedCapability>
  bb0: _1 = DeviceMath::current() -> bb1
  bb1: return

DeviceMath::with_numerical_policy: 2 arguments, 3 locals, 1 block
  _1: &DeviceMath<Brand>
  _2: &NumericalPolicyCapability<Brand, StrictIeee>
  bb0: _0 = PolicyDeviceMath {
      math: copy _1, _policy: copy _2,
      _not_send_sync: const PhantomData::<*mut ()>
  }; return
```

The bridge's ZST aggregate assignment has been optimized away. Recognizing a
retained aggregate in that function would reject the real body. The getter's
receiver still exists despite being unused in its Rust implementation.
Bind, in contrast, really retains both reference operands in an aggregate.
Rustc prints the public `Math` alias in diagnostics; aliases are not identities.

Observed ABI on both targets: getter `[Direct] -> Ignore`, bridge `[] -> Ignore`,
Bind `[Direct, Direct] -> Pair`; `can_unwind=false`. References carry NonNull and
NoUndef, zero pointee bytes, no pointee alignment. Do not demand a direct owned
Math return or direct wrapper return. Host MIR call unwind is Continue; AMD is
Unreachable. Retain the original record and existing importer unwind handling,
not a new normalization justified by these observations.

`body.rs` accepted getter, bridge and Bind on both targets. Seven mutations
rejected on each: wrong receiver local type, wrong getter destination, missing
getter call, substituted bridge callee, swapped Bind references, missing Bind
aggregate, and an extra Bind statement. The current leaf's original MIR is
absent from this host metadata and present as a panic stub on AMD; source
classification must authenticate the terminal, not require its panic body.

## Provider Authentication

No new public diagnostic attribute is needed for the proposed implementation.
Reuse the exact-reviewed external helper mechanism already used for epoch():

- Getter: `is_exact_reviewed_provider_definition_v1` for structural provider
  path `fe2o3_device::context::KernelContext::math`, AND
  `authenticate_reviewed_safe_external_helper_v1`. These checks bind reviewed
  source closure, external provider membership and structural definition.
- Bridge: apply the same checks to
  `fe2o3_device::math::DeviceMath::current_branded`. It has no independent
  issuance role. Require its exact instantiated callee to classify as the
  existing DeviceMath Current terminal.
- Bind: require `TrustedDeviceItem::PolicyMathBind`, retain its defined body,
  call the existing `validate_policy_bind_source_v1`, then the complete body
  predicate. The existing marker is `fe2o3_device_policy_math_bind_v1`.
- Validate exact Item instances, generic arity, no const substitutions, safe
  monomorphic Rust signatures and source ABI. Normalize types using existing
  `TypingEnv::fully_monomorphized` helpers; do not compare pretty-printed paths.
- Getter receiver must be the authenticated KernelContext ADT. Its K/T/L axes
  must equal the returned Math's KernelCapabilityBrand axes and selected root.
  The bridge must instantiate precisely that Brand; Current is unbranded.
- Bind must retain precisely the reviewed Math, policy capability and wrapper
  ADTs, identical kernel brand, and exact StrictIeee policy. Keep the existing
  source nominal checks, including sealed marker and wrapper field types.

The paths above are inputs to the reviewed structural provider API, not string
recognition of arbitrary rustc definitions. If the parent prefers a new public
getter diagnostic item, it must coordinate the device attribute, registry and
reviewed source-closure update; that churn is not needed by the existing helper
authentication mechanism. No provider-hash field should be accepted as caller-
supplied authority in an inert MIR contract.

## Closed Canonical Payloads

Extend Pauli's `SemanticDefinedCapabilityContractV1`, preserving its attachment
API and common `function/source_identity/body_identity/provenance` methods.
Reserve the new discriminants centrally; do not modify epoch0 or KIR payloads.

Proposed typed cases, not opaque labels:

```text
KernelMathDerive(SemanticKernelMathDeriveV1)
  common: function, source_identity, body_identity, provenance
  types: context_reference, context, math
  kernel_brand: SemanticTypeIdentityV1
  bridge: exact defined-body dependency
    function, source_identity, body_identity
    current_callable, current_source_identity, unbranded_math

PolicyMathBind(SemanticPolicyMathBindV1)
  common: function, source_identity, body_identity, provenance
  types: math_reference, math, policy_reference, capability, bound
  policy, kernel_brand: SemanticTypeIdentityV1
```

Use the same bounded unannotated canonical-function digest machinery as epoch,
clearing the generalized metadata field to avoid self-referential hashes. For
Math, check both the getter and bridge commitments. Source IDs derive from the
actual rustc instances, not method names; canonical function IDs resolve to
exact retained Defined roster entries. Current resolves to its exact intrinsic
binding and the existing MathContextCurrent operation on the unbranded type.
The source Rust MIR hash is an observation, not a replacement canonical digest.

The closed admission validators mirror the observed full bodies, including
local roles/types, fixed normal-return edges, call argument/destination arity,
ordered references, actual result aggregate definition and marker constant.
Reject extra blocks, statements, stores, drops, aliases, raw/mutable/fake
borrows, missing calls, non-item bodies, substitutions and stale commitments.
Charge bounded body serialization/hash work and dependency lookup work.

Do not put the consumer's `element`, FP function, or `bound_reference` on a
getter/constructor that does not mention them. Join those from MIR19's existing
seven-edge consumer contract by exact identity only after source-origin custody
is known. The derive recipe is policy-independent; Bind supplies the policy.

## Reusable Checked Occurrences

Generalize the existing replay-checked epoch occurrence mapping, not a second
Math-only collection of parent fields. A private-constructor
`SemanticExpandedDefinedCapabilityV1` can carry:

```text
contract, expansion_identity, expanded_root_identity, root
caller_instance, callee_instance, caller_function
original_call_block, expanded_call_block
argument_transfers[]:
  source_argument, original_operand, expanded_operand, callee_local
  source_type, expanded_parameter_transfer_location
return_transfers[]:
  source_return_local, expanded_return_local, actual_destination_place
  original_return_block, expanded_return_transfer_location
recipe_locations[]: original source location + expanded location
```

Derive it only after `verify_replay(source)`. Check every argument transfer
against the original call and actual `ParameterTransfer` statement; preserve
Copy versus Move and projected places. Check every result against ReturnTransfer
and the actual caller destination. Retain FrameStorageLive/Dead attribution.
Account for nested instances of the reviewed Math bridge: its Current terminal
is subordinate to one exact getter instance, never a freestanding Math issuer.

Use one per-root indexed scan and the existing work/storage limits, not a scan
of all blocks for each consumer. Keep source type/layout/function tables owned
by the original admitted module. Feed the checked occurrence plan into the
lowerer before it discards original bodies and keeps only an expansion digest.

## Typed Origin And Borrow Plan

1. Key definitions by expanded root, instance, original block, statement or
   terminator, destination projection and storage generation. Locals/types alone
   are not origins. Associate genuine issuers with their existing typed KIR SSA
   values; no reference, tuple, signature or metadata record creates an owner.
2. Track a shared reference as `(owner origin, referent generation, typed
   projection path, nominal reference type, borrow occurrence)`. Copying a
   shared reference copies this alias; moving one kills its old local binding,
   not the underlying owner. Shared reborrows preserve that same owner.
3. ParameterTransfer retains the exact argument's reference and Copy/Move
   semantics. ReturnTransfer retains the returned wrapper/reference dependencies.
   FrameStorageDead kills a callee alias, not a caller-owned referent. Returning
   a loan of a callee-owned value that dies at frame exit rejects.
4. An owner move, overwrite, Deinit, Drop, StorageDead or aliasing mutable/raw/
   fake borrow invalidates dependent loans. Constructor movement transfers the
   pair of loans and kills the old wrapper binding; it does not move the Math
   or policy referents. Subsequent consumer use requires both referents live.
5. At CFG joins retain an origin only if every incoming live fact agrees on
   owner, generation, projection and nominal types. Distinct origins reject,
   even if types match. Reuse the same dominating KIR ValueId for an identical
   origin, not a block parameter that pretends to reissue it. Bound the dataflow
   worklist and reject unknown/conflicting loop-carried custody.
6. At the checked getter's normal return, emit MathDerive with the actual
   KernelContext SSA owner captured from its original receiver. At Bind's exact
   aggregate statement, emit Bind with the two resolved referent SSA values.
   Both must trace to the SAME KernelContextIssue value and matching provenance.
   Emit the F32 consumer only from that Bind and its live retained loans.
7. Frozen KIR MathSource roles contain a full binding (including consumer type
   edges). The planner must reconcile these from real reachable Bind/consumer
   uses before emission. Repeated functions with the same binding reuse the
   source; conflicting bindings reject in this bounded first implementation.
   Do not turn a type-only registration into an issuer or invent extra issuance.

The plan applies to epoch projection as a SharedRef with its fixed field path,
so epoch and Math share move/lifetime/generation handling without sharing their
closed operation semantics. Existing KIR producer/dominance verification and
all lifetime, numerical-policy and target obligations remain required.

## Source Coordinates And Integration Boundary

ExecutionCapabilitySourceV1 still has only function/operation hashes and block.
Do not remove the global expanded-execution rejection. Agree one common source-
occurrence commitment derived internally from the replay-checked record and
retained by correspondence: expansion/root identities, instance, original
location, typed recipe, and original function/body identity. This commitment is
an identity, not a new proof. Recompute it during replay; do not expose a builder
accepting a caller's digest or disguise missing coordinates as a callee hash.

After agreement: add source authentication wrappers in this scoped child,
Pauli adds the closed typed payloads/validators, parent attaches the shared
defined record, and the lowerer consumes one checked occurrence/origin plan.
Then wire actual derive/Bind/consumer emission and run central positive source
and negative custody tests before removing the temporary source rejection.
The scoped body predicates already have actual-rustc evidence; the full
authenticated source and lowering chain is not claimed complete.
