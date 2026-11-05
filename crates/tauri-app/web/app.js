// SPDX-License-Identifier: GPL-3.0-or-later
import {Transport} from './transport.mjs';
import {Keyboard} from './keyboard.mjs';
import {Instrument,Dialogs,pickOutput} from './instrument.mjs';
import {address,gestureTarget,partAction,contextValue,contextAction,slotView,meterDb,timelineHits,nearestMarker} from './performance.mjs';
import {Workspace,shortcutCommand} from './workspace.mjs';
import {MusicalOverview} from './overview.mjs';
import {Sketch} from './sketch.mjs';
import {ContentPanel} from './content.mjs';
import {MidiPanel} from './midi.mjs';
import {SkinManager,SkinPanel} from './skins.mjs';
import {browserStorage} from './skin-store.mjs';
const $=id=>document.getElementById(id),invoke=window.__TAURI__.core.invoke;
let state={online:false},errorText='',lastMode='Volume',hits=[],markerSignature='',dragging=new Map(),midi,content,sketch,workspace,visuals,instrument,dialogs;
let canAutoStart=false;
let automaticAudio=false,devices=[];
function error(e){errorText=String(e);$('error').textContent=errorText}
const t=new Transport(invoke,show,error),keyboard=new Keyboard();
const run=(a,source='Gui')=>{errorText='';$('error').textContent='';return t.call(a,source).catch(e=>{error(e);throw e})};
const fire=(a,source='Gui')=>{void run(a,source).catch(()=>{})};
function bind(id,fn){$(id).onclick=()=>Promise.resolve().then(fn).catch(error)}
function button(text,fn,title){const e=document.createElement('button');e.textContent=text;e.title=title??text;e.setAttribute('aria-label',title??text);e.onclick=fn;return e}
function setValue(id,value){const e=typeof id==='string'?$(id):id;const editing=document.activeElement===e&&e.tagName!=='SELECT'&&e.type!=='range';if(!editing&&!dragging.has(e))e.value=value}
function continuous(id,key,action){$(id).oninput=()=>{try{const target=id.startsWith('part-')?gestureTarget(dragging.get($(id))?.target,state):null;t.coalesce(key,action(Number($(id).value),target))}catch(e){error(e)}}}
for(let deck=0;deck<2;deck++){
  const section=document.createElement('section');section.className='deck';section.id='deck-'+deck;
  const header=document.createElement('header'),title=document.createElement('h2');title.textContent=`${deck?'B':'A'} | Empty deck`;
  const select=button('Select',()=>fire({kind:'deck',deck}),`Select deck ${deck?'B':'A'}`);select.id='deck-select-'+deck;
  const gain=document.createElement('input');gain.id='deck-gain-'+deck;gain.type='range';gain.min=0;gain.max=1;gain.step=.005;gain.setAttribute('aria-label',`Deck ${deck?'B':'A'} gain`);gain.dataset.live='';
  gain.oninput=()=>t.coalesce('deck-'+deck,{kind:'deck_gain',deck,value:Number(gain.value)});
  const cue=button('Cue',()=>fire({kind:'cue',deck}),`Toggle cue ${deck?'B':'A'}`);cue.id='cue-'+deck;cue.dataset.live='';const gainLabel=document.createElement('label');gainLabel.textContent='Gain';gainLabel.append(gain);header.append(title,select,gainLabel,cue);section.append(header);
  const slots=document.createElement('div');slots.className='slots';
  for(let slot=0;slot<8;slot++){
    const card=document.createElement('article');card.id=`slot-${deck}-${slot}`;
    const identity=button(`${deck?'B':'A'}${slot+1}`,()=>{workspace?.select(deck,slot);if(sketch?.visible){sketch.select(deck,slot);if(state.online)fire({kind:'select_part',deck,slot})}else if(state.online)fire({kind:'select_part',deck,slot});else {void workspace?.edit(deck,slot).catch(error)}});identity.className='identity';identity.id=`select-${deck}-${slot}`;identity.dataset.live='';
    const swatch=document.createElementNS('http://www.w3.org/2000/svg','svg');swatch.setAttribute('width','10');swatch.setAttribute('height','10');swatch.setAttribute('aria-hidden','true');const dot=document.createElementNS('http://www.w3.org/2000/svg','circle');dot.setAttribute('cx','5');dot.setAttribute('cy','5');dot.setAttribute('r','4');swatch.append(dot);const partLabel=document.createElement('span');partLabel.className='part-label';partLabel.textContent=identity.textContent;identity.replaceChildren(swatch,partLabel);
    const status=document.createElement('div');status.className='state';
    const actions=document.createElement('div');actions.className='actions';
    for(const [action,glyph]of [['Launch','▶'],['Stop','■'],['Cancel','×'],['Retrigger','↻']]){
      const b=button(glyph,()=>fire(partAction(deck,slot,action)),`${action} ${deck?'B':'A'}${slot+1}`);b.dataset.live='';b.dataset.action=action;b.id=`${action.toLowerCase()}-${deck}-${slot}`;actions.append(b);
    }
    const control=document.createElement('input');control.type='range';control.className='context';control.min=0;control.max=1;control.step=.005;control.dataset.live='';control.id=`context-${deck}-${slot}`;
    control.onpointerdown=()=>dragging.set(control,{mode:state.selection?.mode??'Volume',deck,slot});
    control.oninput=()=>{
      const binding=dragging.get(control)??{mode:state.selection?.mode??'Volume',deck,slot};
      if(binding.mode!==state.selection?.mode){error('Mode changed during gesture; release and operate the control again.');return}
      t.coalesce(`part-${deck}-${slot}`,contextAction(deck,slot,binding.mode,control.value));
    };
    control.onpointerup=control.onpointercancel=()=>dragging.delete(control);
    card.append(identity,status,actions,control);slots.append(card);
  }
  section.append(slots);$('decks').append(section);
}
$('decks').replaceChildren($('deck-0'),$('mixer'),$('deck-1'),$('sketch'));

