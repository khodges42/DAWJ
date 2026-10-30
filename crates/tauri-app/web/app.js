// SPDX-License-Identifier: GPL-3.0-or-later
import {names,action,label,cross,summary,isActive} from './model.mjs';
import {invokeRequest} from './protocol.mjs';
const invoke=window.__TAURI__.core.invoke,$=id=>document.getElementById(id);let state={},serial=Promise.resolve(),times=[],pendingCross=null,sendingCross=false,admitted=0,lastError='';
// Bounded frontend admission complements the independent Rust IPC gate.
function call(a){if(admitted>=32)return Promise.reject(Error('Frontend queue full'));admitted++;const run=async()=>{const t=performance.now();try{return a.kind==='report'?await invoke('report',{metrics:a.metrics}):await invokeRequest(invoke,a)}finally{admitted--;times.push(performance.now()-t);if(times.length>1000)times.shift()}};const p=serial.then(run);serial=p.catch(()=>{});return p}
function show(s){state=s;$('clock').textContent=s.online?`${s.transport} | Bar ${Math.floor(s.beat/3840)+1}.${Math.floor(s.beat/960)%4+1} | ${s.bpm} BPM | frame ${s.frame}`:'Output offline';for(let d=0;d<2;d++)for(let i=0;i<8;i++){const p=s.decks?.[d]?.[i],el=$(`s${d}${i}`);el.dataset.active=isActive(p);el.querySelector('p').textContent=label(p)+(p?.pending?` @ tick ${p.pending.tick}`:'');el.querySelectorAll('button').forEach(b=>b.disabled=!s.online)}$('notice').textContent=(lastError?lastError+' | ':'')+(s.failed?'STREAM FAILED - stop and restart output. ':s.notice??'')+' | Stereo master only; cue unavailable.'}
for(let d=0;d<2;d++){const deck=document.createElement('section');deck.className='deck';const h=document.createElement('h2');h.textContent=`Deck ${d?'B - Glass Tide':'A - Copper Steps'}`;deck.append(h);const slots=document.createElement('div');slots.className='slots';for(let i=0;i<8;i++){const el=document.createElement('article');el.id=`s${d}${i}`;const title=document.createElement('strong');title.textContent=`${d?'B':'A'}${i+1} ${names[i]}`;el.append(title,document.createElement('p'));for(const [kind,text] of [['launch','Launch'],['stop_part','Stop']]){const b=document.createElement('button');b.textContent=text;b.onclick=()=>call(action(kind,d,i)).then(show).catch(error);el.append(b)}slots.append(el)}deck.append(slots);$('decks').append(deck)}
function error(e){lastError=String(e);$('notice').textContent=lastError}
$('start').onclick=()=>call({kind:'start',device:Number($('device').value)}).then(show).catch(error);$('stop').onclick=()=>call({kind:'stop'}).then(()=>call({kind:'snapshot'})).then(show).catch(error);
$('cross').oninput=e=>pendingCross=cross(e.target.value);
setInterval(()=>{if(pendingCross&&!sendingCross){const a=pendingCross;pendingCross=null;sendingCross=true;call(a).then(show).catch(error).finally(()=>sendingCross=false)}},33);
$('theme').value=localStorage.theme??'dark';document.body.dataset.theme=$('theme').value;$('theme').onchange=e=>{document.body.dataset.theme=e.target.value;localStorage.theme=e.target.value};
$('accent').value=localStorage.accent??'#73c5df';
document.documentElement.style.setProperty('--accent',$('accent').value);
$('accent').oninput=e=>{document.documentElement.style.setProperty('--accent',e.target.value);localStorage.accent=e.target.value};
$('user-style').textContent=localStorage.customCSS??'';$('css').onchange=async e=>{const f=e.target.files[0];if(!f)return;if(f.size>65536)return error('CSS limit 64 KiB');const css=await f.text();$('user-style').textContent=css;localStorage.customCSS=css};
$('reset-style').onclick=()=>{localStorage.removeItem('customCSS');$('user-style').textContent=''};
try {for(const d of await call({kind:'devices'})){const o=document.createElement('option');o.value=d.id;o.textContent=d.name;$('device').append(o)}}catch(e){error(e)}
const initial=await call({kind:'snapshot'});initial.smoke=await invoke('smoke_enabled');show(initial);
if (initial.smoke) {
  try {
    const savedTheme=document.body.dataset.theme;
    const themeBackgrounds=[];
    for (const theme of ['dark','light','high-contrast']) {
      document.body.dataset.theme=theme;
      themeBackgrounds.push(getComputedStyle(document.body).backgroundColor);
    }
    document.body.dataset.theme=savedTheme;
    if (new Set(themeBackgrounds).size!==3) throw Error('Theme styles failed to load');
    const controlsFit=[...document.querySelectorAll('article button'),$('cross')].every(el=>{
      const r=el.getBoundingClientRect();
      return r.left>=0 && r.right<=innerWidth && r.bottom<=innerHeight;
    });
    show(await call({kind:'start', device:0}));
    const queued = await call(action('launch',0,0));
    await call(action('launch',1,3));
    const started = performance.now();
    let sawQueued = queued.decks[0][0].pending !== null;
    while (performance.now()-started < 2000) {
      show(await call({kind:'snapshot'}));
      sawQueued ||= state.decks[0][0].pending !== null;
      if (isActive(state.decks[0][0]) && isActive(state.decks[1][3])) break;
      await new Promise(r=>setTimeout(r,20));
    }
    if (!isActive(state.decks[0][0]) || !isActive(state.decks[1][3])) throw Error('Engine did not start both parts');
    const observed = performance.now()-started;
    show(await call(cross(.7)));
    if (Math.abs(state.crossfader-.7)>0.000001) throw Error('Crossfader was not accepted');
    times=[];
    for (let i=0;i<100;i++) await call({kind:'snapshot'});
    const metrics={ipc_ms:summary(times),launch_response_to_active_ms:observed,audio_peaks:state.peaks,saw_queued:sawQueued,themes_distinct:true,controls_fit:controlsFit,viewport:[innerWidth,innerHeight]};
    for (const invalid of [{kind:'launch',deck:2,slot:0},{kind:'crossfader',value:2}]) {
      let rejected=false;
      try { await call(invalid); } catch { rejected=true; }
      if (!rejected) throw Error('Invalid command accepted');
    }
    await call(action('stop_part',0,0));
    const stopping=performance.now();
    while (performance.now()-stopping<2000) {
      show(await call({kind:'snapshot'}));
      if (!isActive(state.decks[0][0]) && state.decks[0][0].pending===null) break;
      await new Promise(r=>setTimeout(r,20));
    }
    if (isActive(state.decks[0][0]) || !isActive(state.decks[1][3])) throw Error('Independent stop failed');
    metrics.independent_stop=true;
    await call({kind:'stop'});
    if ((await call({kind:'snapshot'})).online) throw Error('Shutdown still online');
    metrics.clean_audio_shutdown=true;
    $('metrics').textContent=JSON.stringify(metrics);
    await call({kind:'report',metrics});
  } catch (e) {
    await call({kind:'stop'}).catch(()=>{});
    await call({kind:'report',metrics:{error:String(e)}});
    error(e);
  }
}
async function poll(){try{show(await call({kind:'snapshot'}))}catch(e){error(e)}setTimeout(poll,33)}poll();
