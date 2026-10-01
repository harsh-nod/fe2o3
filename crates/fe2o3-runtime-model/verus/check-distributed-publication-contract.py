#!/usr/bin/env python3
"""Model-only raw-state classifier refinement, not wire or peer authority.

Digest and derived-equality bridges are source-calibrated representations of
eleven full 32-byte payloads and four u64 coordinates. Constructor validity,
codec canonicality, external identity authenticity and native execution are not
theorems of this campaign. Counts and selector diagnostics start unmeasured.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path('crates/fe2o3-runtime-model/verus')
SRC = Path('crates/fe2o3-runtime-model/src')
OWNER = SRC / 'distributed_publication_contract.rs'
IDENTITY = SRC / 'identity.rs'
DECLARATIONS = SRC / 'distributed_publication_contract/declarations.rs'
BODY = SRC / 'distributed_publication_contract/classifier_body.rs'
PROOF = V / 'distributed_publication_contract_v1.rs'
FILES = [PROOF, DECLARATIONS, BODY]
BASE = V / 'check-compute-pipeline-publication.py'
BASE_SHA = '1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e'
SOURCE_FILES = 305
SOURCE_TREE_SHA = '6813bf95c19da1d3c08b6047bb8095d1709fb9fbd4bd7adc3533b1c625d82adb'
PROOF_SHA = '57edf7747e4c86c6e73b0311e59e06b7fb2dbf276fba5988e526dfd4fcacd853'
IDENTITY_SHA = '4df48a80f5bff5ff82481358ee320819de5afc38eee8e7520c50732326fc3564'
# Measured by the unfiltered development discovery, including 19 Clone derives.
EXPECTED_VERIFIED = 23
SELECTION_NOTES = {
    '*classify_receipt': frozenset({'verifying root module (selected functions)'}),
    '*ModelDistributedPublicationRecordV1::record_untrusted_receipt': frozenset({'verifying root module (selected functions)'}),
    '*ModelDistributedPublicationRecordV1::observe': frozenset({'verifying root module (selected functions)'}),
}
MUTANT_COUNT = 16
SELECTORS = {'classify': '*classify_receipt',
             'record': '*ModelDistributedPublicationRecordV1::record_untrusted_receipt',
             'observe': '*ModelDistributedPublicationRecordV1::observe'}
COORDINATES = ('runtime_instance', 'participant', 'participant_incarnation', 'coordinator',
               'coordinator_epoch', 'membership', 'membership_epoch', 'run', 'operation',
               'attempt', 'artifact', 'execution_plan', 'placement_plan', 'target', 'runtime_model')
DIGEST_TYPES = ('DistributedRuntimeInstanceIdV1', 'DistributedParticipantIdV1',
                'DistributedMembershipIdV1', 'DistributedRunIdV1', 'DistributedOperationIdV1',
                'DistributedExecutionPlanIdV1', 'DistributedPlacementPlanIdV1',
                'DistributedTargetDescriptionIdV1')

def need(value, message):
    if not value:
        raise ValueError(message)

def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()

def tree_hash(sources):
    leaves = {str(p): sha(text) for p, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(',', ':')).encode()).hexdigest()

def snapshot():
    names = {PROOF} | {p.relative_to(ROOT) for p in (ROOT / SRC).rglob('*.rs')}
    result = {}
    for p in names:
        selected = ROOT / p
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected, 'ordinary exact source file')
        result[p] = selected.read_bytes().decode()
    return result

def audit(sources):
    implementation = {p: text for p, text in sources.items() if p.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF}, 'complete model Rust roster plus exact proof')
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA, 'reviewed whole model source bytes')
    need(sha(sources[PROOF]) == PROOF_SHA and sha(sources[IDENTITY]) == IDENTITY_SHA, 'exact proof and external identity declaration bridge')
    owner, declarations, proof = (sources[p] for p in (OWNER, DECLARATIONS, PROOF))
    need(owner.count('include!("distributed_publication_contract/declarations.rs");') == 1
         and owner.count('mod classifier_body;') == 1, 'actual model shares complete declarations and bodies')
    need(declarations.count('distributed_contract_declarations_v1! {') == 1
         and not re.search(r'\binclude!\(', declarations + sources[BODY]), 'closed shared declaration/body inputs')
    need(proof.count('include!("../src/distributed_publication_contract/declarations.rs");') == 1
         and proof.count('include!("../src/distributed_publication_contract/classifier_body.rs");') == 1
         and len(re.findall(r'\binclude!\(', proof)) == 2, 'exact three-file executable proof closure')
    for macro in ('distributed_receipt_classifier_body_v1!', 'distributed_receipt_record_body_v1!', 'distributed_observation_body_v1!'):
        need(owner.count(macro) == proof.count(macro) == 1, 'same actual classifier/commit/observation body')
    fields = re.search(r'pub struct UntrustedDistributedOperationCoordinatesV1 \{(.*?)\n\}', declarations, re.S)
    need(fields is not None and tuple(re.findall(r'pub ([a-z_]+):', fields[1])) == COORDINATES, 'all fifteen named binding coordinates, in exact order')
    need(all(proof.count('left.' + field + (' == ' if field in ('participant_incarnation', 'coordinator_epoch', 'membership_epoch', 'attempt') else '.0.0 == ')) == 1
             for field in COORDINATES), 'explicit equality theorem uses every full payload and integer')
    compact_identity = re.sub(r'\s+', '', sources[IDENTITY])
    need('IDENTITY_DIGEST_BYTES_V1:usize=32;' in compact_identity
         and 'pubstructIdentityDigestV1([u8;IDENTITY_DIGEST_BYTES_V1]);' in compact_identity,
         'real identity representation is exactly thirty-two bytes')
    need('#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]#[repr(transparent)]pubstruct$name(IdentityDigestV1);' in compact_identity,
         'external typed digest wrappers use structural derived equality')
    compact_owner = re.sub(r'\s+', '', owner)
    need('#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]pubstruct$name(IdentityDigestV1);' in compact_owner
         and all(compact_owner.count(name + ',') == 1 for name in DIGEST_TYPES), 'all eight model wrappers preserve exact digest payloads')
    record = re.search(r'#\[derive\(([^\n]+)\)\]\npub struct ModelDistributedPublicationRecordV1', declarations)
    need(record is not None and record[1] == 'Debug, Eq, PartialEq', 'record stays non-Copy and non-Clone')
    need('binding_matches' not in re.sub(r'//[^\n]*', '', proof)
         and not re.search(r'\b(?:assume|admit|assume_specification|requires)\b|verifier::external|\buninterp\b', proof),
         'raw-state proof has no authority Boolean, invented premises or trusted executable body')
    need('#[allow(' not in proof and '#![allow(' not in proof, 'no proof diagnostic suppressions')

def change(body, macro, before, after):
    anchor = 'macro_rules! ' + macro + ' {'
    need(body.count(anchor) == 1 and before != after, 'unique actual macro and meaningful mutation')
    start = body.index(anchor)
    end = body.find('\nmacro_rules! ', start + len(anchor))
    end = len(body) if end < 0 else end
    selected = body[start:end]
    need(selected.count(before) == 1, 'unique executable mutation anchor')
    return body[:start] + selected.replace(before, after) + body[end:]

def mutations(body):
    prefix = 'distributed_receipt_classifier_body_v1'
    terminal = '''if previous.outcome != P::Published
                    || !matches!(
                        $receipt.description.outcome,
                        P::Completed | P::FailedMayStillExecute
                    )'''
    rows = [
        ('binding-checks-only-participant', 'classify', 'if $record.binding != $receipt.binding',
         'if $record.binding.coordinates.participant != $receipt.binding.coordinates.participant'),
        ('binding-mismatch-recorded', 'classify', 'return Err(E::BindingMismatch);', 'return Ok(D::Recorded);'),
        ('conflicting-duplicate-idempotent', 'classify', 'Err(E::ConflictingDuplicate)', 'Ok(D::ExactDuplicate)'),
        ('stale-receipt-recorded', 'classify', 'return Err(E::StaleReceipt);', 'return Ok(D::Recorded);'),
        ('interruption-ignored', 'classify', 'if $record.interrupted {', 'if false {'),
        ('initial-sequence-ignored', 'classify', 'if $receipt.description.sequence != 1 {', 'if false {'),
        ('initial-completion-accepted', 'classify', 'if $receipt.description.outcome == P::Completed {', 'if false {'),
        ('successor-gap-accepted', 'classify', 'if $receipt.description.sequence != next {', 'if { let _ = next; false } {'),
        ('terminal-outcome-reopened', 'classify', terminal, 'if false'),
        ('exact-duplicate-marked-recorded', 'classify', 'Ok(D::ExactDuplicate)', 'Ok(D::Recorded)'),
        ('record-forgets-last-receipt', 'record', '$record.last = Some($receipt.description);', 'let _ = &$receipt;'),
        ('record-mutates-before-refusal', 'record', 'let disposition = classify_receipt($record, &$receipt)?;',
         '$record.last = Some($receipt.description); let disposition = classify_receipt($record, &$receipt)?;'),
        ('record-clears-interruption', 'record', 'let disposition = classify_receipt($record, &$receipt)?;',
         '$record.interrupted = false; let disposition = classify_receipt($record, &$receipt)?;'),
        ('loss-clears-interruption', 'observe', '$record.interrupted = true;', '$record.interrupted = false;'),
        ('timeout-treated-as-loss', 'observe', '$event == DistributedObservationEventV1::ConnectionLost',
         '$event != DistributedObservationEventV1::ObserverDropped'),
        ('nonloss-reopens-interruption', 'observe', '$record.interrupted = true;\n        }',
         '$record.interrupted = true;\n        } else { $record.interrupted = false; }'),
    ]
    macros = {'classify': prefix, 'record': 'distributed_receipt_record_body_v1', 'observe': 'distributed_observation_body_v1'}
    result = {name: (change(body, macros[kind], before, after), SELECTORS[kind]) for name, kind, before, after in rows}
    need(len(result) == len(rows) == len(set(result.values())) == MUTANT_COUNT, 'sixteen distinct actual-body mutations, not outcomes')
    return result

def selection_notes(leaf, focus):
    need(focus in SELECTORS.values(), 'exact classifier/record/observe selector')
    need(isinstance(SELECTION_NOTES, dict) and set(SELECTION_NOTES) == set(SELECTORS.values())
         and all(isinstance(value, frozenset) and value and all(type(n) is str for n in value)
                 for value in SELECTION_NOTES.values()), 'actual selection diagnostics not measured')
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=SELECTION_NOTES[focus])

def inherited_controller():
    data = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(data).hexdigest() == BASE_SHA, 'authenticated unchanged process/classification controller')
    module = types.ModuleType('distributed_publication_contract_campaign')
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(data, module.__file__, 'exec'), module.__dict__)
    return module

def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, 'full positive count is unmeasured')
    for focus in SELECTORS.values():
        selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus)
    module = inherited_controller()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module

if __name__ == '__main__':
    runpy.run_path(str(ROOT / V / 'test-distributed-publication-contract.py'))
    campaign().main()
