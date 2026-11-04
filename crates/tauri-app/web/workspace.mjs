// SPDX-License-Identifier: GPL-3.0-or-later
// Presentation and command routing only. All musical positions come from Rust snapshots.
import {address,partAction} from './performance.mjs';
export const WORKSPACES=['browse','performance','extended'];
export function workspace(value){if(!WORKSPACES.includes(value))throw Error('Unknown workspace');return value}
export function partPresentation(s,deck,slot){
 const c=s.session?.project.compositions.find(c=>c.id===s.session.project.decks[deck]),p=c?.parts[slot],live=s.decks?.[deck]?.[slot],q=live?.pattern;
 const loaded=!!(p?.factory||p?.media||s.session?.demo&&live?.available),active=live?.active!=null;
 const pending=!!q&&(q.target_revision!==q.revision||q.pending_revision!=null);
 const progress=q?.running&&q.step!=null&&q.bars?q.step/(q.bars*16):active&&live.phase!=null&&live.nominal_length?live.phase/live.nominal_length:0;
 return {loaded,active,pending,progress:Math.min(1,Math.max(0,progress)),
  source:p?.factory?.path.split('/').at(-1)??p?.media?.split(/[\\/]/).at(-1)??(s.session?.demo?'Demo sound':'No sound assigned'),
  length:p?.pattern?`${p.pattern.bars} bar pattern`:p?.looping?`${(p.nominal_ticks/960).toFixed(0)} beats · loop`:'One shot',
  variation:!p?.pattern?'Sample playback':p.variation==null?'Edited pattern':`Variation ${p.variation+1}`,
  edit:pending?(q.pending_at!=null?`Edit queued @ ${musicalPosition(q.pending_at)}`:'Edit awaiting engine'):'',
  steps:p?.pattern?.steps.slice(0,p.pattern.bars).flat()??[],step:q?.running?q.step:null};
}
export function musicalPosition(tick){return `${Math.floor(tick/3840)+1}.${Math.floor(tick/960)%4+1}`}
export function commandIntent(id,s){
 if(id==='edit.undo'||id==='edit.redo')return {kind:'session',action:{kind:id.slice(5)}};
 if(id==='deck.a'||id==='deck.b')return {kind:'deck',deck:id==='deck.a'?0:1};
 if(id==='mixer.center')return {kind:'crossfader',value:.5};
 if(id==='deck.launch'||id==='deck.stop'||id==='deck.cancel'||id==='deck.retrigger')return partAction(address(s).deck,address(s).slot,{launch:'Launch',stop:'Stop',cancel:'Cancel',retrigger:'Retrigger'}[id.slice(5)]);
 return null;
}
export const COMMANDS=[
 ['file.new','File / Session','New empty session','Ctrl+N','jam-new'],['file.open','File / Session','Open session…','Ctrl+O','jam-open'],['file.save','File / Session','Save session','Ctrl+S','jam-save'],['file.save-as','File / Session','Save session as…','Ctrl+Shift+S','jam-save-as'],['file.session','File / Session','Session and composition assignments','','session-open'],
 ['edit.undo','Edit','Undo','Ctrl+Z'],['edit.redo','Edit','Redo','Ctrl+Shift+Z'],['view.browse','View','Simple / Browse','Ctrl+1'],['view.performance','View','Performance','Ctrl+2'],['view.extended','View','Extended','Ctrl+3'],['view.sketch','View','Sketch / Perform','Ctrl+E'],
 ['deck.a','Deck','Focus Deck A','A'],['deck.b','Deck','Focus Deck B','B'],['deck.launch','Deck','Launch focused part','Space'],['deck.stop','Deck','Stop focused part','X'],['deck.cancel','Deck','Cancel queued action','Esc'],['deck.retrigger','Deck','Retrigger focused part','R'],
 ['mixer.center','Mixer','Center crossfader'],['mixer.audio','Mixer','Output and cue routing','','audio-open'],['sounds.browse','Sounds / Library','Browse sounds and compositions'],['sounds.edit','Sounds / Library','Edit focused part'],
 ['tools.audio','Tools / Settings','Audio settings','','audio-open'],['tools.midi','Tools / Settings','MIDI configuration','','midi-open'],['tools.skins','Tools / Settings','Skins and accessibility','','skins-open'],['help.keys','Help','Performance shortcuts','','help-open']
].map(([id,group,label,shortcut='',button])=>({id,group,label,shortcut,button}));
export function searchCommands(query){const terms=query.trim().toLowerCase().split(/\s+/);return COMMANDS.filter(c=>terms.every(t=>(c.group+' '+c.label+' '+c.shortcut).toLowerCase().includes(t)))}
export function shortcutCommand(e){
 if(e.type!=='keydown'||e.repeat||!(e.ctrlKey||e.metaKey)||e.altKey)return null;
 const k=e.key.toLowerCase();return ({k:'palette',s:e.shiftKey?'file.save-as':'file.save',o:'file.open',n:'file.new',e:'view.sketch','1':'view.browse','2':'view.performance','3':'view.extended',z:e.shiftKey?'edit.redo':'edit.undo',y:'edit.redo'})[k]??null;
}
export class Workspace {
 constructor({getState,run,release,sketch,skinManager,onError,openDialog}){
  Object.assign(this,{getState,run,release,sketch,skinManager,onError,openDialog});this.layout=skinManager.committed.workspace;this.offlineFocus={deck:0,slot:0};this.$=id=>document.getElementById(id);this.library=sketch.$('sketch').querySelector('.sound-browser');this.libraryDock=document.createElement('section');this.libraryDock.id='library-dock';this.libraryDock.setAttribute('aria-label','Sound browser');this.compositionLibrary=document.createElement('aside');this.compositionLibrary.id='composition-library';this.compositionLibrary.setAttribute('aria-label','Composition library');
  const $=this.$;for(const d of [0,1]){
   const label=$('deck-gain-'+d).closest('label');label.firstChild.textContent='Volume';$('channel-'+d).append(label,$('cue-'+d));
   const tools=document.createElement('div');tools.className='deck-tools bar';const compositions=document.createElement('select');compositions.id='deck-composition-'+d;compositions.setAttribute('aria-label',`Composition on Deck ${d?'B':'A'}`);compositions.onchange=()=>{void run({kind:'session',action:{kind:'assign',deck:d,composition:compositions.value===''?null:Number(compositions.value)}}).catch(onError)};
   const edit=document.createElement('button');edit.textContent='Sketch';edit.setAttribute('aria-label',`Edit composition on Deck ${d?'B':'A'}`);edit.onclick=()=>{void this.edit(d,this.offlineFocus.deck===d?this.offlineFocus.slot:0).catch(onError)};
   tools.append(compositions,edit,$('fill-'+(d?'b':'a')),$('cancel-fill-'+(d?'b':'a')));
   for(const [kind,action] of [['play','Launch'],['stop','Stop']]){const b=document.createElement('button');b.id=`browse-${kind}-${d}`;b.className='browse-only';b.dataset.live='';b.textContent=kind==='play'?'Launch focused':'Stop focused';b.onclick=()=>{const s=this.getState(),slot=s.selection?.slots[d]??(this.offlineFocus.deck===d?this.offlineFocus.slot:0);void this.run(partAction(d,slot,action)).catch(this.onError)};tools.append(b)}$('deck-'+d).querySelector('header').after(tools);
   const overview=document.createElement('div');overview.className='deck-overview';overview.id='deck-overview-'+d;overview.setAttribute('aria-label',`Deck ${d?'B':'A'} loaded and playing material`);tools.after(overview);const phase=document.createElement('small');phase.id='deck-phase-'+d;phase.className='deck-phase';overview.before(phase);
   for(let slot=0;slot<8;slot++){
    const card=$(`slot-${d}-${slot}`),meta=document.createElement('small');meta.className='part-source';card.querySelector('.identity').after(meta);
    const detail=document.createElement('small');detail.className='part-detail';card.append(detail);
    const position=document.createElement('progress');position.className='part-position';position.max=1;position.value=0;position.setAttribute('aria-label',`${d?'B':'A'}${slot+1} musical position`);card.append(position);
   }
  }
  $('decks').replaceChildren($('deck-0'),$('mixer'),$('deck-1'),$('sketch'),this.libraryDock,this.compositionLibrary);
  for(const id of WORKSPACES)$('workspace-'+id).onclick=()=>{void this.setLayout(id).catch(onError)};
  $('cross-center').onclick=()=>{void this.execute('mixer.center').catch(onError)};
  $('audio-open').onclick=()=>openDialog('audio-settings');$('command-open').onclick=()=>this.palette();$('edit-focused').onclick=()=>{void this.editFocused().catch(onError)};
  for(const store of [true,false])$('variation-'+(store?'store':'recall')).onclick=()=>{void this.variation(store).catch(onError)};
  this.buildMenus();$('command-search').oninput=()=>this.results();$('command-search').onkeydown=e=>{if(e.key==='ArrowDown'||e.key==='Enter'){e.preventDefault();$('command-results').querySelector('button:not(:disabled)')?.focus()}};
  $('command-results').onkeydown=e=>{if(!['ArrowDown','ArrowUp','Home','End'].includes(e.key))return;const buttons=[...$('command-results').querySelectorAll('button:not(:disabled)')],i=buttons.indexOf(document.activeElement);e.preventDefault();buttons[e.key==='Home'?0:e.key==='End'?buttons.length-1:(i+(e.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length]?.focus()};
  document.addEventListener('click',e=>{if(!e.target.closest('#application-menu'))this.closeMenus()});document.addEventListener('keydown',e=>{if(e.key==='Escape')this.closeMenus()});
  this.renderLayout();if(this.layout==='browse')void this.sketch.loadCatalog(false).catch(onError);
 }
 focused(){return this.getState().selection?address(this.getState()):this.offlineFocus}
 select(deck,slot){this.offlineFocus={deck,slot};this.sketch.select(deck,slot)}
 async edit(deck,slot){if(this.getState().online)await this.run({kind:'select_part',deck,slot});this.select(deck,slot);if(!this.sketch.visible)await this.toggleSketch();this.sketch.select(deck,slot);this.$('sketch-name').focus()}
 async editFocused(){const p=this.focused();return this.edit(p.deck,p.slot)}
 async setLayout(value){this.release();const next=workspace(value);this.skinManager.setWorkspace(next);this.layout=next;if(this.sketch.visible)await this.sketch.toggle();this.renderLayout();if(value==='browse')await this.sketch.loadCatalog(false);this.update(this.getState());this.$('workspace-'+value).focus()}
 async toggleSketch(){this.release();if(!this.sketch.visible)this.$('sketch').prepend(this.library);await this.sketch.toggle();this.renderLayout();this.update(this.getState())}
 renderLayout(){const $=this.$;document.body.dataset.workspace=this.layout;document.body.dataset.context=this.sketch.visible?'sketch':'perform';for(const value of WORKSPACES)$('workspace-'+value).setAttribute('aria-pressed',!this.sketch.visible&&this.layout===value);
  const browse=this.layout==='browse'&&!this.sketch.visible;this.libraryDock.hidden=!browse;this.compositionLibrary.hidden=!browse;for(const slots of document.querySelectorAll('.deck .slots'))slots.hidden=browse;for(const e of document.querySelectorAll('.browse-only'))e.hidden=!browse;if(browse)this.libraryDock.append(this.library);else if(this.sketch.visible)$('sketch').prepend(this.library);
  for(const e of document.querySelectorAll('.extended-only,.part-detail,.context'))e.hidden=this.sketch.visible||this.layout!=='extended';
  for(const e of document.querySelectorAll('.deck .actions,.deck .part-source,.deck .part-position'))e.hidden=browse||this.sketch.visible;
  $('phrase-panel').hidden=this.sketch.visible||this.layout!=='extended';$('inspector').hidden=this.sketch.visible||this.layout==='browse';
 }
 enabled(c){const s=this.getState();if(c.id==='edit.undo')return !!s.undo&&!s.session?.busy;if(c.id==='edit.redo')return !!s.redo&&!s.session?.busy;if(c.id.startsWith('deck.')||c.id==='mixer.center')return !!s.online;return !s.session?.busy||!c.id.startsWith('file.')}
 buildMenus(){const nav=this.$('application-menu');for(const group of [...new Set(COMMANDS.map(c=>c.group))]){const details=document.createElement('details'),summary=document.createElement('summary'),list=document.createElement('div');summary.textContent=group.split(' / ')[0];list.className='menu-actions';details.append(summary,list);details.addEventListener('toggle',()=>{if(details.open){this.release();for(const other of nav.querySelectorAll('details'))if(other!==details)other.open=false;for(const b of list.querySelectorAll('button'))b.disabled=!this.enabled(COMMANDS.find(c=>c.id===b.dataset.command))}});for(const c of COMMANDS.filter(c=>c.group===group))list.append(this.commandButton(c));nav.append(details)}}
 closeMenus(){for(const e of this.$('application-menu').querySelectorAll('details'))e.open=false}
 commandButton(c){const b=document.createElement('button');b.textContent=c.label+(c.shortcut?'  ·  '+c.shortcut:'');b.dataset.command=c.id;b.disabled=!this.enabled(c);b.onclick=()=>{void this.execute(c.id).catch(this.onError)};return b}
 palette(){this.release();this.closeMenus();this.openDialog('command-palette');this.$('command-search').value='';this.results();this.$('command-search').focus()}
 results(){this.$('command-results').replaceChildren(...searchCommands(this.$('command-search').value).map(c=>this.commandButton(c)))}
 async execute(id){this.release();this.closeMenus();this.$('command-palette').close();if(id==='palette')return this.palette();const c=COMMANDS.find(c=>c.id===id);if(!c)throw Error('Unknown command');if(!this.enabled(c))throw Error('Command unavailable in the current state');
  const intent=commandIntent(id,this.getState());if(intent)return this.run(intent);
  if(c.button){this.$(c.button).click();return}
  if(id==='view.sketch')return this.toggleSketch();
  if(id.startsWith('view.'))return this.setLayout(id.slice(5));
  if(id==='sounds.browse')return this.setLayout('browse');if(id==='sounds.edit')return this.editFocused();
 }
 async variation(store){const f=this.focused(),c=this.getState().session.project.decks[f.deck];if(c==null)throw Error('Create a composition first');return this.run({kind:'session',action:{kind:'variation',composition:c,slot:f.slot,index:Number(this.$('variation-select').value),store}})}
 update(s){const $=this.$,w=s.session;if(!w)return;if(s.selection){this.offlineFocus=address(s);if(!this.sketch.visible)this.sketch.select(this.offlineFocus.deck,this.offlineFocus.slot);}$('session-title').textContent=w.project.name+(w.dirty?' · Unsaved':'');$('session-title').title=w.path??'Save As to choose a local session file';$('audition-status').textContent=s.audition!=null?'Audition active · shared master':'Audition plays through master';
  for(const d of [0,1]){const c=w.project.compositions.find(c=>c.id===w.project.decks[d]),select=$('deck-composition-'+d),signature=JSON.stringify(w.project.compositions.map(c=>[c.id,c.name]));if(select.dataset.signature!==signature){select.replaceChildren(new Option('Empty deck',''));for(const composition of w.project.compositions)select.add(new Option(composition.name,composition.id));select.dataset.signature=signature}if(document.activeElement!==select)select.value=c?.id??'';select.disabled=w.busy;
   const focusedSlot=s.selection?.slots[d]??(this.offlineFocus.deck===d?this.offlineFocus.slot:0);for(const kind of ['play','stop']){const b=$(`browse-${kind}-${d}`);b.textContent=(kind==='play'?'Launch ':'Stop ')+(d?'B':'A')+(focusedSlot+1);b.disabled=!s.online||s.decks?.[d]?.[focusedSlot]?.available===false;}
   const clock=s.musical_decks?.[d];$('deck-phase-'+d).textContent=clock?`Phrase ${Math.floor(clock.phase/3840)+1}/${clock.phrase_length/3840} | beat ${Math.floor(clock.phase/960)%4+1}`:'Loaded material | output closed';
   $('deck-meter-'+d).value=s.deck_peaks?.[d]??0;$('deck-level-'+d).textContent=(s.deck_peaks?.[d]??0)>0?(20*Math.log10(s.deck_peaks[d])).toFixed(1)+' dBFS':'−∞ dBFS';
   const overview=$('deck-overview-'+d);if(!overview.children.length)for(let slot=0;slot<8;slot++){const b=document.createElement('button');b.onclick=()=>{this.select(d,slot);if(this.getState().online)void this.run({kind:'select_part',deck:d,slot}).catch(this.onError)};overview.append(b)}
   for(let slot=0;slot<8;slot++){const p=partPresentation(s,d,slot),card=$(`slot-${d}-${slot}`),live=s.decks?.[d]?.[slot];card.dataset.loaded=p.loaded;card.dataset.pendingEdit=p.pending;card.querySelector('.part-source').textContent=p.source;card.querySelector('.part-source').title=p.source;card.querySelector('.part-detail').textContent=p.length+' · '+p.variation+(p.edit?' · '+p.edit:'')+(live?' · gain '+live.gain.toFixed(2)+(live.cutoff_hz==null?' · filter off':' · '+Math.round(live.cutoff_hz)+' Hz'):'');card.querySelector('.part-position').value=p.progress;const b=overview.children[slot];b.textContent=String(slot+1);b.dataset.loaded=p.loaded;b.dataset.active=p.active;b.dataset.pending=!!live?.pending||p.pending;b.setAttribute('aria-label',`${d?'B':'A'}${slot+1}: ${p.source}, ${p.active?'playing':p.loaded?'loaded':'empty'}${live?.pending?', '+live.pending.kind+' queued at '+musicalPosition(live.pending.tick):''}`);b.title=b.getAttribute('aria-label');}
  }
  const f=this.focused(),p=partPresentation(s,f.deck,f.slot);$('variation-status').textContent=p.edit||p.variation;
  if(!this.compositionLibrary.hidden){
   const signature=JSON.stringify([w.revision,this.focused(),p.source,p.variation]);
   if(this.compositionSignature!==signature){this.compositionSignature=signature;const title=document.createElement('h2');title.textContent='Compositions';const focused=document.createElement('p');focused.textContent=`Focused ${f.deck?'B':'A'}${f.slot+1} | ${p.source} | ${p.length}`;const edit=document.createElement('button');edit.textContent='Edit focused part';edit.onclick=()=>{void this.editFocused().catch(this.onError)};this.compositionLibrary.replaceChildren(title,focused,edit);
    for(const composition of w.project.compositions){const row=document.createElement('section'),name=document.createElement('strong'),info=document.createElement('small');name.textContent=composition.name;info.textContent=`${composition.parts.filter(p=>p.factory||p.media).length} sounds | ${composition.parts.filter(p=>p.pattern).length} patterns`;row.append(name,info);for(const deck of [0,1]){const b=document.createElement('button');b.textContent=`Load ${deck?'B':'A'}`;b.disabled=w.busy;b.onclick=()=>{void this.run({kind:'session',action:{kind:'assign',deck,composition:composition.id}}).catch(this.onError)};row.append(b)}this.compositionLibrary.append(row)}
    for(const deck of [0,1]){const b=document.createElement('button');b.textContent=`New on ${deck?'B':'A'}`;b.onclick=()=>{void (async()=>{await this.run({kind:'session',action:{kind:'create',deck,name:`Composition ${w.project.compositions.length+1}`}});await this.sketch.waitContent();await this.edit(deck,0)})().catch(this.onError)};this.compositionLibrary.append(b)}
   }
  }
  if(this.libraryDock.hidden===false&&this.browserRevision!==w.revision){this.browserRevision=w.revision;this.sketch.renderSounds()}
 }
}
