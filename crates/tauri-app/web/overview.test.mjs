// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import {MaterialCache,requestKey,overlayKey,resolution,frameFraction,visualization,visibleParts} from './overview-model.mjs';
import {editSection,sectionTick,emphasisOptions} from './structure.mjs';
import {SkinStore,DEFAULT_PREFERENCES} from './skin-store.mjs';
import {SkinManager} from './skins.mjs';
test('stale snapshots and late material for old sources or styles are discarded',()=>{
 const c=new MaterialCache(),s={control_revision:4,overview_revision:'new',session:{project:{decks:[1,2]}}};c.snapshot(s);
 assert.equal(c.snapshot({...s,control_revision:3,overview_revision:'old'}),false);assert.equal(c.revision,'new');
 assert.equal(c.accept({revision:'old',data:{}},'old-key','new-key'),false);assert.equal(c.data,null);
 assert.equal(c.accept({revision:'new',data:{decks:[]}},'new-key','new-key'),true);assert.equal(c.builds,1);
 assert.equal(c.accept({revision:'new',data:{}},'waveform-key','hybrid-key'),false);assert.equal(c.builds,1);
});
test('clock-only updates do not invalidate static material or repaint a repeated generation',()=>{
 const a={control_revision:1,overview_revision:'material',generation:4,frame:100,online:true,session:{project:{decks:[1,2]}}},b={...a,control_revision:100,generation:6,frame:200};
 assert.equal(requestKey(a,'structure',256,null),requestKey(b,'structure',256,null));assert.notEqual(overlayKey(a),overlayKey(b));assert.equal(overlayKey(a),overlayKey({...a,control_revision:100}));
 assert.notEqual(requestKey(a,'waveform',256,null),requestKey(a,'hybrid',256,null));assert.equal(visibleParts(a).length,16);
});
test('DPI resolution and source-frame mapping are bounded without a frontend musical clock',()=>{
 assert.equal(resolution(400,2),832);assert.equal(resolution(10000,4),1024);assert.equal(resolution(0),32);
 assert.equal(frameFraction({source_start:100,source_end:300},{source:.5},{frames:400}),.5);assert.equal(frameFraction({},null,{}),null);
 assert.throws(()=>visualization('automatic-drop-detector'));
});
test('markers add rename reposition and remove without changing the original source object',()=>{
 const s={bars:8,sections:[],show_labels:true};const a=editSection(s,'upsert',{id:1,tick:sectionTick(5,1),label:'Drop',intensity:null});
 const b=editSection(a,'upsert',{id:2,tick:sectionTick(1,1),label:'Intro',intensity:null});assert.equal(b.sections[0].label,'Intro');assert.equal(s.sections.length,0);
 const c=editSection(b,'upsert',{id:1,tick:sectionTick(4,2.5),label:'Custom quiet drop',intensity:.1});assert.equal(c.sections[1].tick,3*3840+1440);assert.equal(c.sections[1].label,'Custom quiet drop');assert.equal(editSection(c,'remove',{id:1}).sections.length,1);
 assert.deepEqual(emphasisOptions(null),[]);assert.deepEqual(emphasisOptions(0),[]);assert.equal(emphasisOptions(c.sections[1].intensity)[0].value,'0.1');
});
test('visualization preferences migrate and survive skin and workspace changes in the existing store',()=>{
 const map=new Map(),storage={getItem:k=>map.get(k),setItem:(k,v)=>map.set(k,v)},store=new SkinStore(storage);
 const manager={committed:{...DEFAULT_PREFERENCES},prefs:{...DEFAULT_PREFERENCES},store};SkinManager.prototype.setVisualization.call(manager,'hybrid');SkinManager.prototype.setWorkspace.call(manager,'browse');
 assert.equal(new SkinStore(storage).load().prefs.visualization,'hybrid');assert.equal(new SkinStore(storage).load().prefs.workspace,'browse');
 manager.prefs.selected='bundled:light';store.savePreferences(manager.prefs);assert.equal(store.load().prefs.visualization,'hybrid');
 store.savePreferences({...DEFAULT_PREFERENCES,visualization:undefined});assert.equal(store.load().prefs.visualization,'structure');
});
