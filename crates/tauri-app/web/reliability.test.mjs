// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';import assert from 'node:assert/strict';
import {Timing} from './reliability.mjs';import {Transport} from './transport.mjs';
test('timing histogram has fixed storage and retains slow outliers',()=>{
 const t=new Timing();for(let i=1;i<=100;i++)t.record(i);t.record(NaN);t.record(-1);
 assert.deepEqual(t.summary(),{n:100,median:50,p95:95,worst:100});t.record(2000);assert.equal(t.summary().worst,2000);assert.equal(t.bins.length,10001);
});
test('100000 encoder-like frontend updates retain one latest target while IPC stalls',async()=>{
 let unlock;const gate=new Promise(r=>unlock=r),log=[];
 const t=new Transport(async(_,{request})=>{log.push(request);await gate;return {version:1,id:request.id,result:{Ok:{kind:'snapshot',data:{online:true}}}}});
 t.coalesce('cross',{kind:'crossfader',value:0});const first=t.flush();
 for(let i=0;i<100000;i++)t.coalesce('cross',{kind:'crossfader',value:i/100000});
 assert.equal(t.latest.size,1);assert.equal(t.sending,true);assert.equal(t.admitted,1);
 unlock();await first;await t.flush();assert.equal(log.length,2);assert.equal(log[1].command.action.value,.99999);
});
