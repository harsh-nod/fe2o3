// Opt-in real-filesystem readback only. Root launches launch-main separately; this test starts no child.
import test from 'node:test';
import assert from 'node:assert/strict';
import {performance} from 'node:perf_hooks';
import {filesystemProvider} from '../loaded-input-reader/reader-fs.mjs';
import {readPinnedRequest} from './launch-reader.mjs';
import {admitRequestSpec,sha} from './launch-model.mjs';
import {verifySyntheticLaunchResult} from './launch-fixture.mjs';
const json=v=>Buffer.from(JSON.stringify(v));
const encoded=process.env.FE2O3_STATIC_LAUNCH_READBACK;
assert.equal(typeof encoded,'string','explicit FE2O3_STATIC_LAUNCH_READBACK canonical base64 JSON is required; no fallback or skip');
assert(encoded.length>0&&encoded.length<=4*Math.ceil(65536/3),'bounded readback manifest argument');
const raw=Buffer.from(encoded,'base64');assert.equal(raw.toString('base64'),encoded);assert(raw.length>0&&raw.length<=65536);
const text=raw.toString('utf8');assert(Buffer.from(text).equals(raw));const config=JSON.parse(text);
assert.deepEqual(Object.keys(config).sort(),['schema','not_before_utc_ms','expires_utc_ms','max_elapsed_ms','process_exit','expected','stdout','final','temporary'].sort());
assert.equal(config.schema,'fe2o3-static-launch-readback-v1');
for(const k of ['not_before_utc_ms','expires_utc_ms','max_elapsed_ms'])assert(Number.isSafeInteger(config[k])&&config[k]>=0);
assert(config.expires_utc_ms>config.not_before_utc_ms&&config.expires_utc_ms-config.not_before_utc_ms<=86400000);
assert(config.max_elapsed_ms>0&&config.max_elapsed_ms<=30000);assert.equal(config.process_exit,0);
assert.deepEqual(Object.keys(config.expected).sort(),['bytes','sha256','base64'].sort());
assert(Number.isInteger(config.expected.bytes)&&config.expected.bytes>0&&config.expected.bytes<=65536);
assert.equal(typeof config.expected.base64,'string');assert.equal(config.expected.base64.length,4*Math.ceil(config.expected.bytes/3));
const expectedBytes=Buffer.from(config.expected.base64,'base64');assert.equal(expectedBytes.toString('base64'),config.expected.base64);
assert.equal(expectedBytes.length,config.expected.bytes);assert.equal(sha(expectedBytes),config.expected.sha256);
const expected=JSON.parse(expectedBytes.toString('utf8'));
for(const [name,index,cap] of [['stdout',3,8*1024*1024],['final',1,64*1024*1024],['temporary',0,64*1024*1024]]){
 admitRequestSpec(json(config[name]));assert(config[name].pin.bytes<=cap);assert.equal(config[name].pin.path,expected.outputs[index].path);
}
assert.equal(new Set(['stdout','final','temporary'].map(k=>config[k].pin.path)).size,3);
test('opt-in complete real-filesystem request to adapter to report readback',()=>{
 const start=performance.now();let previous=start,checks=0;
 const guard=()=>{assert(++checks<=20000,'finite readback guard checks');const mono=performance.now(),utc=Date.now();assert(Number.isFinite(mono)&&mono>=previous&&mono-start<config.max_elapsed_ms,'finite monotonic readback window');previous=mono;assert(utc>=config.not_before_utc_ms&&utc<config.expires_utc_ms,'external UTC readback scope');};
 const provider=filesystemProvider(),read=name=>{const result=readPinnedRequest(json(config[name]),provider,{guard});assert.equal(result.observation.status,'complete-request-observed',JSON.stringify(result.observation.first_failure));assert(result.bytes);assert.equal(result.observation.counts.opened,result.observation.counts.closed);assert.equal(result.observation.counts.content_calls,result.observation.ceiling.content_calls);return result.bytes;};
 const stdout=read('stdout'),final=read('final'),temporary=read('temporary');guard();
 const result=verifySyntheticLaunchResult(stdout,final,temporary,expectedBytes,config.process_exit);
 assert.equal(result.synthetic_request_to_adapter_to_bounded_report_observed,true);assert.equal(result.historical_operation_activated,false);assert.equal(result.native_authority,false);assert.equal(result.global_io_bound_proved,false);
});
