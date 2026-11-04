// SPDX-License-Identifier: GPL-3.0-or-later
import {emptyPattern} from './sketch.mjs';
export async function runJam2Smoke({t,getState,invoke,sketch,options}) {
 const assert=(v,m)=>{if(!v)throw Error(m)},session=a=>t.call({kind:'session',action:a});
 const wait=async(p,label)=>{const end=performance.now()+10000;while(performance.now()<end){await t.call({kind:'snapshot'});if(p(getState()))return;await new Promise(r=>setTimeout(r,25))}throw Error('Timeout '+label)};
 const ready=async()=>{await wait(s=>!s.session.busy,'preparation');assert(!getState().session.message.startsWith('Content unchanged:'),getState().session.message)};
 try {
  await sketch.toggle();await sketch.loadCatalog(false);
  let catalog;for(let i=0;i<100;i++){catalog=await t.call({kind:'catalog',rescan:false});if(!catalog.loading)break;await new Promise(r=>setTimeout(r,50))}
  const sounds=catalog.entries.filter(e=>!e.error&&e.hints.kind!=='Loop');assert(sounds.length>=2,'Need two supplied factory sounds');
  if(options.phase==='create'){
   for(let d=0;d<2;d++){
    await session({kind:'create',deck:d,name:`Original pattern ${d?'B':'A'}`});await ready();sketch.select(d,0);
    sketch.choose(sounds[d]);document.getElementById('sound-search').value=sounds[d].hints.display_name;sketch.renderSounds();assert(document.querySelector('#sound-list button'),'Search result');
    await sketch.assign();await ready();
    let p=emptyPattern();p.steps[0][d]=127;p.steps[0][4+d]=64;p.steps[0][8+d]=110;await sketch.setPattern(p);await ready();
    await session({kind:'undo'});await ready();assert(!sketch.target().part.pattern,'Pattern undo');await session({kind:'redo'});await ready();assert(sketch.target().part.pattern,'Pattern redo');
   }
  } else {await session({kind:'open',path:options.path});await ready();assert(getState().session.project.compositions.length===2,'Independent compositions reopened');assert(getState().session.project.compositions.every(c=>c.parts[0].factory&&c.parts[0].pattern.steps.flat().filter(Boolean).length>=3),'Factory assignments/patterns reopened');}
  const devices=await t.call({kind:'devices'}),matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));assert(matches.length===1,'Require unique HyperX; no fallback');
  await t.call({kind:'start',device:matches[0].id,cue:false,phrase_bars:8});await wait(s=>s.online,'audio output');
  await sketch.audition(sounds[0]);await new Promise(r=>setTimeout(r,250));await t.call({kind:'audition',reference:null,deck:null,slot:null,stop:true});
  for(let d=0;d<2;d++)await t.call({kind:'launch',deck:d,slot:0});
  await wait(s=>s.decks.every(d=>d[0].pattern.running&&d[0].pattern.triggers>=3)&&s.peaks.some(v=>v>0),'two audible factory patterns');
  const before=getState().frame;await sketch.toggle();assert(getState().online,'Sketch switch closed output');await sketch.toggle();assert(getState().frame>=before,'Clock moved backwards');
  sketch.select(0,0);const pattern=structuredClone(sketch.target().part.pattern);pattern.steps[0][12]=115;await sketch.setPattern(pattern);await ready();await wait(s=>s.decks[0][0].pattern.revision===s.decks[0][0].pattern.target_revision&&!s.decks[0][0].pattern.pending_revision,'live edit boundary');
  await session({kind:'save',path:options.path});assert(!getState().session.dirty,'Dirty state after save');
  await t.call({kind:'stop'});await invoke('report',{metrics:{jam2:true,passed:true,phase:options.phase,real_audio:true,clean_audio_shutdown:true,project:getState().session.project.id,assignments:getState().session.project.decks,sounds:catalog.entries.length}});
 }catch(e){try{await t.call({kind:'stop'})}catch{}await invoke('report',{metrics:{jam2:true,error:String(e),phase:options.phase}})}
}
