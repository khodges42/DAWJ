// SPDX-License-Identifier: GPL-3.0-or-later
// Actual WebView, Rust preparations, local factory WAVs and native output. No fake clock/PCM.
import {emptyPattern} from './sketch.mjs';
import {requestKey,visibleParts} from './overview-model.mjs';
import {inspectSurface,nextPaint} from './skin-safety.mjs';
export async function runJam4Smoke({t,getState,invoke,sketch,workspace,visuals,skinManager,options}){
 const $=id=>document.getElementById(id),assert=(v,m)=>{if(!v)throw Error(m)};
 const session=action=>t.call({kind:'session',action});
 const wait=async(p,label)=>{const end=performance.now()+15000;while(performance.now()<end){await t.call({kind:'snapshot'});if(p(getState()))return;await new Promise(r=>setTimeout(r,25))}throw Error('Timeout: '+label)};
 const ready=async()=>{await wait(s=>!s.session.busy,'background content');assert(!getState().session.message.startsWith('Content unchanged:'),getState().session.message)};
 const click=async id=>{assert(!$(id).disabled,'Disabled '+id);$(id).click();await Promise.resolve();await Promise.resolve();await t.serial;await ready()};
 const material=async()=>{await wait(s=>{const o=visuals.settings();return !visuals.cache.pending&&visuals.dataMode===visuals.mode&&visuals.cache.key===requestKey(s,visuals.mode,o.columns,o.detail)},'current material geometry')};
 const marker=async(label,bar)=>{$('section-preset').value=label;$('section-preset').dispatchEvent(new Event('change'));$('section-label').value=label;$('section-bar').value=bar;$('section-beat').value=1;$('section-intensity').value=({Intro:'0.2',Build:'0.5',Drop:'1',Groove:'0.5',Fill:'0.8'})[label]??'';await click('section-add')};
 const capture=async(name)=>{await material();await nextPaint();scrollTo(0,0);const errors=inspectSurface();assert(!errors.length,name+': '+errors.slice(0,7).join('; '));await invoke('jam_capture',{name});};
 try{
  assert(!getState().session.demo,'No demo fallback');
  if(options.phase==='create'){
   await skinManager.paint('bundled:dark',skinManager.prefs);await workspace.edit(0,0);await sketch.loadCatalog(false);await wait(()=>!sketch.catalog.loading,'factory scan');await sketch.loadCatalog(false);
   const sounds=sketch.catalog.entries.filter(e=>!e.error&&e.hints.kind!=='Loop');assert(sounds.length>=2,'Actual local factory WAVs required');
   for(let d=0;d<2;d++){
    await workspace.edit(d,0);$('sketch-composition-name').value=d?'Orbit after dark':'Morning machinery';await click('sketch-create');
    for(let slot=0;slot<8;slot++){
     sketch.select(d,slot);sketch.choose(sounds[(slot+d*3)%sounds.length]);await click('sound-assign');
     $('sketch-name').value=['Kick','Backbeat','Hat sequence','Bass pulse','Percussion','Free loop','Texture loop','Fill'][slot];await click('sketch-name-apply');
     if(slot<5){const p=emptyPattern();p.bars=[1,2,4,1,2][slot];for(let bar=0;bar<p.bars;bar++)for(let i=(d+slot)%4;i<16;i+=4)p.steps[bar][i]=i<4?127:bar%2?70:100;await sketch.setPattern(p);await ready()}
     else if(slot<7){const a=sketch.target(),frames=getState().session.media.find(m=>m.deck===d&&m.slot===slot).prepared_frames;await session({kind:'edit',composition:a.composition,slot,part:{...a.part,looping:true,loop_start:0,loop_end:Math.min(frames,slot===5?12000:18000)}});await ready()}
    }
    sketch.select(d,0);visuals.update(getState());await marker('Groove',3);await marker('Fill',8);
   }
   await session({kind:'rename',name:'Two scenes in motion'});
  }else{
   assert(visuals.mode==='hybrid','Visualization preference not restored');await session({kind:'open',path:options.path});await ready();
   const p=getState().session.project;assert(p.version===3&&p.compositions.length===2,'Versioned compositions');
   assert(p.compositions.every(c=>['Intro','Build','Drop'].every(label=>c.structure.sections.some(m=>m.label===label))),'Manual sections did not persist');
   assert(p.compositions.every(c=>c.parts.every(p=>p.factory)),'Factory references not restored');
  }
  await workspace.setLayout('performance');await visuals.setMode('structure');await material();
  assert(!getState().online,'Stopped overview must precede audio');
  assert(visuals.cache.data.decks.every(d=>d.parts.length===8&&d.parts.some(p=>p.density?.some(v=>v>0))&&d.parts.some(p=>p.energy.some(v=>v>0))),'Loaded musical content absent before launch');
  assert(visuals.cache.data.decks.every(d=>d.parts[1].period_ticks===7680&&d.parts[2].period_ticks===15360&&d.parts[5].repetitions>1&&d.parts[7].repetitions===1),'Patterns, free loops and one-shots misrepresented');
  await invoke('skin_test_window',{width:1600,height:1000});await nextPaint();
  if(options.phase==='create')await capture('jam4-structure-stopped');
  const devices=await t.call({kind:'devices'}),matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));assert(matches.length===1,'Unique HyperX required; no fallback');
  await t.call({kind:'start',device:matches[0].id,cue:false,phrase_bars:8});await wait(s=>s.online,'fresh output');
  await t.call({kind:'quantize',grid:'Beat'});for(const slot of [0,1,5])await t.call({kind:'launch',deck:0,slot});
  await wait(s=>s.decks[0][0].pattern.running&&s.decks[0][5].active!=null&&s.peaks.some(v=>v>0),'audible A pattern / free loop');await material();
  const clock=getState().overview_positions[0],len=getState().session.project.compositions[0].structure.bars*3840;assert(Math.abs(clock.fraction-clock.tick/len)<.00001,'Rust clock projection');assert(clock.cursors.some(c=>c.slot===5&&c.source>=0&&c.source<=1),'Actual source cursor absent');
  await workspace.edit(0,0);const old=structuredClone(sketch.target().part.pattern),edited=structuredClone(old);edited.steps[0][1]=old.steps[0][1]===100?90:100;
  await wait(s=>s.beat%3840>450&&s.beat%3840<1100,'observable bar edit');await sketch.setPattern(edited);await ready();await wait(s=>s.decks[0][0].pattern.pending_revision!=null,'queued pattern replacement');await material();
  let projected=visuals.cache.data.decks[0].parts.find(p=>p.slot===0);assert(JSON.stringify(projected.pattern)===JSON.stringify(old)&&projected.target_pattern,'Unconfirmed edit replaced active material');assert(getState().overview_positions[0].events.some(e=>e.label==='Edit'),'Pending edit boundary missing');
  await wait(s=>s.decks[0][0].pattern.revision===s.decks[0][0].pattern.target_revision&&!s.decks[0][0].pattern.pending_revision,'engine edit activation');await material();projected=visuals.cache.data.decks[0].parts[0];assert(JSON.stringify(projected.pattern)===JSON.stringify(edited)&&projected.target_pattern==null,'Authoritative edit not reflected');
  const frame=getState().frame,a=getState().decks[0][0].active;await workspace.edit(1,0);await material();assert(getState().online&&getState().decks[0][0].active===a&&getState().frame>=frame,'Inspecting B interrupted A');
  for(const mode of ['waveform','hybrid']){await visuals.setMode(mode);await material();const data=visuals.cache.data;assert(data.decks.every(d=>d.parts.some(p=>p.peaks?.some(e=>e[0].some(v=>v!==0)||e[1].some(v=>v!==0)))),'No actual projected peaks');assert(data.sources.length===1,'Only inspected source detail should cross IPC');const source=data.sources[0],info=getState().session.media.find(m=>m.deck===1&&m.slot===0);assert(source.id===info.waveform&&source.frames===info.prepared_frames&&source.peaks.some(p=>p.rms>0),'Source preview disagrees with decoded asset identity / frames');}
  if(options.phase==='create')for(let d=0;d<2;d++){await workspace.edit(d,0);await marker('Intro',1);await marker('Build',5);await marker('Drop',7);}
  if(options.phase==='create'){
   await marker('Temporary',4);const composition=getState().session.project.compositions[1],structure=structuredClone(composition.structure);structure.sections.find(s=>s.label==='Temporary').intensity=.1;await session({kind:'structure',composition:composition.id,structure});await ready();const b=[...$('section-list').querySelectorAll('button')].find(b=>b.textContent.startsWith('Temporary'));b.click();$('section-label').value='Repositioned';$('section-beat').value=2;await click('section-update');assert(getState().session.project.compositions[1].structure.sections.find(s=>s.label==='Repositioned').intensity===.1,'Renaming lost authored emphasis');await click('section-remove');assert(!sketch.getState().session.project.compositions[1].structure.sections.some(s=>s.label==='Repositioned'),'Remove marker failed');
  }
  for(const slot of [0,2,5])await t.call({kind:'launch',deck:1,slot});await wait(s=>s.decks[1][0].pattern.running&&s.decks[0][0].pattern.running,'independent two-deck playback');
  const durationBefore=visuals.cache.data.decks[0].parts[5].source_seconds;await t.call({kind:'tempo',bpm:124});await wait(s=>s.bpm===124,'engine tempo change');await material();assert(visuals.cache.data.bpm===124&&visuals.cache.data.decks[0].parts[5].source_seconds===durationBefore,'Tempo stretched source data or stale preview');
  await workspace.setLayout('performance');await material();
  const counts=[visuals.staticDraws,visuals.overlayDraws,visuals.requests],frozen=getState();for(let i=0;i<100;i++)visuals.update(frozen);assert(visuals.staticDraws===counts[0]&&visuals.overlayDraws===counts[1]&&visuals.requests===counts[2],'Repeated snapshots rebuild geometry or redraw unchanged overlays');
  for(const value of [.2,.8,.55]){$('cross').value=value;$('cross').dispatchEvent(new Event('input',{bubbles:true}));while(t.latest.size||t.sending){await t.flush();await new Promise(r=>setTimeout(r,10))}await t.serial;}
  const before=getState().frame;for(const layout of ['browse','extended','performance']){await workspace.setLayout(layout);await t.call({kind:'snapshot'});assert(getState().online&&getState().decks[0][0].active===a,'Workspace reset playback');}
  assert(getState().frame>before,'Clock did not advance');
  if(options.phase==='create'){for(const mode of ['structure','waveform','hybrid']){await visuals.setMode(mode);await material();await capture('jam4-'+mode);}await invoke('skin_test_window',{width:1024,height:768});await nextPaint();await capture('jam4-hybrid-small');await invoke('skin_test_window',{width:1600,height:1000});await nextPaint();}
  await visuals.setMode('hybrid');await material();const o=visuals.settings(),unchanged=await t.call({kind:'overview',style:'hybrid',columns:o.columns,visible:visibleParts(getState()),detail:o.detail,known:getState().overview_revision});assert(unchanged.data===null,'Unchanged overview retransmitted waveforms');
  await session({kind:'save',path:options.path});assert(!getState().session.dirty,'Save dirty state');await t.call({kind:'stop'});
  await invoke('report',{metrics:{jam4:true,phase:options.phase,passed:true,real_audio:true,clean_audio_shutdown:true,project:getState().session.project.id,assignments:getState().session.project.decks,markers:getState().session.project.compositions.map(c=>c.structure.sections.length),visualization:visuals.mode,loaded_stopped_material:true,actual_source_peaks:true,pending_patterns:true,unchanged_geometry_suppressed:true,static_draws:visuals.staticDraws,requests:visuals.requests,controller_tested:false,physical_cue_tested:false,human_usability_tested:false}});
 }catch(e){try{await t.call({kind:'stop'})}catch{}await invoke('report',{metrics:{jam4:true,phase:options.phase,error:String(e)}})}
}
