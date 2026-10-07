#!/usr/bin/env python3
"""Bind actual field operations; source calibration alone is not proof acceptance."""
import hashlib
import json
from pathlib import Path
import re
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
SRC = Path('crates/fe2o3-runtime-model/src')
V = Path('crates/fe2o3-runtime-model/verus')
OWNER = SRC / 'distributed_publication_contract.rs'
NATIVE = SRC / 'distributed_publication_contract/codec_fields.rs'
BODY = SRC / 'distributed_publication_contract/codec_fields_body.rs'
TESTS = SRC / 'distributed_publication_contract/codec_fields_tests.rs'
PROOF = V / 'distributed_codec_fields_v1.rs'
PRIMITIVE = V / 'distributed_codec_primitives_v1.rs'
CONSTRUCTION = V / 'distributed_publication_construction_v1.rs'
CLASSIFIER = V / 'distributed_publication_contract_v1.rs'
PINS = {
    SRC / 'distributed_publication_contract/classifier_body.rs': 'a9523b717e47851e50163e41a9cc4ff2ec31b10e814a90521644aaaa0158f18d',
    SRC / 'distributed_publication_contract/codec_primitives_body.rs': '222b9a154402bfad72a745ee6900fe29aead0b8f3e00db9a62d01e7498188f5c',
    SRC / 'distributed_publication_contract/construction_body.rs': '0057a0902bd3f8731affed1f51f8ea6296135a38588828515d5fe57149ea437a',
    SRC / 'distributed_publication_contract/declarations.rs': '39a5e23849a75df9af224b2a4193cd037b5bb39cbddc85c3de07ada299015363',
    PRIMITIVE: '2fac9dd32338e9e613bcd918571f893f1a1cc18228dc39aca14bad8bf92ef9f6',
    CONSTRUCTION: 'b0955242b7c9e8868ba917be3145451d8e26af0ddf3a49ddf7379b5155d24790',
    CLASSIFIER: '57edf7747e4c86c6e73b0311e59e06b7fb2dbf276fba5988e526dfd4fcacd853',
    OWNER: '5801e06a532ee2e8bd5db33f7284957c27e132aeb8f6fa85d75de2077bb0f771',
    NATIVE: '4649702b71345f403c5493347c5789d72cc21974d601329e9e85e2cc05d53ec7',
    BODY: '90c6e5ff451bdeafd39c6fe120119ce435cb9df2dc2038c60b0561dd859b3e83',
    TESTS: 'f2ea9fa5a064c3e85bff5a57d58e012c8f19b936db519276df4df8901ee581d8',
    PROOF: 'd40cefa6acd21c25e6f9d004f6c00118d01ba46304e7c4ebf36910ad7c5ddfe4',
}
FILES = [path for path in PINS if path not in (OWNER, NATIVE, TESTS)]
SOURCE_FILES = 308
SOURCE_TREE_SHA = '641e9e3c94274ba46410d9312e3259bde6999387c9eca3f4963394f1b31e7d73'
KINDS = ('read_header', 'write_header', 'read_u64', 'write_u64')
EXPECTED_VERIFIED = 63
DIAGNOSTICS = V / 'distributed_codec_fields_diagnostics_v1.py'
CALIBRATION = V / 'distributed_codec_fields_calibration_v1.json'
MUTATIONS = V / 'distributed_codec_fields_mutations_v1.json'
VERIFICATION_PINS = {
    DIAGNOSTICS: 'c9b364de00897bcfade87face6257f02aed50f0f58a015ae9085ab4e66738f7d',
    CALIBRATION: 'b0e1b6ba170b223c5dc5065622291d47435ec51ecde05657b71c3875becc8bfe',
    MUTATIONS: '9f21eea3b8fa08764d6e3299789bf63f9118235fd32f5e4787c647581e617d9e',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def invocation_guard(flags=None):
    flags = sys.flags if flags is None else flags
    need(getattr(flags, 'isolated', 0) and getattr(flags, 'dont_write_bytecode', 0)
         and not getattr(flags, 'optimize', 1), 'use isolated python3 -I -B without optimization')


invocation_guard()


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(path): sha(text) for path, text in sources.items()},
        sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def verification_metadata():
    buffers = {}
    for path, digest in VERIFICATION_PINS.items():
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary canonical verification metadata')
        data = selected.read_bytes()
        need(hashlib.sha256(data).hexdigest() == digest, 'exact reviewed classifier/calibration/mutation metadata')
        buffers[path] = data
    calibration, roster = [json.loads(buffers[path]) for path in (CALIBRATION, MUTATIONS)]
    need(calibration['expected_full_positive_verified'] == EXPECTED_VERIFIED
         and calibration['source_capture_accepted_negative_classifications'] == 0
         and calibration['source_capture_qualified_kills'] == 0
         and calibration['proof_inputs'] == {str(path): PINS[path] for path in FILES}
         and len(calibration['cases']) == len(roster['logical_candidates']) == 34
         and set(calibration['cases']) == set(roster['logical_candidates']),
         'measured full63/0 and unchanged nine-input 34-case calibration, not historical kills')
    need(roster['body_sha256'] == PINS[BODY] and roster['negative_count_qualified'] == 0
         and roster['compiler_or_solver_executed'] is False
         and len(roster['wrapper_structural_controls']) == 5
         and roster['panic_only_candidate']['logical_kill_claimed'] is False,
         'original source-only roster and separate panic/structural scope')
    for name, case in roster['logical_candidates'].items():
        observed = calibration['cases'][name]
        need(case['kind'] in (*KINDS, 'bytes_equal') and case['selector'] == '*' + case['kind']
             and case['qualified'] is False and case['candidate_logical_negative'] is True
             and all(observed[key] == case[key] for key in ('kind', 'selector', 'body_sha256')),
             'same actual-body mutation and selected calibrated family')
    return calibration, roster, buffers


