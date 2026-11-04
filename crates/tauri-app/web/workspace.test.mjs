// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import {WORKSPACES,workspace,partPresentation,commandIntent,shortcutCommand,searchCommands,Workspace,COMMANDS} from './workspace.mjs';
import {SkinStore,DEFAULT_PREFERENCES,PREFERENCES_KEY} from './skin-store.mjs';
import {Keyboard} from './keyboard.mjs';
test('workspace preferences extend the existing store and migrate previous preferences',()=>{
 const map=new Map(),storage={getItem:k=>map.get(k),setItem:(k,v)=>map.set(k,v)};
 const store=new SkinStore(storage);for(const value of WORKSPACES){store.savePreferences({...DEFAULT_PREFERENCES,workspace:value});assert.equal(new SkinStore(storage).load().prefs.workspace,value)}
 const {workspace:_,...old}=DEFAULT_PREFERENCES;storage.setItem(PREFERENCES_KEY,JSON.stringify(old));assert.equal(store.load().prefs.workspace,'performance');
 assert.throws(()=>workspace('tracker'));store.savePreferences({...DEFAULT_PREFERENCES,workspace:'bad'});assert.equal(store.load().prefs.workspace,'performance');
});
test('menu and palette route performance commands to the existing fixed part address',()=>{
 const state={selection:{deck:1,slots:[7,3]}};
 assert.deepEqual(commandIntent('deck.launch',state),{kind:'part',deck:1,slot:3,action:'Launch'});
 assert.deepEqual(commandIntent('deck.cancel',state),{kind:'part',deck:1,slot:3,action:'Cancel'});
 assert.deepEqual(commandIntent('mixer.center',state),{kind:'crossfader',value:.5});
 assert.deepEqual(commandIntent('edit.undo',state),{kind:'session',action:{kind:'undo'}});
 assert.equal(commandIntent('view.extended',state),null);
 assert.equal(searchCommands('save as')[0].id,'file.save-as');assert.equal(new Set(COMMANDS.map(c=>c.id)).size,COMMANDS.length);
});
test('workspace reconfiguration releases held input and never dispatches transport',async()=>{
 const events=[],elements=new Map(WORKSPACES.map(k=>['workspace-'+k,{focus:()=>events.push('focus')}]));
 const mock={release:()=>events.push('release'),layout:'performance',skinManager:{setWorkspace:v=>events.push(v)},sketch:{visible:true,toggle:async()=>events.push('context'),loadCatalog:async()=>events.push('catalog')},renderLayout:()=>events.push('layout'),update:()=>events.push('project'),getState:()=>({online:true}),$:id=>elements.get(id)};
 await Workspace.prototype.setLayout.call(mock,'extended');assert.deepEqual(events,['release','extended','context','layout','project','focus']);
 assert.equal(mock.layout,'extended');assert.ok(!events.includes('stop'));
});
test('projection distinguishes loaded material, active part zero and pending pattern revisions',()=>{
 const part={factory:{pack:'any',path:'Drums/Kick.wav'},pattern:{bars:1,steps:[Array(16).fill(100)]},variation:2};
 const s={session:{project:{decks:[42,null],compositions:[{id:42,parts:[part]}]}},decks:[[{active:0,pattern:{running:true,step:4,bars:1,target_revision:8,revision:7,pending_revision:8,pending_at:3840}}]]};
 const v=partPresentation(s,0,0);assert.equal(v.loaded,true);assert.equal(v.active,true);assert.equal(v.progress,.25);assert.equal(v.pending,true);assert.match(v.edit,/2\.1/);assert.equal(v.variation,'Variation 3');
 s.decks=null;const stopped=partPresentation(s,0,0);assert.equal(stopped.loaded,true);assert.equal(stopped.active,false);assert.equal(stopped.progress,0);assert.equal(stopped.pending,false);
});
test('navigation shortcuts do not steal plain performance keys or leave momentary input held',()=>{
 const event={type:'keydown',key:'2',ctrlKey:true};assert.equal(shortcutCommand(event),'view.performance');assert.equal(shortcutCommand({...event,ctrlKey:false}),null);
 assert.equal(shortcutCommand({...event,key:'s',shiftKey:true}),'file.save-as');assert.equal(shortcutCommand({...event,repeat:true}),null);
 const k=new Keyboard();k.handle({type:'keydown',key:'f',shiftKey:true});assert.equal(k.held.size,1);k.release();assert.equal(k.held.size,0);
});
