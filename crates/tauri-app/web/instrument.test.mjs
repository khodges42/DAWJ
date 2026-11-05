// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import {pickOutput,tempoValue,regionDrag,assignmentBlocked} from './instrument.mjs';
import {sourceFacts} from './library.mjs';
import {helpTarget,helpSearch} from './help.mjs';
import {Sketch} from './sketch.mjs';
import {SkinStore,DEFAULT_PREFERENCES,launchPreferences} from './skin-store.mjs';
import {loopFit} from './overview-model.mjs';
test('configured audio identity never silently falls back or resolves an ambiguous endpoint',()=>{
 const devices=[{id:1,name:'Default',is_default:true},{id:2,name:'Intended'}];assert.equal(pickOutput(devices,null).id,1);assert.equal(pickOutput(devices,'Intended').id,2);
 assert.throws(()=>pickOutput(devices,'Unplugged'));assert.throws(()=>pickOutput([...devices,devices[1]],'Intended'));assert.throws(()=>pickOutput([],null));
});
test('tempo accepts precision within bounds and trim handles keep valid nondestructive nested regions',()=>{
 assert.equal(tempoValue('126.53'),126.53);for(const v of [NaN,0,241,''])assert.throws(()=>tempoValue(v));
 const part={sample_start:0,sample_end:1000,loop_start:100,loop_end:300};const moved=regionDrag(part,1000,'sample_start',.8);assert.deepEqual(part,{sample_start:0,sample_end:1000,loop_start:100,loop_end:300});assert.equal(moved.sample_start,800);assert(moved.loop_end>moved.loop_start);
 const short=regionDrag(moved,1000,'sample_end',0);assert.equal(short.sample_end,801);assert(short.loop_start<short.loop_end);assert.throws(()=>regionDrag(part,0,'loop_end',1));
 assert.match(loopFit(2,120,3840),/^Fits/);assert.match(loopFit(2,130,3840),/differ.*No stretching/);
});
test('assignment safety distinguishes the addressed part from other running parts',()=>{
 const s={session:{busy:false},decks:[[{active:7},{active:null}],[]]};assert.equal(assignmentBlocked(s,0,0),true);assert.equal(assignmentBlocked(s,0,1),false);s.decks[0][1].rule={};assert.equal(assignmentBlocked(s,0,1),true);
});
test('organizational metadata cannot overwrite sample provenance or its per-sample license',()=>{
 const entry={reference:{pack:'stable',path:'Drums/Kick.wav',sample_id:'sample-one'},category:'Any virtual category',hints:{display_name:'My renamed sound'},origin:{source:'SOURCE.md',original_path:'bd8/BD00.WAV',license:'CC-BY-4.0',attribution:'Credit Alice'}};
 const catalog={packs:[{id:'stable',name:'Collection',license:'CC0-1.0',sources:{'SOURCE.md':{creator:'Alice',license:'Other',text:'Original record'}}}]};const facts=sourceFacts(catalog,entry);assert.equal(facts.creator,'Alice');assert.equal(facts.license,'CC-BY-4.0');assert.equal(facts.original,'bd8/BD00.WAV');assert.equal(facts.sampleId,'sample-one');assert.equal(facts.text,'Original record');
});
test('offline help routes only canonical local pages and searches actual Markdown',()=>{
 assert.deepEqual(helpTarget('sketch.md#continuous-loops'),{id:'sketch',anchor:'continuous-loops'});for(const href of ['javascript:alert(1)','https://example.org/x.md','../../engineering.md','data:text/html,x'])assert.equal(helpTarget(href),null);
 assert.equal(helpSearch([{id:'a',markdown:'# Loops\nStop part first'},{id:'b',markdown:'# MIDI'}],'loops stop')[0].id,'a');
});
test('rapid sequencer clicks compose against accepted state without losing preceding edits',async()=>{
 let pattern=null;const model={deck:0,slot:0,page:0,step:0,patternEdits:Promise.resolve(),target:()=>({part:{pattern}}),$:id=>id==='pattern-velocity'?{value:100}:null,setPattern:async p=>{await Promise.resolve();pattern=p},refreshVelocity(){}};
 await Promise.all([0,4,8,12].map(step=>Sketch.prototype.editPattern.call(model,'toggle',step)));assert.deepEqual(pattern.steps[0].filter(v=>v>0),[100,100,100,100]);
 const next=Sketch.prototype.editPattern.call(model,'toggle',1);model.slot=1;await assert.rejects(next,/selection changed/);assert.equal(pattern.steps[0][1],0);
});
test('audio preferences survive the existing settings store and old preferences migrate',()=>{
 const data=new Map(),store=new SkinStore({getItem:k=>data.get(k),setItem:(k,v)=>data.set(k,v)});store.savePreferences({...DEFAULT_PREFERENCES,audio:{device:'Intended',cue:true}});assert.deepEqual(store.load().prefs.audio,{device:'Intended',cue:true});store.savePreferences({...DEFAULT_PREFERENCES,audio:undefined});assert.deepEqual(store.load().prefs.audio,{device:null,cue:false});
 const safe=launchPreferences({...DEFAULT_PREFERENCES,selected:'user:broken',audio:{device:'Intended',cue:true}},'safe');assert.equal(safe.selected,'bundled:dark');assert.equal(safe.audio.device,'Intended');
});
