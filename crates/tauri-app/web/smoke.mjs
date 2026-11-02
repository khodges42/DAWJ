// SPDX-License-Identifier: GPL-3.0-or-later
// Opt-in actual WebView/CPAL integration. No physical MIDI messages or profile files are written.
import {invokeRequest} from './protocol.mjs';
import {summary} from './model.mjs';
export async function runSmoke({t,getState,invoke,skinManager}){
  const skinKeys=['dawj.skins.v1','dawj.skin-preferences.v1','customCSS','theme','accent'],savedSkinStorage=new Map(skinKeys.map(k=>[k,localStorage.getItem(k)]));
  const restoreSkinStorage=()=>{for(const[k,v]of savedSkinStorage){if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)}};
  const $=id=>document.getElementById(id),assert=(yes,message)=>{if(!yes)throw Error(message)};
  const settle=async()=>{await Promise.resolve();await Promise.resolve();await t.serial;await new Promise(r=>setTimeout(r,15));await t.serial};
  const click=async id=>{assert(!$(id).disabled,`Disabled control: ${id}`);$(id).click();await settle()};
  const wait=async(predicate,label)=>{
    const end=performance.now()+2500;while(performance.now()<end){await t.call({kind:'snapshot'});if(predicate(getState()))return;await new Promise(r=>setTimeout(r,20))}throw Error('Timeout: '+label);
  };
  const key=async(key,options={})=>{window.dispatchEvent(new KeyboardEvent(options.type??'keydown',{key,bubbles:true,...options}));await settle()};
  const input=async(id,value)=>{$(id).value=value;$(id).dispatchEvent(new Event('input',{bubbles:true}));while(t.latest.size||t.sending){await t.flush();await new Promise(r=>setTimeout(r,10))}await settle()};
  let original;
  try{
    const safeStartup=(await invoke('skin_startup'))==='safe';if(safeStartup)assert(skinManager.current==='bundled:dark','Safe startup did not ignore persisted skin');
    const {skinSmoke}=await import('./skin-smoke.mjs');const skinMetrics=await skinSmoke(skinManager,invoke);
    const options=await invoke('reliability_options');
    const outputs=[...$('device').options].filter(o=>o.textContent.toLowerCase().includes(options.output.toLowerCase()));
    assert(options.output.trim()&&outputs.length===1,'Required test output missing or ambiguous: '+options.output);
    $('device').value=outputs[0].value;
    await click('start');
    const essential=[...document.querySelectorAll('article button,article .context,footer input,#transport button')];
    const outside=essential.filter(e=>{const r=e.getBoundingClientRect();return r.bottom>innerHeight||r.left<0||r.right>innerWidth}).map(e=>e.id);
    assert(outside.length===0,'Essential controls outside viewport: '+outside.join(','));
    await click('launch-0-0');await click('launch-1-3');
    let sawQueued=false;await wait(s=>{sawQueued ||= !!s.decks[0][0].pending;return s.decks[0][0].active!=null&&s.decks[1][3].active!=null},'two-deck playback');
    const frameBefore=getState().frame,playingIds=[getState().decks[0][0].active,getState().decks[1][3].active];
    await skinManager.preview('bundled:psychic-genesis');await t.call({kind:'snapshot'});assert(getState().frame>frameBefore&&getState().decks[0][0].active===playingIds[0]&&getState().decks[1][3].active===playingIds[1],'Skin preview interrupted audio');await skinManager.cancel();skinMetrics.skin_live_audio=true;
    await key('a');await key('2');await key(' ');await wait(s=>s.decks[0][1].active!=null,'keyboard selected launch');
    await key('F4');await key('3');await key(' ');await wait(s=>!!s.decks[0][2].pending,'queued phrase launch');
    const pendingId=getState().decks[0][2].pending.command_id;await key('Escape');await wait(s=>!s.decks[0][2].pending,'cancel queued ID');
    assert(getState().decks[0][2].active==null,'Cancel manufactured playback');
    await key('F2');await click('select-0-0');await key('f');assert(getState().selection.mode==='Filter','Persistent FILTER');
    await input('context-0-0',.6);await wait(s=>s.decks[0][0].cutoff_hz!=null,'filter accepted');const cutoff=getState().decks[0][0].cutoff_hz;
    await key('V',{shiftKey:true});assert(getState().selection.mode==='Volume'&&getState().selection.temporary,'Momentary VOLUME');
    await input('context-0-0',.55);await wait(s=>Math.abs(s.decks[0][0].gain-.55)<.002,'volume accepted');
    await key('Shift',{type:'keyup'});assert(getState().selection.mode==='Filter'&&!getState().selection.temporary,'Modifier release restores persistent mode');assert(getState().decks[0][0].cutoff_hz===cutoff,'Mode changed cutoff');
    await key('v');await key('F',{shiftKey:true});window.dispatchEvent(new Event('blur'));await settle();await wait(s=>s.selection.mode==='Volume'&&!s.selection.temporary,'focus loss release');
    await key('f',{type:'keyup'});await click('retrigger-selected');await key('F4');await click('stop-selected');await wait(s=>s.decks[0][0].pending?.kind==='Stop','queued stop');await click('cancel-selected');await wait(s=>!s.decks[0][0].pending,'cancel queued stop');await key('F2');
    await input('deck-gain-0',.8);await input('trim',30);await input('cross',.7);
    assert(Math.abs(getState().crossfader-.7)<.00001,'Crossfader');assert($('cue-0').disabled,'Stereo cue misrepresented');
    $('bpm').value=123;await click('tempo');await wait(s=>s.bpm===123,'tempo applied by scheduler');
    await click('select-0-7');$('rule-kind').value='beats';$('rule-number').value=1;await click('arm');await wait(s=>!!s.decks[0][7].rule,'rule installed');
    await wait(s=>s.decks[0][7].rule.fired>0,'periodic fill fired');
    assert(getState().timeline.generation===getState().generation,'Timeline generation mismatch');
    await click('rule-toggle');await wait(s=>s.decks[0][7].rule.next==null,'rule pause');await click('rule-toggle');
    const marker=getState().timeline.markers[0];if(marker){$('marker-list').value=0;$('marker-list').dispatchEvent(new Event('change'));await settle();assert(getState().selection.deck===marker.deck&&getState().selection.slots[marker.deck]===marker.slot,'Timeline selection')}
    await click('select-0-7');await click('rule-remove');await wait(s=>!s.decks[0][7].rule,'rule removal');
    await click('fill-b');await wait(s=>s.decks[1][7].active!=null,'one-shot fill');
    original=await t.call({kind:'midi_profile'});await click('midi-open');await click('profile-factory');
    assert($('settings').open,'MIDI dialog');
    $('map-kind').value='Note';$('map-number').value=127;$('map-action').value='Launch';$('map-behavior').value='Button';
    const before=(await t.call({kind:'midi_profile'})).mappings.length;await click('map-add');await wait(()=>$('mapping-list').children.length===before+1,'mapping installed');
    await click('map-add');assert($('settings-error').textContent.includes('conflict'),'Mapping conflict feedback');
    await click('map-learn');await wait(s=>s.midi_learning,'learn starts');await click('map-cancel-learn');await wait(s=>!s.midi_learning,'learn cancels');
    await t.call({kind:'midi_replace',profile:original});original=null;
    $('settings').close();document.activeElement?.blur();
    const samples=[];for(let i=0;i<100;i++){const start=performance.now();await invokeRequest(invoke,{kind:'snapshot'});samples.push(performance.now()-start)}
    await click('stop-0-0');await wait(s=>s.decks[0][0].active==null,'independent stop');assert(getState().decks[1][3].active!=null,'B stopped with A');
    const metrics={...skinMetrics,safe_startup:safeStartup,web2:true,controls_fit:true,viewport:[innerWidth,innerHeight],ipc_ms:summary(samples),audio_peaks:getState().peaks,saw_queued:sawQueued,cancelled_id:pendingId,keyboard:true,context_modes:true,focus_release:true,timeline:true,rules:true,midi_editor:true,independent_stop:true};
    await click('stop');assert(!getState().online,'Output still online');metrics.clean_audio_shutdown=true;
    await click('start');assert(getState().online&&!getState().decks.flat().some(p=>p.active!=null),'Restart reused old part state');
    await click('launch-0-0');await wait(s=>s.decks[0][0].active!=null,'software output restart');
    await click('stop');assert(!getState().online,'Restarted output did not close');metrics.software_restart=true;
    restoreSkinStorage();await invoke('report',{metrics});
  }catch(e){
    if(original)await t.call({kind:'midi_replace',profile:original}).catch(()=>{});
    await t.call({kind:'stop'}).catch(()=>{});restoreSkinStorage();await invoke('report',{metrics:{error:String(e),web2:true}});
  }
}