def field_classifier():
    calibration, roster, buffers = verification_metadata()
    module = types.ModuleType('distributed_codec_fields_diagnostics_v1')
    module.__file__ = str(ROOT / DIAGNOSTICS)
    exec(compile(buffers[DIAGNOSTICS], module.__file__, 'exec'), module.__dict__)
    need(module.VSTD_SEQ_SHA == calibration['vstd_seq_sha256'], 'same authenticated vstd auxiliary source')
    return module


def mutations(original):
    calibration, roster, buffers = verification_metadata()
    need(sha(original) == PINS[BODY], 'unchanged actual field body before mutation')
    result = {}
    for name, case in roster['logical_candidates'].items():
        marker = 'macro_rules! distributed_codec_' + case['kind'] + '_body_v1 {'
        need(original.count(marker) == 1 and case['before'] != case['after'], 'unique selected macro')
        start = original.index(marker)
        end = original.find('\nmacro_rules! ', start + len(marker))
        end = len(original) if end < 0 else end
        region = original[start:end]
        need(region.count(case['before']) == 1, 'one substitution in selected actual shared body')
        body = original[:start] + region.replace(case['before'], case['after']) + original[end:]
        need(sha(body) == case['body_sha256'], 'exact calibrated mutant bytes')
        result[name] = (body, case['selector'])
    need(len(result) == len({body for body, selector in result.values()}) == 34, '34 distinct actual-body mutants')
    return result


def snapshot():
    paths = {PROOF, PRIMITIVE, CONSTRUCTION, CLASSIFIER}
    paths.update(path.relative_to(ROOT) for path in (ROOT / SRC).rglob('*.rs'))
    sources = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary canonical field source')
        sources[path] = selected.read_bytes().decode()
    return sources


def structural(sources):
    owner, native, body, proof = [sources[path] for path in (OWNER, NATIVE, BODY, PROOF)]
    declaration = 'const DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1: u16 = 1;'
    need(owner.count('pub ' + declaration) == proof.count(declaration) == 1,
         'exact production and proof schema declarations are both one')
    need(owner.count('mod codec_fields;') == owner.count('mod codec_fields_tests;') == 1,
         'actual private helper and native test modules')
    compact = re.sub(r'\s+', '', owner)
    forwarders = (
        'fnheader(&mutself,domain:&[u8]){codec_fields::write_header(self.bytes,&mutself.offset,domain);}',
        'fnu64(&mutself,value:u64){codec_fields::write_u64(self.bytes,&mutself.offset,value);}',
        'fnheader(&mutself,domain:&[u8])->Result<(),DistributedPublicationContractErrorV1>{codec_fields::read_header(self.bytes,&mutself.offset,domain)}',
        'fnu64(&mutself)->Result<u64,DistributedPublicationContractErrorV1>{codec_fields::read_u64(self.bytes,&mutself.offset)}',
    )
    need(all(compact.count(body) == 1 for body in forwarders),
         'all four actual Reader/Writer methods delegate only to the shared helpers')
    need(native.count('include!("codec_fields_body.rs");') == 1
         and proof.count('include!("distributed_codec_primitives_v1.rs");') == 1
         and proof.count('include!("../src/distributed_publication_contract/codec_fields_body.rs");') == 1
         and len(re.findall(r'\binclude!\(', proof)) == 2
         and not re.search(r'\binclude!\(', body), 'closed nine-file field proof')
    need('use super::codec_primitives::{fixed, put, take, u16_from_le, u16_le, u64_from_le, u64_le};'
         in native, 'exact real primitive names, no forwarding adapter')
    for kind in KINDS:
        macro = 'distributed_codec_' + kind + '_body_v1'
        need(native.count(macro + '!') == proof.count(macro + '!') == 1
             and body.count('macro_rules! ' + macro + ' {') == 1,
             'same actual field body called once by native and proof')
    comparator = 'distributed_codec_bytes_equal_body_v1'
    need(native.count(comparator + '!') == proof.count(comparator + '!') == 1
         and body.count('macro_rules! ' + comparator + ' {') == 1
         and 'pub(super)fnbytes_equal(left:&[u8],right:&[u8])->bool{'
             + comparator + '!(distributed_codec_fields_expr_v1,left,right,index,[])}'
             in re.sub(r'\s+', '', native)
         and body.count('if !bytes_equal(actual_domain, $domain) {') == 1
         and body.count('if reserved[0] != 0 || reserved[1] != 0 {') == 1,
         'actual shared equality loop and post-read scalar reserved comparison')
    stripped = re.sub(r'//[^\n]*', '', proof)
    need(not re.search(r'\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external|#\!?\[allow', stripped),
         'no new trusted comparison, executable shim or diagnostic allowance')
    need(len(re.findall(r'\brequires\b', stripped)) == 2
         and proof.count('requires\n        *old(offset) <= old(bytes)@.len(),') == 2,
         'only writer capacity premises; no reader entry-validity premise')
    need(not re.search(r'\b(?:Vec|Box|unsafe|alloc)\b|std::', body + native),
         'safe fixed-storage no_std field operations')
    need(sources[TESTS].count('#[test]') == 7, 'seven focused field differential tests')


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF, PRIMITIVE, CONSTRUCTION, CLASSIFIER}
         and len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         'complete reviewed native model source roster')
    need(all(sha(sources[path]) == digest for path, digest in PINS.items()),
         'exact field candidate and unchanged seven-file primitive proof closure')
    structural(sources)


if __name__ == '__main__':
    audit(snapshot())
    verification_metadata()
    print('PASS: codec field source binding (four field helpers plus shared equality, nine proof inputs, 34 calibrated mutants; no proof execution or qualification)')
