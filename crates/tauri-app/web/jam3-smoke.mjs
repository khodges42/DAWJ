// SPDX-License-Identifier: GPL-3.0-or-later
// Real WebView DOM, shared worker, factory WAVs and CPAL. No simulated musical clock.
import {emptyPattern} from './sketch.mjs';
import {inspectSurface,nextPaint} from './skin-safety.mjs';
export async function runJam3Smoke({t,getState,invoke,sketch,workspace,skinManager,options}){
 const $=id=>document.getElementById(id),assert=(v,m)=>{if(!v)throw Error(m)},session=action=>t.call({kind:'session',action});
 const wait=async(p,label)=>{const until=performance.now()+12000;while(performance.now()<until){await t.call({kind:'snapshot'});if(p(getState()))return;await new Promise(r=>setTimeout(r,25))}throw Error('Timeout: '+label)};
 const ready=async()=>{await wait(s=>!s.session.busy,'content preparation');assert(!getState().session.message.startsWith('Content unchanged:'),getState().session.message)};
 const click=async id=>{assert(!$(id).disabled,'Disabled '+id);$(id).click();await Promise.resolve();await Promise.resolve();await t.serial;await ready()};
 const input=async(id,value)=>{$(id).value=value;$(id).dispatchEvent(new Event('input',{bubbles:true}));await t.flush();await t.serial;await t.call({kind:'snapshot'})};
 const capture=async(layout,width,height,zoom=1)=>{await invoke('skin_test_zoom',{zoom});await invoke('skin_test_window',{width,height});await workspace.setLayout(layout);await nextPaint();await new Promise(r=>setTimeout(r,220));assert(getComputedStyle(document.body).fontFamily.includes('system-ui')||getComputedStyle(document.body).fontFamily.includes('Segoe'),'Base stylesheet missing');await invoke('jam_capture',{name:`jam3-${layout}-${width}${zoom>1?'-hidpi':''}`});const errors=inspectSurface();assert(!errors.length,layout+' '+width+': '+errors.slice(0,8).join('; '));
  const a=$('deck-0').getBoundingClientRect(),m=$('mixer').getBoundingClientRect(),b=$('deck-1').getBoundingClientRect(),cross=$('cross').getBoundingClientRect();assert(a.right<=m.left&&m.right<=b.left,'Mixer is not between the decks');assert(cross.width>=140,'Crossfader unreachable');assert(getState().online,'Layout stopped audio');};
 try{
  assert(!getState().session.demo,'No demo fallback');
  if(options.phase==='create'){
   await skinManager.paint('bundled:dark',skinManager.prefs);
   await workspace.edit(0,0);await sketch.loadCatalog(false);await wait(()=>!sketch.catalog.loading,'factory catalog');await sketch.loadCatalog(false);
   const sounds=sketch.catalog.entries.filter(e=>!e.error&&e.hints.kind!=='Loop');assert(sounds.length>=2,'Need real local factory samples');
   for(let d=0;d<2;d++){
    await workspace.edit(d,0);$('sketch-composition-name').value=d?'Night garden':'Signal bloom';await click('sketch-create');
    for(let slot=0;slot<8;slot++){
     sketch.select(d,slot);sketch.choose(sounds[(d*3+slot)%sounds.length]);await click('sound-assign');
     $('sketch-name').value=['Pulse','Clap','Hat','Bass','Texture','Chord','Accent','Fill'][slot]+(d?' / dusk':' / dawn');$('sketch-color').value=d?'#e2b67e':'#73c5df';await click('sketch-name-apply');
     let p=emptyPattern();p.steps[0][d]=127;p.steps[0][4+d]=80;p.steps[0][8+d]=110;await sketch.setPattern(p);await ready();
    }
   }
   await session({kind:'rename',name:'Signal bloom × Night garden'});
   sketch.select(0,0);const original=structuredClone(sketch.target().part.pattern);await session({kind:'undo'});await ready();await session({kind:'redo'});await ready();assert(JSON.stringify(sketch.target().part.pattern)===JSON.stringify(original),'Authoring undo/redo');
  }else{
   assert(workspace.layout==='extended','Workspace preference did not survive restart');
   await session({kind:'open',path:options.path});await ready();const p=getState().session.project;
   assert(p.compositions.length===2&&p.decks[0]!==p.decks[1],'Original compositions did not restore');
   assert(p.compositions.every(c=>c.parts.every(p=>p.factory&&p.pattern&&p.color)),'Assignments, patterns and colors not restored');
   assert(p.compositions[0].parts[0].variations[0]&&p.compositions[0].parts[0].variations[1],'Pattern variations not restored');
   assert(Math.abs(p.settings.crossfader-.68)<.001&&Math.abs(p.settings.deck_gain[0]-.72)<.001,'Mixer settings not restored');
   assert(!getState().online,'Open auto-started transport');
  }
  const devices=await t.call({kind:'devices'}),matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));assert(matches.length===1,'Require unique HyperX output; no fallback');
  $('device').value=matches[0].id;await t.call({kind:'start',device:matches[0].id,cue:false,phrase_bars:4});await wait(s=>s.online&&s.decks.every(d=>d.every(p=>p.available)),'resident original compositions');
  if(options.phase==='reopen')await wait(s=>Math.abs(s.decks[0][0].gain-.58)<.002&&s.decks[1][0].cutoff_hz===1200&&Math.abs(s.deck_gain[0]-.72)<.001&&Math.abs(s.deck_gain[1]-.84)<.001&&Math.abs(s.crossfader-.68)<.001,'restored engine gain/filter and mixer targets before any new gesture');
  assert($('cue-0').disabled&&$('cue-1').disabled&&!getState().cue_available,'Stereo falsely offers isolated cue');
  await t.call({kind:'quantize',grid:'Beat'});for(let d=0;d<2;d++)for(let slot=0;slot<3;slot++)await t.call({kind:'launch',deck:d,slot});
  await wait(s=>s.decks.every(d=>d[0].pattern.running&&d[0].pattern.triggers>=3)&&s.peaks.some(v=>v>0),'audible simultaneous original patterns');
  await workspace.setLayout('performance');const initial=getState().frame;
  await input('cross',.18);await input('cross',.82);await input('cross',.68);await input('deck-gain-0',.72);await input('deck-gain-1',.84);
  await t.call({kind:'select_part',deck:0,slot:0});await input('part-gain',.58);await t.call({kind:'part',deck:1,slot:0,action:{Cutoff:1200}});
  await wait(s=>Math.abs(s.decks[0][0].gain-.58)<.002&&s.decks[1][0].cutoff_hz===1200,'gain/filter execution');assert(getState().deck_peaks.every(v=>v>0),'No real deck meter levels');
  workspace.palette();$('command-search').value='center crossfader';$('command-search').dispatchEvent(new Event('input'));$('command-results').querySelector('button').click();await t.serial;await t.call({kind:'snapshot'});assert(Math.abs(getState().crossfader-.5)<.001,'Palette mixer command routing');await input('cross',.68);
  window.dispatchEvent(new KeyboardEvent('keydown',{key:'f',shiftKey:true}));await t.serial;await wait(s=>s.selection.temporary&&s.selection.mode==='Filter','momentary mode activated');window.dispatchEvent(new Event('blur'));await t.serial;await wait(s=>!s.selection.temporary,'focus loss releases held mode');
  await t.call({kind:'quantize',grid:'Bar'});await t.call({kind:'stop_part',deck:1,slot:1});await wait(s=>!s.decks[1][1].pattern.running,'queued stop executes');await wait(s=>s.beat%3840>600&&s.beat%3840<2000,'inside bar for observable cancellation');await t.call({kind:'launch',deck:1,slot:1});await wait(s=>!!s.decks[1][1].pending,'engine-published queued launch');await t.call({kind:'part',deck:1,slot:1,action:'Cancel'});await t.call({kind:'launch',deck:1,slot:1});await wait(s=>s.decks[1][1].pattern.running,'launch after cancel');await t.call({kind:'part',deck:0,slot:1,action:'Retrigger'});
  await workspace.setLayout('extended');assert(getState().frame>initial&&getState().decks[0][0].pattern.running,'Extended interrupted playback');
  await workspace.edit(0,0);const base=structuredClone(sketch.target().part.pattern);await session({kind:'variation',composition:sketch.target().composition,slot:0,index:0,store:true});
  const edited=structuredClone(base);edited.steps[0][12]=120;await sketch.setPattern(edited);await ready();await wait(s=>s.decks[0][0].pattern.revision===s.decks[0][0].pattern.target_revision&&!s.decks[0][0].pattern.pending_revision,'Sketch edit activation');
  await session({kind:'variation',composition:sketch.target().composition,slot:0,index:1,store:true});await session({kind:'variation',composition:sketch.target().composition,slot:0,index:0,store:false});await wait(s=>s.decks[0][0].pattern.revision===s.decks[0][0].pattern.target_revision&&!s.decks[0][0].pattern.pending_revision,'variation recall execution');
  await workspace.setLayout('performance');assert(getState().decks[1][0].pattern.running,'Sketch edit interrupted other deck');
  if(options.phase==='create'){
   for(const [width,height]of [[1280,900],[1024,768]])for(const layout of ['browse','performance','extended'])await capture(layout,width,height);
   await capture('performance',1280,900,1.25);await invoke('skin_test_zoom',{zoom:1});
  }
  await workspace.setLayout('extended');await session({kind:'save',path:options.path});assert(!getState().session.dirty,'Save did not clear dirty state');
  await t.call({kind:'stop'});await invoke('report',{metrics:{jam3:true,passed:true,phase:options.phase,real_audio:true,clean_audio_shutdown:true,project:getState().session.project.id,assignments:getState().session.project.decks,workspace:workspace.layout,original_parts:16,stereo_cue_disabled:true,physical_cue_tested:false,controller_tested:false,human_usability_tested:false}});
 }catch(e){try{await t.call({kind:'stop'})}catch{}await invoke('report',{metrics:{jam3:true,phase:options.phase,error:String(e)}})}
}
