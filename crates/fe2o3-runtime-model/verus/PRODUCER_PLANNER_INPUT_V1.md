# Producer-to-Planner Input Composition

This is a present-root observation theorem, not a complete `Context` refinement.
`producer_planner_input_composition_v1.rs` executes the existing concrete
`Composition::reconcile`, projects its actual returned status into the selected
completion node, and calls the existing planner `directed_input_status_v1` leaf.
The input answer is not an executable argument of the composed operation.

The contracts retain the submission generation/local identity, exact reached
producer receipts/calls, all non-input node fields, all other nodes, and the
planner's existing check/rejection state. Journal errors become
`InvalidBackendDescription` and set the planner's quarantine projection. The
success-ready gate requires actual producer success; at an exhausted dependency
cursor, the quiescent-ready gate requires actual producer unknown.

The assignment to `CompletionNodeV1::input` materializes an observation in the
finite planner model. It is not a proposed production write or an assertion that
the real runtime stores this observation field. The adapter source checks bind
the real `validate_producer_read_v1`, `directed_input_status_v1`, journal-error
adapter, and shared planner consumer without pretending those full adapters are
executed by this theorem.

## Qualification

The source guard constructs a generated proof root by inserting one include into
the exact retained concrete producer root. It does not rewrite any existing
executable body, contract, precondition, solver limit, or historical proof pin.
The resulting whole root has 51 proof inputs and four additional source-only
adapter bindings. Its unfiltered positive has 260 verified obligations.

Run the checker tests separately from proof execution:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/test-producer-planner-input-composition.py
python3 -I -B crates/fe2o3-runtime-model/verus/check-producer-planner-input-composition.py
```

For a reviewed frozen input manifest, run the maintained campaign:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/qualify-producer-planner-input-v1.py \
  --inputs "$(pwd)/crates/fe2o3-runtime-model/verus/pins/PRODUCER_PLANNER_INPUT_CAMPAIGN_V1.json" \
  --verus /absolute/path/to/pinned/verus \
  --output /absolute/path/to/fresh-owned-output
```

`--capture-inputs /absolute/fresh/path.json` only records candidate input hashes.
It is deliberately separate from proof execution. A changed source or support
file invalidates a frozen manifest. No source pin is refreshed by this runner.
The retained manifest was installed only after the complete local campaign:
two 260/0 positives, eleven 259/1 logical negatives, and both verifier closure
checks passed in 429.562 seconds. Its SHA-256 is
`a59fd44d08081b5330794de94f562859e73d96c192dfaa2e12c54ab49111b1f4`.
The runtime-model workflow runs this command on the existing main-only reviewed
host lane and retains its diagnostics. Runner provisioning remains a separate
operational prerequisite; local qualification is not a CI execution claim.

The campaign verifies the entire pinned verifier closure before and after the
run, executes positive roots before and after all eleven controls, and retains each
exact command, source hashes, raw output, exit code, and process-group cleanup.
Every child uses the existing 120-second/default-SMT/four-thread profile. A
negative is accepted only as a complete whole-root 259/1 logical rejection at
its exact intended postconditions, with explicitly calibrated finite enumeration
and range-recommendation notes. Timeouts, signals, compiler errors, filtered
proofs, other diagnostics, and source/tool drift are failures. At least 16 GiB
available memory is required, and the owned campaign scratch is capped at
128 MiB. Captures and result JSON are not signing authority.

## Limits

- Root selection and absent-root behavior remain premises outside this slice.
- Credit-lock and `Arc` identity refinement are not added.
- The complete mutable `Context`, allocation behavior, and writer-quarantine
  side effects are not refined by the finite planner projection.
- Full DAG traversal and the concrete journal settlement/release sequence are
  not composed here; their existing theorems and limitations remain unchanged.
- No operating-system, queue-publication, device, or GPU-completion claim follows.
- The historical 114-stage producer/live campaign remains separate and must be
  requalified against the final runtime inventory after runtime changes freeze.
