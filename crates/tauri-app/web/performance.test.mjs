// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';import assert from 'node:assert/strict';
import {Keyboard} from './keyboard.mjs';import {Transport} from './transport.mjs';
import {gestureTarget,contextAction,contextValue,slotView,timelineHits,nearestMarker,partAction} from './performance.mjs';
import {mappingFromForm,profileEdit} from './midi.mjs';import {contract} from './contract.generated.mjs';import {makeRequest} from './protocol.mjs';
const key=(key,extra={})=>({key,type:'keydown',repeat:false,shiftKey:false,ctrlKey:false,altKey:false,metaKey:false,...extra});
test('selected control gestures cannot jump to a part selected mid-gesture by MIDI',()=>{
  const binding={deck:0,slot:1},s={selection:{deck:0,slots:[1,7]}};assert.deepEqual(gestureTarget(binding,s),binding);
  s.selection.deck=1;assert.throws(()=>gestureTarget(binding,s),/Selection changed/);assert.deepEqual(gestureTarget(null,s),{deck:1,slot:7});
});
test('all sixteen fixed contextual addresses generate gain/cutoff intents without selecting or launching',()=>{
  for(let d=0;d<2;d++)for(let s=0;s<8;s++){
    assert.deepEqual(contextAction(d,s,'Volume',.4),partAction(d,s,{Gain:.4}));
    assert.deepEqual(contextAction(d,s,'Filter',1),partAction(d,s,{Cutoff:null}));
    assert.ok(contextAction(d,s,'Filter',.5).action.Cutoff>20);
  }
  assert.equal(contextValue({cutoff_hz:null},'Filter'),1);assert.throws(()=>contextAction(0,0,'Volume',NaN));
});
test('projection separates queued execution from pending receipt and respects active part ID zero',()=>{
  const s={online:true,selection:{deck:1,slots:[0,7]},decks:[[],Array(8).fill(null)],feedback:[[],[]]};
  s.decks[1][7]={state:'QUEUED START',active:null,pending:{tick:3840}};
  assert.equal(slotView(s,1,7).active,false);assert.equal(slotView(s,1,7).selected,true);
  s.decks[1][7]={state:'PLAYING',active:0};assert.equal(slotView(s,1,7).active,true);
  assert.equal(slotView({online:false},0,0).disabled,true);
});
test('complete keyboard bindings use selected semantic actions and repeat only the crossfader',()=>{
  const k=new Keyboard();
  for(const [name,kind]of [['a','deck'],['b','deck'],['1','slot'],['8','slot'],[' ','selected'],['x','selected'],['Escape','selected'],['r','selected'],['v','mode'],['f','mode'],['F1','quantize'],['F4','quantize'],['q','cue'],['w','cue'],['ArrowLeft','crossfader_step'],['h',undefined]])assert.equal(k.handle(key(name))[0].kind,kind);
  assert.deepEqual(k.handle(key(' ',{repeat:true})),[]);assert.equal(k.handle(key('ArrowRight',{repeat:true}))[0].delta,.05);
  for(const extra of [{ctrlKey:true},{altKey:true},{metaKey:true}])assert.deepEqual(k.handle(key('a',extra)),[]);
  assert.deepEqual(k.handle(key('a'),true),[]);
});
test('momentary releases survive text entry, modifier release, repeated events and focus loss',()=>{
  const k=new Keyboard();assert.equal(k.handle(key('F',{shiftKey:true}))[0].pressed,true);
  assert.deepEqual(k.handle(key('F',{shiftKey:true,repeat:true})),[]);
  assert.equal(k.handle(key('f',{type:'keyup'}),true)[0].pressed,false);
  k.handle(key('v',{shiftKey:true}));assert.equal(k.handle(key('Shift',{type:'keyup'}))[0].kind,'release_modes');
  k.handle(key('f',{shiftKey:true}));assert.equal(k.release().length,3);assert.equal(k.held.size,0);
});
test('timeline canvas hit testing uses only Rust-projected fractions and selection',()=>{
  const p={markers:[{deck:1,slot:7,tick:15360,fraction:.5,kind:'Fill'}]};
  const hits=timelineHits(p,1000,112,{deck:1,slot:7});assert.equal(hits.length,2);
  assert.equal(nearestMarker(hits,hits[0].x,hits[0].y).slot,7);assert.equal(nearestMarker(hits,0,0),undefined);
});
test('MIDI forms encode typed profiles without implementing MIDI normalization or pickup',()=>{
  const form={target:JSON.stringify({DeckSlot:{deck:1,slot:7}}),action:'Context',deck:1,slot:8,mode:'Filter',grid:'Phrase',channel:16,kind:'CC',number:74,behavior:'Absolute',min:0,max:127,invert:false,step:.02};
  const m=mappingFromForm(form);assert.deepEqual(m.input,{channel:15,kind:'CC',number:74});assert.deepEqual(m.action,{Context:{DeckSlot:{deck:1,slot:7}}});
  assert.deepEqual(mappingFromForm({...form,action:'Momentary',behavior:'Button'}).action,{Momentary:'Filter'});
  const p=profileEdit(contract.default_profile,'add',null,m);assert.equal(p.mappings.length,contract.default_profile.mappings.length+1);assert.equal(contract.default_profile.mappings.length,p.mappings.length-1);
});
test('generated command fixtures cover every emitted control family',()=>{
  for(const a of contract.actions)assert.equal(makeRequest(a).command.kind,'perform');
  for(const c of contract.commands)assert.equal(makeRequest(c).command.kind,c.kind);
  assert.throws(()=>makeRequest({kind:'unsupported_synth'}));
});
function fakeInvoke(log,gate){return async(_,{request})=>{log.push(request);if(request.command.kind==='perform'&&gate)await gate;return {version:1,id:request.id,result:{Ok:{kind:'snapshot',data:{online:true}}}}}}
test('transport bounds admission, coalesces values and serializes release behind in-flight work',async()=>{
  let unlock;const gate=new Promise(r=>unlock=r),log=[],t=new Transport(fakeInvoke(log,gate));
  const work=Array.from({length:24},()=>t.call({kind:'selected',action:'Launch'}));
  await assert.rejects(t.call({kind:'selected',action:'Launch'}),/admission/);
  const released=t.release('Keyboard');unlock();await Promise.all(work);await released;
  assert.ok(log.some(r=>r.command.kind==='release_modes'));assert.ok(t.admitted<=1);
  t.coalesce('cross',{kind:'crossfader',value:.2});t.coalesce('cross',{kind:'crossfader',value:.8});assert.equal(t.latest.size,1);await t.flush();assert.equal(log.at(-1).command.action.value,.8);
});
test('queued held presses are discarded after release and rejected commands never become snapshots',async()=>{
  let unlock;const gate=new Promise(r=>unlock=r),log=[],t=new Transport(fakeInvoke(log,gate));
  const first=t.call({kind:'selected',action:'Launch'});const hold=t.call({kind:'hold_mode',mode:'Filter',token:1,pressed:true},'Keyboard');
  const release=t.release('Keyboard');unlock();await first;await hold;await release;
  assert.equal(log.filter(r=>r.command.action?.kind==='hold_mode').length,0);
  let snapshots=0;const reject=new Transport(async(_,{request})=>({version:1,id:request.id,result:{Err:{code:'Invalid',message:'conflict'}}}),()=>snapshots++);
  await assert.rejects(reject.call({kind:'selected',action:'Launch'}),/conflict/);assert.equal(snapshots,0);
});
