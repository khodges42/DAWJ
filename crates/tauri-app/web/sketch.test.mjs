// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import {emptyPattern,patternEdit,filterSounds} from './sketch.mjs';
test('pattern authoring preserves velocity, rotates across bars and duplicates independently',()=>{
 const a=patternEdit(emptyPattern(),'toggle',0,15,127),b=patternEdit(a,'duplicate');
 assert.equal(a.steps[0][15],127);assert.equal(b.bars,2);
 const c=patternEdit(b,'rotate');assert.equal(c.steps[0][0],127);assert.equal(c.steps[1][0],127);
 b.steps[1][15]=64;assert.equal(a.steps[0][15],127);assert.equal(b.steps[0][15],127);
 assert.ok(patternEdit(b,'clear').steps.flat().every(v=>v===0));
});
test('browser filters generic packs, categories and optional filename hints with corrections',()=>{
 const e={reference:{pack:'new-pack',path:'Drums/K.wav'},category:'Drums',filename:'K_C.wav',hints:{display_name:'Kick',kind:'OneShot',note:'C',bpm:null}};
 const c={entries:[e]};assert.equal(filterSounds(c,{}, {search:'kick',pack:'new-pack',pitch:'c'}).length,1);
 assert.equal(filterSounds(c,{}, {min:'100'}).length,0);
 assert.equal(filterSounds(c,{'new-pack/Drums/K.wav':{...e.hints,bpm:120}}, {min:'100',max:'130'}).length,1);
 assert.equal(filterSounds(c,{}, {category:'Bass'}).length,0);
});
