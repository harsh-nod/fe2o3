#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
cd "$stage"
node <<'NODE'
const fs=require('fs'),crypto=require('crypto');
const root='/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const raw=fs.readFileSync('source-before.log');
if(sha(raw)!=='fa3dfa4fa0a5faa2c92c5f84726174f308498e208af6e11b16a6a242cb8ba008')throw Error('frozen manifest changed');
const doc=JSON.parse(raw);
if(doc.base!=='b87f30d1b87b2dca29e9f03e8b00f99a65b04391'||Object.keys(doc.files).length!==5543)throw Error('source scope');
for(const[name,digest]of Object.entries(doc.files))if(sha(fs.readFileSync(root+'/'+name))!==digest)throw Error('source '+name);
for(const [local,name] of [['copy-host-observe.py','benchmarks/runtime_gfx942/copy-host-observe.py'],['copy_accounting.rs.txt','crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs']])if(sha(fs.readFileSync(local))!==doc.files[name])throw Error('copied source '+local);
const binary='target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-afaad0775e07e3e7';
const expected='84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312';
if(sha(fs.readFileSync(root+'/'+binary))!==expected||sha(fs.readFileSync('runtime-test'))!==expected)throw Error('binary mismatch');
console.log(JSON.stringify({base:doc.base,sourceFiles:Object.keys(doc.files).length,sourceManifestSha256:sha(raw),observerSha256:sha(fs.readFileSync('copy-host-observe.py')),testSourceSha256:sha(fs.readFileSync('copy_accounting.rs.txt')),binarySourcePath:root+'/'+binary,binarySha256:expected,sourceMatchesFrozenMap:true},null,2));
NODE
file runtime-test
readelf -h runtime-test
readelf -l runtime-test
if readelf -l runtime-test | grep -q INTERP; then exit 1; fi
sha256sum runtime-test copy-host-observe.py copy_accounting.rs.txt source-before.log
