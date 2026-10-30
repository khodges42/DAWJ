// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {VERSION,makeRequest,readReply,invokeRequest} from './protocol.mjs';
const fixture=JSON.parse(readFileSync(new URL('./ipc-v1.examples.json',import.meta.url)));
test('Rust-shared v1 examples and all sixteen wire targets',()=>{
  assert.equal(fixture.version,VERSION);
  for(let deck=0;deck<2;deck++)for(let slot=0;slot<8;slot++) {
    const r=makeRequest({kind:'launch',deck,slot});
    assert.equal(r.version,VERSION);assert.equal(r.command.kind,'perform');
    assert.deepEqual(r.command.action,{kind:'launch',deck,slot});
  }
});
test('busy, unknown version, and mismatched replies never succeed',()=>{
  assert.throws(()=>readReply(fixture.reply,42),/Busy/);
  assert.throws(()=>readReply({...fixture.reply,version:2},42),/Unsupported/);
  assert.throws(()=>readReply(fixture.reply,43),/Mismatched/);
});
test('adapter carries correlation and returns snapshot without inventing playback',async()=>{
  const result=await invokeRequest(async(name,{request})=>{
    assert.equal(name,'request');assert.equal(request.command.kind,'snapshot');
    return {version:VERSION,id:request.id,result:{Ok:{kind:'snapshot',data:{online:false,decks:null}}}};
  },{kind:'snapshot'});
  assert.deepEqual(result,{online:false,decks:null});
});
