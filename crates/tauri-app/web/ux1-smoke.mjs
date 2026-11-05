// SPDX-License-Identifier: GPL-3.0-or-later
// Scripted real WebView + shared Rust worker + factory WAVs + unique HyperX output.
import {inspectSurface,nextPaint} from './skin-safety.mjs';
import {pickOutput} from './instrument.mjs';
export async function runUx1Smoke({t,getState,invoke,sketch,workspace,visuals,instrument,skinManager,ensureOutput,options}){
 const $=id=>document.getElementById(id),assert=(ok,message)=>{if(!ok)throw Error(message)};
 const session=action=>t.call({kind:'session',action});
 const wait=async(predicate,label)=>{const end=performance.now()+15000;while(performance.now()<end){await t.call({kind:'snapshot'});if(predicate(getState()))return;await new Promise(r=>setTimeout(r,25))}throw Error('Timeout: '+label)};
 const ready=async()=>{await wait(s=>!s.session.busy,'content preparation');assert(!getState().session.message.startsWith('Content unchanged:'),getState().session.message)};
 const click=async id=>{assert(!$(id).disabled,'Disabled '+id);$(id).click();await Promise.resolve();await Promise.resolve();await t.serial;await ready();if(['sound-assign','assignment-stop'].includes(id))await wait(()=>!sketch.assigning&&$('assignment-status').textContent.startsWith('Assigned'),'assignment outcome')};
 const drain=async()=>{while(t.latest.size||t.sending){await t.flush();await new Promise(r=>setTimeout(r,20))}await t.serial};
 const material=async()=>{await wait(s=>!!visuals.cache.data&&!visuals.cache.pending&&visuals.cache.revision===s.overview_revision,'analyzed material');await nextPaint()};
 const capture=async(name)=>{await material();scrollTo(0,0);await nextPaint();const errors=inspectSurface();assert(!errors.length,name+': '+errors.join('; '));await invoke('jam_capture',{name})};
 try{
  await skinManager.paint('bundled:dark',skinManager.prefs);
  await sketch.loadCatalog(false);await wait(()=>!sketch.catalog.loading,'factory index');await sketch.loadCatalog(false);
  assert(sketch.catalog.packs.length>=4&&sketch.catalog.entries.length>=853,'Expanded factory library not discovered');
  assert(sketch.catalog.packs.reduce((n,p)=>n+Object.keys(p.sources??{}).length,0)>=50,'Provenance records missing');
  const valid=sketch.catalog.entries.filter(e=>!e.error),kick=valid.find(e=>e.filename.includes('A01_808_Kick')),loop=valid.find(e=>e.filename.includes('Mystery_Garage_Loop')),hat=valid.find(e=>e.filename.includes('A03_LM2')),third=valid.find(e=>e.origin?.source&&e.reference.pack!==kick?.reference.pack);
  assert(kick&&loop&&hat&&third,'Actual diagnosis sounds unavailable');
  const devices=await t.call({kind:'devices'}),matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));assert(matches.length===1,'Unique HyperX required; no fallback');
  const device=pickOutput(devices,matches[0].name);$('device').value=device.id;$('device').dispatchEvent(new Event('change'));if(options.phase==='create')skinManager.setAudio({device:device.name,cue:false});
  if(options.phase==='reopen'){
   assert(skinManager.committed.audio.device===device.name,'Output preference not restored');assert(getState().online&&getState().transport==='Paused'&&getState().decks.every(d=>d.every(p=>p.active==null)),'Configured audio did not initialize automatically');await t.call({kind:'stop'});
   await session({kind:'open',path:options.path});await ready();assert(getState().session.project.compositions.length===2,'Two compositions did not reopen');assert(getState().session.project.compositions.every(c=>c.parts[0].factory.sample_id),'Source identity did not persist');assert(skinManager.committed.audio.device===device.name,'Output preference did not persist');
  }
  await instrument.initializeAudio();await wait(s=>s.online&&s.transport==='Paused','automatic audio initialization');assert(getState().decks.every(d=>d.every(p=>p.active==null)),'Opening output launched sounds');
  if(options.phase==='create'){
   await workspace.edit(0,0);sketch.choose(kick);await click('sound-assign');
   for(const step of [0,4,8,12])$('step-'+step).click();await sketch.patternEdits;await ready();assert(sketch.target().part.pattern.steps[0].filter(v=>v>0).length===4,'Rapid steps lost edits');
   await wait(s=>s.transport==='Paused'&&s.decks[0][0].pattern.revision===s.decks[0][0].pattern.target_revision,'stopped pattern activation while paused');
   await instrument.playPart(0,0);await wait(s=>s.decks[0][0].pattern.running&&s.deck_peaks[0]>0.00001,'audible A groove');
   const identity=getState().decks[0][0].active;
   $('sketch-gain').value=0;$('sketch-gain').dispatchEvent(new Event('input'));await drain();await wait(s=>s.decks[0][0].gain===0&&s.deck_peaks[0]<0.000001,'measured native part mute');
   $('sketch-gain').value=.8;$('sketch-gain').dispatchEvent(new Event('input'));$('sketch-filter').value=.45;$('sketch-filter').dispatchEvent(new Event('input'));await drain();await wait(s=>s.decks[0][0].gain>.79&&s.decks[0][0].cutoff_hz!=null&&s.deck_peaks[0]>0.000001,'applied audible DSP');
   await workspace.edit(0,1);sketch.choose(loop);await click('sound-assign');assert(getState().decks[0][0].active===identity,'Assigning stopped A2 interrupted A1');assert($('step-0').disabled&&!$('pattern-convert').hidden,'Continuous-loop restriction is unexplained');
   await instrument.playPart(0,1);await wait(s=>s.decks[0][1].active!=null,'Garage loop playback');
   let rejected=false;try{await sketch.assign(hat,{deck:0,slot:1})}catch{rejected=true}assert(rejected&&$('assignment-status').textContent.includes('Stop'),'Active assignment did not report rejection');
   sketch.choose(loop);await click('assignment-stop');assert(getState().decks[0][0].active===identity,'Safe replacement stopped other parts');
   await material();const sourceFrames=instrument.regionFrames();const handle=instrument.handles.find(h=>h.dataset.bound==='sample_start'),rect=$('region-canvas').getBoundingClientRect();
   handle.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:9,clientX:rect.left,clientY:rect.top}));handle.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerId:9,clientX:rect.left+rect.width*.05,clientY:rect.top}));handle.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:9}));
   assert(instrument.region?.sample_start>0,'Graphical trim handle did nothing');await instrument.applyRegion();await ready();assert(sketch.target().part.sample_start===Math.round(sourceFrames*.05),'Trim did not become authoritative');
   await click('pattern-convert');assert(!sketch.target().part.looping&&!$('step-0').disabled,'Loop-to-step workflow failed');for(const step of [2,6,10,14])$('step-'+step).click();await sketch.patternEdits;await ready();await instrument.playPart(0,1);
   const transfer=new DataTransfer();transfer.setData('application/x-dawj-sample',hat.reference.sample_id);$('slot-0-2').dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:transfer}));await wait(s=>!sketch.assigning&&!s.session.busy&&s.session.project.compositions[0].parts[2].factory?.sample_id===hat.reference.sample_id,'browser drag assignment');
   await workspace.edit(1,0);sketch.choose(third);await click('sound-assign');for(const step of [0,7,12])$('step-'+step).click();await sketch.patternEdits;await ready();await instrument.playPart(1,0);
   instrument.sourcePanel.open();assert($('source-facts').textContent.includes(third.reference.sample_id)&&$('source-document').textContent.includes('#'),'Original source view missing');await invoke('jam_capture',{name:'ux1-source'});$('sample-source').close();
   await workspace.edit(1,1);sketch.choose(loop);await click('sound-assign');await instrument.playPart(1,1);
   await session({kind:'rename',name:'UX1 two-deck groove'});
   const bpm=$('bpm');bpm.dispatchEvent(new MouseEvent('dblclick',{bubbles:true}));bpm.value='126.5';bpm.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));await t.serial;await wait(s=>s.bpm===126.5,'precise direct tempo');
   bpm.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:10,clientX:100,clientY:100}));bpm.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerId:10,clientX:115,clientY:100,shiftKey:true}));bpm.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:10}));await drain();await wait(s=>s.bpm>126.5&&s.bpm<127,'fine tempo drag');
   await instrument.transport('Pause');await wait(s=>s.transport==='Paused','global pause');const paused=getState();for(let i=0;i<10;i++)await t.call({kind:'snapshot'});assert(getState().frame===paused.frame&&getState().decks[0][0].position===paused.decks[0][0].position,'Pause drifted');await instrument.transport('Play');await wait(s=>s.frame>paused.frame,'resume without reset');
   await t.call({kind:'deck_transport',deck:0,playing:false});await wait(s=>s.decks[0].every(p=>p.active==null)&&s.decks[1][1].active!=null,'independent deck stop');
   await t.call({kind:'deck_transport',deck:0,playing:true});await wait(s=>s.decks[0][0].pattern.running,'deck play');
   for(const layout of ['browse','extended','performance']){await workspace.setLayout(layout);assert(getState().online,'Workspace closed output')}
   await invoke('skin_test_window',{width:1920,height:1080});await capture('ux1-performance');
   await workspace.edit(0,0);await material();assert($('sketch').contains($('step-0'))&&!$('launch-0-0').hidden,'Sketch part actions disappeared');await capture('ux1-sketch');
   await instrument.help.open('getting-started');assert($('help-article').textContent.includes('Your first groove'),'Offline help not rendered');$('help-search').value='continuous loop';$('help-search').dispatchEvent(new Event('input'));assert($('help-navigation').children.length>0,'Help search missing');await invoke('jam_capture',{name:'ux1-help'});$('help').close();
   const opener=$('session-open');opener.focus();opener.click();await Promise.resolve();$('content').dispatchEvent(new Event('cancel',{cancelable:true}));assert(!$('content').open&&document.activeElement===opener,'Escape dismissal/focus restoration failed');
   await workspace.setLayout('performance');const scaling=[];
   for(const [width,height,zoom]of [[1600,900,1],[1280,800,1],[1600,1000,1.25],[1920,1080,1.5]]){await invoke('skin_test_window',{width,height});await invoke('skin_test_zoom',{zoom});await nextPaint();await capture(`ux1-${width}-${Math.round(zoom*100)}`);scaling.push([innerWidth,innerHeight,devicePixelRatio])}
   await invoke('skin_test_zoom',{zoom:1});await invoke('skin_test_window',{width:1600,height:900});
   await session({kind:'save',path:options.path});assert(!getState().session.dirty,'Session save failed');
  }else{
   for(const d of [0,1])await instrument.playPart(d,0);await wait(s=>s.decks.every(d=>d[0].pattern.running),'restored patterns perform again');assert(getState().session.project.compositions[0].parts[1].sample_start>0,'Trim did not persist');
   await workspace.edit(0,0);await click('assigned-source-open');assert($('source-facts').textContent.includes(sketch.target().part.factory.sample_id),'Assigned provenance not resolvable after reopening');$('sample-source').close();
   await instrument.help.open('samples');assert($('help-article').textContent.includes('Source and license'),'Help unavailable after restart');$('help').close();
  }
  await instrument.transport('Stop');await wait(s=>s.transport==='Paused'&&s.decks.every(d=>d.every(p=>p.active==null&&p.pending==null&&p.rule==null)),'global stop semantics');await t.call({kind:'stop'});
  await invoke('report',{metrics:{ux1:true,passed:true,phase:options.phase,real_audio:true,clean_audio_shutdown:true,project:getState().session.project.id,assignments:getState().session.project.decks,samples:sketch.catalog.entries.length,source_records:sketch.catalog.packs.reduce((n,p)=>n+Object.keys(p.sources??{}).length,0),rapid_steps:true,native_part_mute:true,stopped_slot_assignment:true,loop_to_pattern:true,graphical_trim:true,transport_pause_resume:true,offline_help:true,physical_cue_tested:false,controller_tested:false,human_usability_tested:false}});
 }catch(e){try{await invoke('jam_capture',{name:'ux1-failure'})}catch{}try{await t.call({kind:'stop'})}catch{}await invoke('report',{metrics:{ux1:true,phase:options.phase,error:String(e),stack:e.stack}})}
}