document.querySelectorAll('input[type=range]:not(.context)').forEach(e=>{e.addEventListener('pointerdown',()=>dragging.set(e,{target:address(state)}));for(const event of ['pointerup','pointercancel','lostpointercapture'])e.addEventListener(event,()=>dragging.delete(e))});
midi=new MidiPanel(t,error);
content=new ContentPanel(t,()=>state,invoke,openDialog,error);
async function ensureOutput(){if(state.online)return;if(state.decks)throw Error('Audio failed or starting. Use Settings > Audio to close and retry.');if(!canAutoStart)throw Error('Select an output in Audio settings, then retry.');await run({kind:'start',device:Number($('device').value),cue:$('routing').value==='cue',phrase_bars:Number($('phrase').value)});const end=performance.now()+7000;while(performance.now()<end){await t.call({kind:'snapshot'});if(state.online)return;await new Promise(r=>setTimeout(r,20))}throw Error('Audio output did not publish a fresh state')}
sketch=new Sketch({t,getState:()=>state,invoke,ensureOutput,onError:error,openSession:()=>openDialog('content')});
sketch.onSuccess=()=>{errorText='';$('error').textContent='';if(instrument){instrument.region=null;instrument.regionKey=''}};
bind('view-toggle',()=>workspace.toggleSketch());
$('device').onchange=()=>{canAutoStart=true};

