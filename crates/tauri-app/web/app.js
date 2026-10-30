// SPDX-License-Identifier: GPL-3.0-or-later
import {Transport} from './transport.mjs';
import {Keyboard,HELP} from './keyboard.mjs';
import {address,gestureTarget,partAction,contextValue,contextAction,slotView,meterDb,timelineHits,nearestMarker} from './performance.mjs';
import {MidiPanel} from './midi.mjs';
const $=id=>document.getElementById(id),invoke=window.__TAURI__.core.invoke;
let state={online:false},errorText='',lastMode='Volume',hits=[],markerSignature='',dragging=new Map(),midi;
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
  const header=document.createElement('header'),title=document.createElement('h2');title.textContent=`${deck?'B · Glass Tide':'A · Copper Steps'}`;
  const select=button('Select',()=>fire({kind:'deck',deck}),`Select deck ${deck?'B':'A'}`);select.id='deck-select-'+deck;
  const gain=document.createElement('input');gain.id='deck-gain-'+deck;gain.type='range';gain.min=0;gain.max=1;gain.step=.005;gain.setAttribute('aria-label',`Deck ${deck?'B':'A'} gain`);gain.dataset.live='';
  gain.oninput=()=>t.coalesce('deck-'+deck,{kind:'deck_gain',deck,value:Number(gain.value)});
  const cue=button('Cue',()=>fire({kind:'cue',deck}),`Toggle cue ${deck?'B':'A'}`);cue.id='cue-'+deck;cue.dataset.live='';const gainLabel=document.createElement('label');gainLabel.textContent='Gain';gainLabel.append(gain);header.append(title,select,gainLabel,cue);section.append(header);
  const slots=document.createElement('div');slots.className='slots';
  for(let slot=0;slot<8;slot++){
    const card=document.createElement('article');card.id=`slot-${deck}-${slot}`;
    const identity=button(`${deck?'B':'A'}${slot+1}`,()=>fire({kind:'select_part',deck,slot}));identity.className='identity';identity.id=`select-${deck}-${slot}`;identity.dataset.live='';
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
document.querySelectorAll('input[type=range]:not(.context)').forEach(e=>{e.addEventListener('pointerdown',()=>dragging.set(e,{target:address(state)}));for(const event of ['pointerup','pointercancel','lostpointercapture'])e.addEventListener(event,()=>dragging.delete(e))});
midi=new MidiPanel(t,error);
async function scan(){
  const list=await run({kind:'devices'});$('device').replaceChildren();for(const device of list){const option=document.createElement('option');option.value=device.id;option.textContent=device.name;$('device').append(option)}
  if(!list.length)error('No audio output device. Connect one and Rescan.');
}
bind('rescan',scan);
bind('start',()=>run({kind:'start',device:Number($('device').value),cue:$('routing').value==='cue',phrase_bars:Number($('phrase').value)}));
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
function release(){dragging.clear();for(const c of keyboard.release())void t.release(c.source)}
window.addEventListener('blur',release);document.addEventListener('visibilitychange',()=>{if(document.hidden)release()});
function openDialog(id){release();if(!$(id).open)$(id).showModal()}
bind('help-open',()=>openDialog('help'));$('bindings').textContent=HELP;
bind('midi-open',async()=>{openDialog('settings');await midi.open()});
document.querySelectorAll('[data-close]').forEach(e=>e.onclick=()=>$(e.dataset.close).close());
for(const type of ['keydown','keyup'])window.addEventListener(type,e=>{
  const editing=!!e.target.isContentEditable||!!e.target.closest?.('input,textarea,select');
  if(type==='keydown'&&e.key==='Escape'&&document.querySelector('dialog[open]'))return;
  const commands=keyboard.handle(e,editing);if(commands.length)e.preventDefault();
  for(const c of commands){if(c.ui==='help'){if($('help').open)$('help').close();else openDialog('help')}else if(c.kind==='release_modes')void t.release(c.source);else{if(c.kind==='mode'||c.kind==='hold_mode')t.dropContinuous();fire(c,'Keyboard')}}
});
// Preserve the experiment's existing appearance controls, without extending its theme system.
$('theme').value=localStorage.theme??'dark';document.body.dataset.theme=$('theme').value;$('theme').onchange=e=>{document.body.dataset.theme=e.target.value;localStorage.theme=e.target.value};
$('accent').value=localStorage.accent??'#73c5df';document.documentElement.style.setProperty('--accent',$('accent').value);$('accent').oninput=e=>{document.documentElement.style.setProperty('--accent',e.target.value);localStorage.accent=e.target.value};
$('user-style').textContent=localStorage.customCSS??'';$('css').onchange=async e=>{const file=e.target.files[0];if(!file)return;if(file.size>65536)return error('CSS limit 64 KiB');const css=await file.text();$('user-style').textContent=css;localStorage.customCSS=css};
$('reset-style').onclick=()=>{localStorage.removeItem('customCSS');$('user-style').textContent=''};
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
  $('mode-status').textContent=`${mode.toUpperCase()}${s.selection?.temporary?' (held; base '+s.selection.persistent_mode.toUpperCase()+')':' (persistent)'}`;
  for(let deck=0;deck<2;deck++){
    $('deck-select-'+deck).setAttribute('aria-pressed',s.selection?.deck===deck);$('deck-select-'+deck).disabled=!s.online;
    setValue('deck-gain-'+deck,s.deck_gain?.[deck]??1);$('cue-'+deck).disabled=!s.online||!s.cue_available;$('cue-'+deck).setAttribute('aria-pressed',s.cue?.[deck]??false);
    for(let slot=0;slot<8;slot++){
      const p=s.decks?.[deck]?.[slot],v=slotView(s,deck,slot),card=$(`slot-${deck}-${slot}`);card.dataset.active=v.active;card.dataset.selected=v.selected;
      card.querySelector('.identity').textContent=`${deck?'B':'A'}${slot+1} ${p?.name??['kick','snare','hats','bass','chord','arp','shaker','fill'][slot]}`;
      card.querySelector('.identity').setAttribute('aria-pressed',v.selected);
      const label=v.state+(v.pending?` @ ${Math.floor(v.pending.tick/3840)+1}.${Math.floor(v.pending.tick/960)%4+1}`:'');card.querySelector('.state').textContent=label;card.querySelector('.state').title=label+' · '+v.feedback;
      card.querySelector('[data-action=Cancel]').disabled=!s.online||!v.pending;card.querySelector('[data-action=Retrigger]').disabled=!s.online||!v.active;
      const control=card.querySelector('.context');if(!dragging.has(control))control.value=contextValue(p,mode);control.setAttribute('aria-label',`${deck?'B':'A'}${slot+1} ${mode}`);control.title=mode==='Volume'?`Gain ${(p?.gain??1).toFixed(2)}`:`Cutoff ${p?.cutoff_hz==null?'bypassed':Math.round(p.cutoff_hz)+' Hz'}`;
    }
  }
  const selected=address(s),part=s.decks?.[selected.deck]?.[selected.slot];
  $('selected-name').textContent=`${selected.deck?'B':'A'}${selected.slot+1} ${part?.name??''}`;
  setValue('part-gain',part?.gain??1);setValue('part-cutoff',contextValue(part,'Filter'));
  $('cancel-selected').disabled=!s.online||!part?.pending;$('retrigger-selected').disabled=!s.online||part?.active==null;
  $('rule-toggle').disabled=!s.online||!part?.rule||part.rule.consumed;$('rule-remove').disabled=!s.online||!part?.rule;
  $('rule-toggle').textContent=part?.rule?.next!=null?'Pause rule':'Resume rule';
  $('rule-status').textContent=part?.rule?`${part.rule.consumed?'Spent':part.rule.next==null?'Paused':'Next tick '+part.rule.next} · fired ${part.rule.fired} / skipped ${part.rule.skipped}`:'No rule';
  $('inspection').textContent=`Gain ${(part?.gain??1).toFixed(2)} · Filter ${part?.cutoff_hz==null?'bypassed':Math.round(part.cutoff_hz)+' Hz'} · `+(s.timeline?.inspector??'No engine publication')+' · '+(s.feedback?.[selected.deck]?.[selected.slot]??'');$('inspection').title=$('inspection').textContent;
  setValue('cross',s.crossfader??.5);setValue('trim',s.output_trim_db??36);$('trim-value').textContent=s.output_trim_db??36;
  $('cue-status').textContent=s.cue_available?'Master 1–2 / cue 3–4; physical isolation unverified':'Stereo master only · no isolated headphone cue';
  for(const [i,channel]of ['l','r'].entries()){$('meter-'+channel).value=s.peaks?.[i]??0;$('peak-'+channel).textContent=meterDb(s.peaks?.[i]??0)}
  $('error').textContent=errorText;$('notice').textContent=s.failed?'Audio stream failed. Stop output, rescan, and start a fresh session.':s.notice??'';
  $('midi-badge').textContent=`MIDI ${s.midi?.input??'no input'} · ${s.midi_notice??''}`;
  for(const deck of [0,1])$('cancel-fill-'+(deck?'b':'a')).disabled=!s.online||!s.decks?.[deck]?.[7]?.pending;
  if(s.timeline){setValue('zoom',s.timeline.zoom_bars);$('follow').checked=s.timeline.follow;$('phrase-position').textContent=s.timeline.phrase_position.map((p,i)=>`${i?'B':'A'} ${p[0]}/${p[1]}`).join(' · ')}
  drawTimeline();midi?.update(s);
}
function drawTimeline(){
  const canvas=$('timeline'),r=canvas.getBoundingClientRect(),scale=devicePixelRatio||1;
  if(canvas.width!==Math.round(r.width*scale)||canvas.height!==Math.round(r.height*scale)){canvas.width=Math.round(r.width*scale);canvas.height=Math.round(r.height*scale)}
  const ctx=canvas.getContext('2d');ctx.setTransform(scale,0,0,scale,0,0);ctx.clearRect(0,0,r.width,r.height);
  const colors=getComputedStyle(document.body),fg=colors.color,a=colors.getPropertyValue('--accent'),b=colors.getPropertyValue('--b');ctx.font='12px system-ui';ctx.fillStyle=fg;
  const projection=state.timeline,left=116,right=r.width-12,laneHeight=(r.height-28)/3;hits=timelineHits(projection,r.width,r.height,address(state));
  if(!projection){ctx.fillText('Start output for the repeating musical timeline',12,36);return}
  for(const line of projection.grid){const x=left+line.fraction*(right-left);ctx.strokeStyle=line.bar!=null?'#718096':'#495565';ctx.beginPath();ctx.moveTo(x,20);ctx.lineTo(x,r.height);ctx.stroke();if(line.bar!=null){ctx.fillStyle=fg;ctx.fillText(String(line.bar),x+2,13)}}
  for(let lane=0;lane<3;lane++){ctx.fillStyle=lane===0?a:lane===1?b:fg;ctx.fillText(lane<2?`${lane?'B':'A'} · ${projection.activity[lane]} playing`:`${address(state).deck?'B':'A'}${address(state).slot+1} selected`,3,28+(lane+.5)*laneHeight+4)}
  for(const m of hits){const width=m.width??24;ctx.fillStyle=m.deck===0?a:b;ctx.globalAlpha=m.projected?.55:1;ctx.fillRect(m.x-width/2,m.y-9,width,18);ctx.fillStyle=colors.getPropertyValue('--bg');ctx.fillText(m.label,m.x-width/2+2,m.y+4);ctx.globalAlpha=1}
  if(projection.playhead!=null){const x=left+projection.playhead*(right-left);ctx.strokeStyle=fg;ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(x,18);ctx.lineTo(x,r.height);ctx.stroke();ctx.lineWidth=1}
  const signature=JSON.stringify(projection.markers.map(m=>[m.deck,m.slot,m.tick,m.kind,m.projected]));
  if(signature!==markerSignature){markerSignature=signature;$('marker-list').replaceChildren();const initial=document.createElement('option');initial.value=-1;initial.textContent='Scheduled actions';$('marker-list').append(initial);projection.markers.forEach((m,i)=>{const e=document.createElement('option');e.value=i;e.textContent=`${m.deck?'B':'A'}${m.slot+1} ${m.label} @ ${m.tick}`;$('marker-list').append(e)})}
}
window.addEventListener('resize',drawTimeline);
setInterval(()=>{void t.flush()},20);
try{await scan();show(await t.call({kind:'snapshot'}))}catch(e){error(e)}
if(await invoke('smoke_enabled')){const {runSmoke}=await import('./smoke.mjs');await runSmoke({t,getState:()=>state,invoke,show})}
async function poll(){try{await t.call({kind:'snapshot'})}catch(e){error(e)}setTimeout(poll,33)}poll();
