// SPDX-License-Identifier: GPL-3.0-or-later
// Explicit --jam-smoke. Real local WAV files, CPAL, and a fresh process for reopen.
export async function runJamSmoke({t,getState,invoke,content,options}){
 const $=id=>document.getElementById(id),assert=(v,m)=>{if(!v)throw Error(m)};
 const session=action=>t.call({kind:'session',action});
 const wait=async(predicate,label)=>{const until=performance.now()+7000;while(performance.now()<until){await t.call({kind:'snapshot'});if(predicate(getState()))return;await new Promise(r=>setTimeout(r,20))}throw Error('Timeout: '+label)};
 const ready=async()=>{await wait(s=>!s.session.busy,'content preparation');assert(!getState().session.message.startsWith('Content unchanged:'),getState().session.message)};
 const click=async id=>{assert(!$(id).disabled,'Disabled '+id);$(id).click();await Promise.resolve();await Promise.resolve();await t.serial;await ready()};
 try{
  assert(!getState().session.demo&&getState().session.project.compositions.length===0,'Application must start empty');
  if(options.phase==='create'){
   for(let deck=0;deck<2;deck++){
    content.open(deck,0);$('jam-composition-name').value=`User jam ${deck+1}`;await click('jam-create');
    await content.import(options.audio[deck]);await ready();
    $('jam-part-name').value=`My sound ${deck+1}`;$('jam-loop').checked=true;$('jam-length').value=8;await click('jam-apply');
    assert(getState().session.media.find(m=>m.deck===deck&&m.slot===0)?.sample_rate===(deck?48000:44100),'Source metadata');
   }
   await session({kind:'rename',name:'My original local jam'});
  }else{
   await session({kind:'open',path:options.path});await ready();const p=getState().session.project;
   assert(p.name==='My original local jam'&&p.compositions.length===2,'Session identities/references not reopened');
   assert(p.decks.every(id=>p.compositions.some(c=>c.id===id)),'Deck assignments not restored');
   assert(p.compositions.every(c=>c.parts[0].name.startsWith('My sound')&&c.parts[0].nominal_ticks===7680&&c.parts[0].media),'Part data not restored');
   assert(Math.abs(p.settings.crossfader-.68)<.001&&Math.abs(p.compositions[0].parts[0].gain-.58)<.002,'Saved mixer/part settings not restored');
   assert(!getState().online&&getState().decks==null,'Open must not auto-start audio');
  }
  $('content').close();const devices=await t.call({kind:'devices'});const matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));assert(matches.length===1,'Require unique HyperX output; never fall back');
  $('device').value=matches[0].id;
  await t.call({kind:'start',device:matches[0].id,cue:false,phrase_bars:8});
  await wait(s=>s.online&&s.decks?.[0]?.[0]?.available&&s.decks?.[1]?.[0]?.available,'prepared user decks');
  if(options.phase==='reopen')await wait(s=>Math.abs(s.decks[0][0].gain-.58)<.002&&s.decks[1][0].cutoff_hz===1200,'restored engine parameters before new gestures');
  assert([...document.querySelectorAll('article button')].every(e=>{const r=e.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth}),'Both decks visible');
  await t.call({kind:'quantize',grid:'Beat'});
  await t.call({kind:'launch',deck:0,slot:0});await t.call({kind:'launch',deck:1,slot:0});
  await wait(s=>s.decks[0][0].active!=null&&s.decks[1][0].active!=null,'original parts playing');
  const playing=getState();assert(playing.decks[0][0].active!==playing.decks[1][0].active,'Distinct stable part identities');
  await t.call({kind:'crossfader',value:.18});await t.call({kind:'crossfader',value:.82});await t.call({kind:'crossfader',value:.68});
  await t.call({kind:'part',deck:0,slot:0,action:{Gain:.58}});await t.call({kind:'part',deck:1,slot:0,action:{Cutoff:1200}});
  await wait(s=>Math.abs(s.decks[0][0].gain-.58)<.002&&s.decks[1][0].cutoff_hz===1200,'gain/filter execution');
  await wait(s=>s.peaks.some(v=>v>0.0001),'real nonzero output samples');
  let refused=false;try{await session({kind:'remove',composition:getState().session.project.decks[0],slot:0})}catch{refused=true}assert(refused,'Active destination edit must reject');
  await t.call({kind:'stop_part',deck:0,slot:0});await wait(s=>s.decks[0][0].active==null&&s.decks[1][0].active!=null,'independent stop A');
  if(options.phase==='create'){
   const before=getState().frame,bPart=getState().decks[1][0].active;
   await session({kind:'edit',composition:getState().session.project.decks[0],slot:0,part:{...getState().session.project.compositions[0].parts[0],name:'My sound 1',loop_start:120}});await ready();
   assert(getState().frame>before&&getState().decks[1][0].active===bPart,'Loading A interrupted B');
  }
  await session({kind:'save',path:options.path});
  assert(!getState().session.dirty&&getState().session.path===options.path,'Save not committed');
  await t.call({kind:'stop_part',deck:1,slot:0});await wait(s=>s.decks[1][0].active==null,'independent stop B');
  await t.call({kind:'stop'});assert(!getState().online,'Output shutdown');
  await invoke('report',{metrics:{jam1:true,phase:options.phase,passed:true,empty_start:true,real_audio:true,independent_stop:true,clean_audio_shutdown:true,project:getState().session.project.id,assignments:getState().session.project.decks,source_rates:getState().session.media.map(m=>m.sample_rate),crossfader:.68}});
 }catch(e){try{await t.call({kind:'stop'})}catch{}await invoke('report',{metrics:{jam1:true,phase:options.phase,error:String(e)}})}
}
