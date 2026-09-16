
const fs=require('fs'), cp=require('child_process'), crypto=require('crypto'), assert=require('assert');
const root='/home/harsh/.codex-tmp/', repo=root+'fe2o3-r61-execution';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const consumed=new Map();
function bytes(n){if(!consumed.has(n))consumed.set(n,fs.readFileSync(root+n));return consumed.get(n);}
const read=n=>JSON.parse(bytes(n));
assert.strictEqual(hash(bytes('r125-development-evidence-v2.js')),'f13ef55ef7b1ab1cfd93bcbc095f6513345c647f704fc98e4c9c94c40d20628c');
const E=require(root+'r125-development-evidence-v2.js');
assert.strictEqual(hash(bytes('r125-development-full-plan-v3.json')),'a5df79bc041fa305a23b640aaa962d7a0d1804a32ada2b81a2814fbbd5063b2c');
const p=read('r125-development-full-plan-v3.json');
const map=read('r125-development-stack-repair-layout-01-source.json');
assert.strictEqual(hash(JSON.stringify(map)),p.source_map_sha256);
assert.strictEqual(Object.keys(map).length,p.source_count);
const base=['cargo','+nightly-2026-04-03'];
const filters=['persistent_cancel','persistent_allocation::tests','queue::dispatch_binding::control_release::tests::persistent::'];
const specs=[
 ['layout-01',[...base,'rustc','--locked','--offline','-p','fe2o3-runtime','--all-features','--lib','--','-Zprint-type-sizes'],'allocation-01','r125-development-run-v2.js'],
 ['runtime-01',[...base,'test','--locked','--offline','-p','fe2o3-runtime','--all-features','--lib'],'layout-01',p.runner],
 ['kfd-01',[...base,'test','--locked','--offline','-p','fe2o3-kfd','--all-features','--lib','--',...filters],'runtime-01',p.runner],
 ['format-02',[...base,'fmt','--all','--','--check'],'runtime-01',p.runner],
 ['clippy-01',[...base,'clippy','--locked','--offline','-p','fe2o3-kfd','-p','fe2o3-runtime','--all-features','--all-targets','--','-D','warnings'],'kfd-01',p.runner],
];
const results={};
for(const[suffix,command,previous,runner]of specs){
 const name='r125-development-stack-repair-'+suffix, r=read(name+'.json');
 E.checkRecord(r,name,command,map,'r125-development-stack-repair-'+previous+'.json',0,{read,bytes},
 {head:p.source_parent,cwd:repo,contract:'r125-development-raw-utc-boot-monotonic-v1',runner,after:map});
 assert.strictEqual(fs.readFileSync('/proc/sys/kernel/random/boot_id','utf8').trim(),p.admitted_boot);
 assert.strictEqual(r.clock.start.boot_id,p.admitted_boot);
 assert(r.elapsed_seconds<r.deadline_ms/1000);
 assert.deepStrictEqual(E.liveGroupMembers(r.process_group_cleanup.close.pgid),[]);
 results[suffix]=bytes(name+'.log').toString();
}
const git=args=>cp.execFileSync('git',args,{cwd:repo,encoding:'utf8',maxBuffer:64*1024*1024});
const accepted=p.full_runs.find(s=>s.kind==='gnu'), oldLog=git(['show',p.accepted_commit+':'+accepted.accepted_log]);
assert.strictEqual(hash(oldLog),accepted.accepted_log_sha256);
const oldTargets=E.executables(oldLog);
const counts={};
for(const kind of ['runtime','kfd']){
 const old=oldTargets.filter(t=>t.kind==='libtest'&&t.name.endsWith('/fe2o3_'+kind+')'));
 assert.strictEqual(old.length,1);
 const all=[...old[0].passing,...p.new_tests[kind]].sort();
 assert.strictEqual(all.length,p.counts[kind]);assert.strictEqual(new Set(all).size,all.length);
 const expected=kind==='runtime'?all:all.filter(n=>filters.some(f=>n.includes(f)));
 const log=results[kind+'-01'];E.assertPassing(log);
 const markers=[...log.matchAll(/^     Running (.+)$/gm)].map(m=>m[1].replace(/-[0-9a-f]{16}\)$/g,')'));
 assert.deepStrictEqual(markers,[old[0].name]);
 assert.deepStrictEqual(E.passing(log),expected);assert.deepStrictEqual(E.ignored(log),[]);
 assert.deepStrictEqual(E.totals(log),{harnesses:1,passed:expected.length,failed:0,ignored:0});
 const summaries=[...log.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)].map(m=>m.slice(1).map(Number));
 assert.deepStrictEqual(summaries,[[expected.length,0,0,0,all.length-expected.length]]);
 assert(p.new_tests[kind].every(n=>expected.includes(n)));
 counts[kind]=expected.length;
}
assert.strictEqual(results['format-02'],'');
assert.strictEqual((results['clippy-01'].match(/^    Finished .+$/gm)||[]).length,1);
assert(!/^(?:warning|error)(?:\[|:)/m.test(results['clippy-01']));
const file='crates/fe2o3-runtime/src/kfd_backend/qualification_drain_capture.rs';
const now=fs.readFileSync(repo+'/'+file,'utf8'), old=git(['show',p.source_parent+':'+file]);
const anchor='    #[test]\n    fn caller_authority_and_missing_history_cannot_enter_copy_only_observation()';
assert.strictEqual(now.split(anchor).length,2);assert.strictEqual(old.split(anchor).length,2);
assert.strictEqual(now.slice(now.indexOf(anchor)),old.slice(old.indexOf(anchor)));
assert.strictEqual(git(['rev-parse','HEAD']).trim(),p.source_parent);
const current=Object.fromEntries([...new Set(git(['ls-files','--cached','--others','--exclude-standard','-z']).split('\0'))].filter(n=>n&&!n.startsWith('docs/')).sort().map(n=>[n,hash(fs.readFileSync(repo+'/'+n))]));
assert.deepStrictEqual(current,map);
for(const[n,b]of consumed)assert.deepStrictEqual(fs.readFileSync(root+n),b);
console.log(JSON.stringify({development_only:true,packet_accepted:false,source_map_sha256:p.source_map_sha256,source_count:p.source_count,exact_roster_passed:counts,all_18_added_tests_observed:true,original_overflow_fixture_byte_identical:true,format:'pass',strict_clippy:'pass',records:specs.map(([s])=>({name:'r125-development-stack-repair-'+s+'.json',sha256:hash(bytes('r125-development-stack-repair-'+s+'.json'))}))}));
