# Physical machine call-site accounting (V1)

The gfx942 physical machine analyzer preserves every decoded static call site in the instruction trace. A site is identified by function, instruction offset, and exact resolved target. Two instructions calling the same helper are two sites.

The function evidence's direct_callees field remains sorted unique adjacency. It is used for reachability and cycle rejection; it does not count calls. Its existing V1 bytes/domain/schema are unchanged. The trace's existing DirectCall rows preserve multiplicity without a second executable graph.

## Bounds and semantics

- At most 64 reachable functions, no direct-call cycles, therefore at most 64 functions in a simple call chain.
- At most 256 static call sites across all analyzed functions, before graph deduplication. The same cap applies independently in the Rust trace consumer.
- Each entry's unchanged max_direct_calls budget counts actual decoded call instructions across its unique reachable function closure. Shared helpers contribute their own static sites once; repeated invocations do not multiply their memory/return effect rows.
- These are static site bounds, not bounds on loop iterations, recursion at runtime, dynamic calls, stack consumption, or functional execution.
- Existing payload, instruction, block, operand, function, effect, and encoded-byte limits remain unchanged.

The native analyzer still resolves every individual call through its exact LLVM/MC/dataflow rules. The supported direct call spellings are S_CALL_B64_vi and S_SWAPPC_B64_vi; physical return spellings are different. The Rust consumer checks call opcode/tag agreement, complete exact-payload instruction coverage, unique offsets, exact target function addresses, and equality between the actual target set and unique adjacency. It does not independently reimplement LLVM instruction semantics. Authenticating analyzer execution remains a separate requirement.

## Deliberate standalone decoder tightening

PhysicalMachineEffectEvidenceV1::decode_canonical_for now returns TraceRequiredForDirectCalls for any otherwise valid graph containing calls. Effect-only V1 bytes cannot express call-site multiplicity, so they cannot alone validate this budget. A malformed graph/identity/range may be refused earlier.

Call-bearing users must use PhysicalMachineAnalysisEvidenceV1::decode_canonical_for, which validates both bound components before exposing either. Its graph/effect decoding seam is crate-private, used only by that bundle decoder; there is no public unchecked effect-only path. Existing zero-call decoding and old V1 wire bytes remain unchanged.

The authenticated worker path already consumes the combined bundle. No source custody, compiler-refinement, publication, load, GPU execution, or launch permission is introduced. A decoded bundle still does not prove the truth of native extractor claims without the existing authenticated execution chain.

## Qualification

New inert Rust controls exercise one/two calls to the same callee, a 1-versus-2 budget, standalone refusal, zero-call preservation, both supported call spellings, shared nested helper sites, 256/257 sites (including distribution over functions), cycles, 64/65 functions, hidden/missing/duplicated call rows, foreign targets, invalid ranges, adjacency mutations, and request identity substitution.

New native CPU/static controls compile and decode one/two/three actual calls, compare exact call offsets/targets, retain one static helper write/return, reject one-short budgets, and mutate one actual call materialization. All previous native controls remain.

This change must be rebuilt and qualified with the pinned SDK/ordinary worker and the retained actual composition matrix. Previous two-call/O0 refusal and incomplete matrix evidence remain historical; authoring these controls is not a successful run.