bind('session-open',()=>{const p=address(state);content.open(p.deck,p.slot)});
try {
  await window.__TAURI__.event.listen('dawj-audio-drop',event=>{
    try {const {path,x,y}=event.payload;const card=document.elementFromPoint(x/devicePixelRatio,y/devicePixelRatio)?.closest?.('article[id^=slot-]');if(card){const [,deck,slot]=card.id.split('-');content.open(Number(deck),Number(slot));}else if(!$('content').open){const p=address(state);content.open(p.deck,p.slot)}const target=content.target();void content.import(path,target).catch(e=>content.error(e));} catch(e){content.error(e)}
  });
  await window.__TAURI__.event.listen('dawj-drop-error',event=>content.error(event.payload));
} catch(e){error('Audio drag/drop unavailable: '+e)}
async function scan(){
  devices=await run({kind:'devices'});$('device').replaceChildren();for(const device of devices){const option=document.createElement('option');option.value=device.id;option.textContent=device.name+(device.is_default?' (system default)':'');$('device').append(option)}
  canAutoStart=false;if(automaticAudio){try{const selected=pickOutput(devices,skinManager.committed.audio?.device);$('device').value=selected.id;$('routing').value=skinManager.committed.audio?.cue?'cue':'stereo';canAutoStart=true}catch(e){error(e)}}else{const preferred=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));canAutoStart=preferred.length===1;if(canAutoStart)$('device').value=preferred[0].id}
  if(!devices.length)error('No audio output device. Connect one and Rescan.');
}
bind('rescan',scan);
bind('start',async()=>{const selected=devices.find(d=>d.id===Number($('device').value));if(!selected)throw Error('Select an output');skinManager.setAudio({device:selected.name,cue:$('routing').value==='cue'});canAutoStart=true;await ensureOutput();return run({kind:'transport',action:'Pause'})});
bind('stop',()=>{t.dropContinuous();return run({kind:'stop'})});
bind('tempo',()=>run({kind:'tempo',bpm:Number($('bpm').value)}));
$('quantize').onchange=()=>fire({kind:'quantize',grid:$('quantize').value});
for(const mode of ['Volume','Filter']){
  bind(mode.toLowerCase(),()=>{t.dropContinuous();return run({kind:'mode',mode})});
  const held=$('hold-'+mode.toLowerCase()),token=mode==='Volume'?11:12;
  held.onpointerdown=e=>{e.preventDefault();held.setPointerCapture(e.pointerId);fire({kind:'hold_mode',mode,token,pressed:true})};
  held.onpointerup=held.onpointercancel=()=>{void t.release('Gui')};
  held.onlostpointercapture=()=>{void t.release('Gui')};
}
for(const [id,action]of [['launch-selected','Launch'],['stop-selected','Stop'],['cancel-selected','Cancel'],['retrigger-selected','Retrigger']])bind(id,()=>run({kind:'selected',action}));
continuous('cross','crossfader',value=>({kind:'crossfader',value}));
continuous('trim','trim',db=>({kind:'output_trim',db}));
continuous('part-gain','selected-gain',(value,target)=>partAction(target.deck,target.slot,{Gain:value}));
continuous('part-cutoff','selected-cutoff',(value,target)=>contextAction(target.deck,target.slot,'Filter',value));
bind('bypass',()=>run(partAction(address(state).deck,address(state).slot,{Cutoff:null})));
function rule(kind){const p=address(state),phase=$('rule-phase').value,n=Number($('rule-number').value);return {kind:'rule_install',...p,rule:kind==='once'?{kind:'once',boundary:state.selection.quantization}:kind==='phrase'?{kind:'phrase_bar',bar:n,phrase_bars:state.musical_decks[p.deck].phrase_length/3840,phase}:{kind:'every',n,unit:kind==='beats'?'Beats':'Bars',phase}}}
bind('once',()=>run(rule('once')));bind('arm',()=>run(rule($('rule-kind').value)));
bind('rule-toggle',()=>{const p=address(state);return run({kind:'rule_enable',...p,enabled:state.decks[p.deck][p.slot].rule?.next==null})});
bind('rule-remove',()=>run({kind:'rule_cancel',...address(state)}));
for(const deck of [0,1]){bind('fill-'+(deck?'b':'a'),()=>run(partAction(deck,7,'Launch')));bind('cancel-fill-'+(deck?'b':'a'),()=>run(partAction(deck,7,'Cancel')))}
function timelineView(delta=0){return run({kind:'timeline_view',zoom_bars:Number($('zoom').value),follow:$('follow').checked,page_delta:delta})}
$('zoom').onchange=$('follow').onchange=()=>{void timelineView().catch(()=>{})};bind('previous',()=>timelineView(-1));bind('next',()=>timelineView(1));
$('timeline').onclick=e=>{const r=$('timeline').getBoundingClientRect(),marker=nearestMarker(hits,e.clientX-r.left,e.clientY-r.top);if(marker)fire({kind:'select_part',deck:marker.deck,slot:marker.slot})};
$('timeline').onmousemove=e=>{const r=$('timeline').getBoundingClientRect(),marker=nearestMarker(hits,e.clientX-r.left,e.clientY-r.top);$('timeline').title=marker?`${marker.deck?'B':'A'}${marker.slot+1} ${marker.kind} @ tick ${marker.tick}${marker.projected?' (conditional projection)':''}`:'Authoritative musical position; select a marker or use the scheduled part menu'};
$('marker-list').onchange=()=>{const m=state.timeline?.markers[Number($('marker-list').value)];if(m)fire({kind:'select_part',deck:m.deck,slot:m.slot})};
function release(){dragging.clear();void t.release('Gui');for(const c of keyboard.release())void t.release(c.source)}
window.addEventListener('blur',release);document.addEventListener('visibilitychange',()=>{if(document.hidden)release()});
function openDialog(id){if(dialogs)return dialogs.open(id);release();if(!$(id).open)$(id).showModal()}
bind('help-open',()=>openDialog('help'));
bind('midi-open',async()=>{openDialog('settings');await midi.open()});
document.querySelectorAll('[data-close]').forEach(e=>e.onclick=()=>$(e.dataset.close).close());
for(const type of ['keydown','keyup'])window.addEventListener(type,e=>{
  if(type==='keydown'&&e.ctrlKey&&e.altKey&&e.shiftKey&&e.key.toLowerCase()==='r'){e.preventDefault();void invoke('restore_interface').catch(error);return}
  const shortcut=shortcutCommand(e);
  if(shortcut&&!(document.querySelector('dialog[open]')&&!['palette','file.save','file.save-as'].includes(shortcut))){if(['edit.undo','edit.redo'].includes(shortcut)&&e.target.closest?.('input,textarea,[contenteditable=true]'))return;e.preventDefault();void workspace.execute(shortcut).catch(error);return}
  const editing=!!document.querySelector('dialog[open]')||!!e.target.isContentEditable||!!e.target.closest?.('input,textarea,select,#sketch')||e.composedPath().some(n=>n?.hasAttribute?.('data-skin-ui'));
  if(type==='keydown'&&e.key==='Escape'&&document.querySelector('dialog[open]'))return;
  if(type==='keydown'&&sketch?.visible&&(e.ctrlKey||e.metaKey)&&!e.target.closest?.('input,textarea,[contenteditable=true]')&&['z','y'].includes(e.key.toLowerCase())){e.preventDefault();void sketch.send({kind:e.key.toLowerCase()==='y'||e.shiftKey?'redo':'undo'}).catch(error);return}
  const commands=keyboard.handle(e,editing);if(commands.length)e.preventDefault();
  for(const c of commands){if(c.ui==='help'){if($('help').open)$('help').close();else $('help-open').click()}else if(c.kind==='release_modes')void t.release(c.source);else{if(c.kind==='mode'||c.kind==='hold_mode')t.dropContinuous();fire(c,'Keyboard')}}
});
// Skins remain a presentation-only boundary; no engine or performance IPC changes.
const skinManager=new SkinManager(browserStorage(),message=>{$('skin-status').textContent=message;$('skin-status').title=message;if(/rejected|unsafe|not saved|unavailable/i.test(message))error(message)});
try{await skinManager.initialize(await invoke('skin_startup'))}catch(e){error('Skin fallback: '+e.message);skinManager.reset()}
const skinPanel=new SkinPanel(skinManager);
workspace=new Workspace({getState:()=>state,run,release,sketch,skinManager,onError:error,openDialog});
visuals=new MusicalOverview({t,getState:()=>state,workspace,sketch,skinManager,onError:error,run});
dialogs=new Dialogs(release);
instrument=new Instrument({t,getState:()=>state,run,sketch,workspace,visuals,skinManager,invoke,openDialog,ensureOutput,onError:error});
content.beforeReplace=async()=>{if(state.decks)await run({kind:'stop'})};
content.afterReplace=async()=>{await sketch.waitContent();if(automaticAudio){await ensureOutput();await run({kind:'transport',action:'Pause'})}};
bind('skins-open',()=>{$('settings').close();skinPanel.refresh();openDialog('skins')});
document.addEventListener('dawj-skin-applied',drawTimeline);
function show(s){
  if((s.control_revision??0)<(state.control_revision??0))return;
  const old=address(state),next=address(s);if(old.deck!==next.deck||old.slot!==next.slot){t.latest.delete('selected-gain');t.latest.delete('selected-cutoff')}
  state=s;const mode=s.selection?.mode??'Volume';
  if(lastMode!==mode){t.dropContinuous();lastMode=mode}
  document.querySelectorAll('[data-live]').forEach(e=>e.disabled=!s.online);
  $('start').disabled=!!s.decks||!$('device').options.length;$('stop').disabled=!s.decks;
  for(const id of ['device','rescan','routing','phrase'])$(id).disabled=!!s.decks;
  $('clock').textContent=s.online?`${s.transport} · ${Math.floor(s.beat/3840)+1}.${Math.floor(s.beat/960)%4+1} · ${s.bpm?.toFixed(1)} BPM`:(s.failed?'Output FAILED':'Output offline');
  if(s.bpm!=null)setValue('bpm',s.bpm);if(s.selection)setValue('quantize',s.selection.quantization);
  $('volume').setAttribute('aria-pressed',mode==='Volume');$('filter').setAttribute('aria-pressed',mode==='Filter');
  $('mode-status').textContent=`Part sliders: ${mode==='Filter'?'low-pass':'volume'}${s.selection?.temporary?' (held)':''}`;
  for(let deck=0;deck<2;deck++){
    const composition=s.session?.project.compositions.find(c=>c.id===s.session.project.decks[deck]);$('deck-'+deck).querySelector('h2').textContent=`${deck?'B':'A'} | ${s.session?.demo?(deck?'Glass Tide':'Copper Steps'):(composition?.name??'Empty deck')}`;
    $('deck-select-'+deck).setAttribute('aria-pressed',s.selection?.deck===deck);$('deck-select-'+deck).disabled=!s.online;
    setValue('deck-gain-'+deck,s.deck_gain?.[deck]??1);$('cue-'+deck).disabled=!s.online||!s.cue_available;$('cue-'+deck).setAttribute('aria-pressed',s.cue?.[deck]??false);
    for(let slot=0;slot<8;slot++){
      const p=s.decks?.[deck]?.[slot],v=slotView(s,deck,slot),card=$(`slot-${deck}-${slot}`);card.dataset.active=v.active;card.dataset.selected=v.selected;card.dataset.queued=!!v.pending;
      card.querySelector('.part-label').textContent=`${deck?'B':'A'}${slot+1} ${p?.name??composition?.parts[slot]?.name??`Part ${slot+1}`}`;
      card.querySelector('circle').setAttribute('fill',composition?.parts[slot]?.color??'currentColor');card.dataset.sketchEditing=!!sketch?.visible&&sketch.deck===deck&&sketch.slot===slot;
      card.querySelector('.identity').disabled=false;card.querySelector('[data-action=Launch]').disabled=!s.online||p?.available===false;
      card.querySelector('.identity').setAttribute('aria-pressed',v.selected);
      const unavailable=s.session?.media.find(m=>m.deck===deck&&m.slot===slot)?.error;const label=(unavailable?'UNAVAILABLE':!p?(composition?.parts[slot]?.factory||composition?.parts[slot]?.media?'LOADED | output closed':'EMPTY'):p.available===false?'EMPTY':s.transport==='Paused'&&v.active?'PAUSED':v.state)+(v.pending?` @ ${Math.floor(v.pending.tick/3840)+1}.${Math.floor(v.pending.tick/960)%4+1}`:'');card.querySelector('.state').textContent=label;card.querySelector('.state').title=label+(unavailable?' | '+unavailable:'')+' · '+v.feedback;
      card.querySelector('[data-action=Cancel]').disabled=!s.online||!v.pending;card.querySelector('[data-action=Retrigger]').disabled=!s.online||!v.active;
      const control=card.querySelector('.context');if(!dragging.has(control))control.value=contextValue(p,mode);control.setAttribute('aria-label',`${deck?'B':'A'}${slot+1} ${mode}`);control.title=mode==='Volume'?`Gain ${(p?.gain??1).toFixed(2)}`:`Cutoff ${p?.cutoff_hz==null?'bypassed':Math.round(p.cutoff_hz)+' Hz'}`;
    }
  }
  const selected=address(s),part=s.decks?.[selected.deck]?.[selected.slot];
  $('selected-name').textContent=`${selected.deck?'B':'A'}${selected.slot+1} ${part?.name??''}`;
  setValue('part-gain',part?.gain??1);setValue('part-cutoff',contextValue(part,'Filter'));
  for(const id of ['part-gain','part-cutoff','bypass'])$(id).disabled=!s.online||part?.available===false;
  $('part-gain').closest('label').firstChild.textContent=`Volume ${Math.round((part?.gain??1)*100)}% `;$('part-cutoff').closest('label').firstChild.textContent=`Low-pass ${part?.cutoff_hz==null?'off':Math.round(part.cutoff_hz)+' Hz'} `;$('inspection').hidden=workspace?.layout!=='extended';
  $('launch-selected').disabled=!s.online||part?.available===false;
  $('cancel-selected').disabled=!s.online||!part?.pending;$('retrigger-selected').disabled=!s.online||part?.active==null;
  $('rule-toggle').disabled=!s.online||!part?.rule||part.rule.consumed;$('rule-remove').disabled=!s.online||!part?.rule;
  $('rule-toggle').textContent=part?.rule?.next!=null?'Pause rule':'Resume rule';
  $('rule-status').textContent=part?.rule?`${part.rule.consumed?'Spent':part.rule.next==null?'Paused':'Next tick '+part.rule.next} · fired ${part.rule.fired} / skipped ${part.rule.skipped}`:'No rule';
  $('inspection').textContent=`Volume ${Math.round((part?.gain??1)*100)}% · Low-pass ${part?.cutoff_hz==null?'off':Math.round(part.cutoff_hz)+' Hz'} · ${part?.pattern.running?'Pattern playing':part?.active!=null?'Sample playing':part?.available?'Loaded / stopped':'Choose a sound'}`+(workspace?.layout==='extended'?' · '+(s.timeline?.inspector??'')+' · '+(s.feedback?.[selected.deck]?.[selected.slot]??''):'');$('inspection').title=$('inspection').textContent;
  setValue('cross',s.crossfader??.5);setValue('trim',s.output_trim_db??36);$('trim-value').textContent=s.output_trim_db??36;
  $('cue-status').textContent=s.cue_available?'Master 1–2 / cue 3–4; physical isolation unverified':'Stereo master only · no isolated headphone cue';
  for(const [i,channel]of ['l','r'].entries()){$('meter-'+channel).value=s.peaks?.[i]??0;$('peak-'+channel).textContent=meterDb(s.peaks?.[i]??0)}
  $('error').textContent=errorText;
  const diagnostic=workspace?.layout==='extended'&&!sketch?.visible,contentMessage=s.session?.message??'';
  $('notice').textContent=s.failed?'Audio stream failed. Close output, rescan, and start again.':diagnostic?(s.notice??'')+(contentMessage?' | '+contentMessage:''):s.session?.busy?'Preparing sounds...':contentMessage.startsWith('Content unchanged:')?contentMessage:(s.notice&&!/^Command #/.test(s.notice)?s.notice+' | ':'')+(s.session?.dirty?'Unsaved session changes.':'Ready.');
  $('midi-badge').textContent=`MIDI ${s.midi?.input??'no input'} · ${s.midi_notice??''}`;
  for(const deck of [0,1])$('fill-'+(deck?'b':'a')).disabled=!s.online||s.decks?.[deck]?.[7]?.available===false;
  for(const deck of [0,1])$('cancel-fill-'+(deck?'b':'a')).disabled=!s.online||!s.decks?.[deck]?.[7]?.pending;
  if(s.timeline){setValue('zoom',s.timeline.zoom_bars);$('follow').checked=s.timeline.follow;$('phrase-position').textContent=s.timeline.phrase_position.map((p,i)=>`${i?'B':'A'} ${p[0]}/${p[1]}`).join(' · ')}
  if(!s.decks&&s.session&&!s.session.demo){setValue('bpm',s.session.project.settings.bpm);setValue('phrase',s.session.project.settings.phrase_bars)}
  workspace?.update(s);visuals?.update(s);drawTimeline();midi?.update(s);content?.update(s);sketch?.update(s);instrument?.update(s);
}
function drawTimeline(){
  if($('phrase-panel').hidden)return;
  const canvas=$('timeline'),r=canvas.getBoundingClientRect(),scale=devicePixelRatio||1;
  if(canvas.width!==Math.round(r.width*scale)||canvas.height!==Math.round(r.height*scale)){canvas.width=Math.round(r.width*scale);canvas.height=Math.round(r.height*scale)}
  const ctx=canvas.getContext('2d');ctx.setTransform(scale,0,0,scale,0,0);ctx.clearRect(0,0,r.width,r.height);
  const colors=getComputedStyle(document.body),fg=colors.color,a=colors.getPropertyValue('--accent'),b=colors.getPropertyValue('--b');ctx.font=`${Math.max(11,12*parseFloat(colors.getPropertyValue('--font-scale')||'1'))}px ${colors.fontFamily}`;ctx.fillStyle=fg;
  const projection=state.timeline,left=116,right=r.width-12,laneHeight=(r.height-28)/3;hits=timelineHits(projection,r.width,r.height,address(state));
  if(!projection){ctx.fillText('Loaded material | start output for musical position',12,16);for(let d=0;d<2;d++){const c=state.session?.project.compositions.find(c=>c.id===state.session.project.decks[d]);ctx.fillStyle=d?b:a;ctx.fillText(`${d?'B':'A'} | ${c?.name??'Empty deck'}`,12,42+d*24)}return}
  // Loaded-material bands are metadata, never invented waveforms or scheduled events.
  for(let d=0;d<2;d++){const c=state.session?.project.compositions.find(c=>c.id===state.session.project.decks[d]);const parts=c?.parts.filter(p=>p.factory||p.media)??[];if(parts.length){ctx.fillStyle=d?b:a;ctx.globalAlpha=.12;ctx.fillRect(left,28+d*laneHeight,right-left,laneHeight-2);ctx.globalAlpha=1;ctx.fillStyle=fg;ctx.fillText(`${parts.length} loaded | ${parts.filter(p=>p.pattern).length} patterns`,left+5,28+d*laneHeight+12)}}
  for(const line of projection.grid){const x=left+line.fraction*(right-left);ctx.strokeStyle=colors.getPropertyValue('--color-grid');ctx.globalAlpha=line.bar!=null?1:.35;ctx.beginPath();ctx.moveTo(x,20);ctx.lineTo(x,r.height);ctx.stroke();ctx.globalAlpha=1;if(line.bar!=null){ctx.fillStyle=fg;ctx.fillText(String(line.bar),x+2,13)}}
  for(let lane=0;lane<3;lane++){ctx.fillStyle=lane===0?a:lane===1?b:fg;ctx.fillText(lane<2?`${lane?'B':'A'} · ${projection.activity[lane]} playing`:`${address(state).deck?'B':'A'}${address(state).slot+1} selected`,3,28+(lane+.5)*laneHeight+4)}
  for(const m of hits){const width=m.width??24;ctx.fillStyle=m.deck===0?a:b;ctx.globalAlpha=m.projected?.55:1;ctx.fillRect(m.x-width/2,m.y-9,width,18);ctx.fillStyle=colors.getPropertyValue('--color-on-primary');ctx.fillText(m.label,m.x-width/2+2,m.y+4);ctx.globalAlpha=1}
  if(projection.playhead!=null){const x=left+projection.playhead*(right-left);ctx.strokeStyle=fg;ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(x,18);ctx.lineTo(x,r.height);ctx.stroke();ctx.lineWidth=1}
  const signature=JSON.stringify(projection.markers.map(m=>[m.deck,m.slot,m.tick,m.kind,m.projected]));
  if(signature!==markerSignature){markerSignature=signature;$('marker-list').replaceChildren();const initial=document.createElement('option');initial.value=-1;initial.textContent='Scheduled actions';$('marker-list').append(initial);projection.markers.forEach((m,i)=>{const e=document.createElement('option');e.value=i;e.textContent=`${m.deck?'B':'A'}${m.slot+1} ${m.label} @ ${m.tick}`;$('marker-list').append(e)})}
}
window.addEventListener('resize',drawTimeline);
setInterval(()=>{void t.flush()},20);
const reliability=await invoke('reliability_options');
const jam=await invoke('jam_options');
const isSmoke=await invoke('smoke_enabled'),testAutoStartup=jam?.milestone==='ux1'&&jam.phase==='reopen';automaticAudio=testAutoStartup||!jam&&!isSmoke&&!reliability.seconds;
try{await scan();show(await t.call({kind:'snapshot'}));if(testAutoStartup){const matches=devices.filter(d=>d.name.toLowerCase().includes('hyperx'));if(matches.length!==1||skinManager.committed.audio?.device!==matches[0].name)throw Error('UX1 startup requires the saved unique HyperX output; no fallback.')}if(automaticAudio)await instrument.initializeAudio()}catch(e){error(e);if(automaticAudio)openDialog('audio-settings')}
if(jam?.milestone==='ux1'){const {runUx1Smoke}=await import('./ux1-smoke.mjs');await runUx1Smoke({t,getState:()=>state,invoke,sketch,workspace,visuals,instrument,skinManager,ensureOutput,options:jam})}
else if(jam?.milestone==='4'){const {runJam4Smoke}=await import('./jam4-smoke.mjs');await runJam4Smoke({t,getState:()=>state,invoke,sketch,workspace,visuals,skinManager,options:jam})}
else if(jam?.milestone==='3'){const {runJam3Smoke}=await import('./jam3-smoke.mjs');await runJam3Smoke({t,getState:()=>state,invoke,sketch,workspace,skinManager,options:jam})}
else if(jam?.milestone==='2'){const {runJam2Smoke}=await import('./jam2-smoke.mjs');await runJam2Smoke({t,getState:()=>state,invoke,sketch,options:jam})}
else if(jam){const {runJamSmoke}=await import('./jam-smoke.mjs');await runJamSmoke({t,getState:()=>state,invoke,content,options:jam})}
else if(reliability.seconds){const {runReliability}=await import('./reliability.mjs');await runReliability({t,getState:()=>state,invoke,skinManager,options:reliability})}
else if(isSmoke){const {runSmoke}=await import('./smoke.mjs');await runSmoke({t,getState:()=>state,invoke,show,skinManager,workspace})}
async function poll(){try{await t.call({kind:'snapshot'})}catch(e){error(e)}setTimeout(poll,33)}poll();
