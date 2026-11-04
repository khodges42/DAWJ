// SPDX-License-Identifier: GPL-3.0-or-later
// Manual metadata editing only. Rust validates musical positions and persists the document.
export const SECTION_LABELS=['Intro','Groove','Build','Drop','Breakdown','Fill','Outro'];
export function emphasisOptions(value){return [0,.2,.5,.8,1].includes(value)||value==null?[]:[{value:String(value),label:`Custom (${value})`}]}
export function sectionTick(bar,beat){return (Number(bar)-1)*3840+(Number(beat)-1)*960}
export function editSection(structure,kind,section){
 const next=structuredClone(structure??{bars:8,sections:[],show_labels:true});
 if(kind==='remove')next.sections=next.sections.filter(s=>s.id!==section.id);
 else if(kind==='upsert'){const index=next.sections.findIndex(s=>s.id===section.id);if(index<0)next.sections.push(section);else next.sections[index]=section;next.sections.sort((a,b)=>a.tick-b.tick)}
 else throw Error('Unknown section operation');return next;
}
export class StructureEditor {
 constructor({sketch,getState,run,onError}){
  Object.assign(this,{sketch,getState,run,onError});this.selected=null;this.signature='';
  const host=document.createElement('fieldset');host.id='structure-editor';
  host.innerHTML='<legend>Musical sections</legend><div class="bar"><label>Bars <input id="structure-bars" type="number" min="1" max="64" value="8"></label><label><input id="structure-labels" type="checkbox" checked> Show labels</label><button id="structure-apply">Apply length / labels</button></div><div id="section-list" role="list" aria-label="Composition sections"></div><div class="bar"><select id="section-preset" aria-label="Section label preset"></select><input id="section-label" maxlength="48" placeholder="Custom section label" aria-label="Section label"><label>Bar <input id="section-bar" type="number" min="1" max="64" value="1"></label><label>Beat <input id="section-beat" type="number" min="1" max="4.75" step=".25" value="1"></label><label>Emphasis <select id="section-intensity"><option value="">Source energy</option><option value="0">Minimal</option><option value="0.2">Quiet</option><option value="0.5">Medium</option><option value="0.8">Strong</option><option value="1">Intense</option></select></label></div><div class="bar"><button id="section-add">Add section</button><button id="section-update">Update selected</button><button id="section-remove">Remove selected</button></div><small>Labels are your musical meaning. Energy is a source estimate, not automatic Intro/Drop detection. Sections do not schedule parts.</small>';
  document.querySelector('#sketch .sketch-editor').append(host);this.host=host;const $=id=>document.getElementById(id);this.$=$;
  for(const label of [...SECTION_LABELS,'Custom'])$('section-preset').add(new Option(label,label));$('section-label').value='Intro';
  $('section-preset').onchange=()=>{if($('section-preset').value!=='Custom')$('section-label').value=$('section-preset').value};
  const bind=(id,fn)=>$(id).onclick=()=>Promise.resolve().then(fn).catch(onError);
  bind('section-add',()=>this.change('upsert',true));bind('section-update',()=>this.change('upsert',false));bind('section-remove',()=>this.change('remove',false));
  bind('structure-apply',()=>this.send({...this.composition().structure,bars:Number($('structure-bars').value),show_labels:$('structure-labels').checked}));
 }
 composition(){const w=this.getState().session;return w?.project.compositions.find(c=>c.id===w.project.decks[this.sketch.deck])}
 send(structure){const c=this.composition();if(!c)throw Error('Create a composition on this deck first');return this.run({kind:'session',action:{kind:'structure',composition:c.id,structure}})}
 change(kind,add){const c=this.composition();if(!c)throw Error('Create a composition first');if(!add&&this.selected==null)throw Error('Select a section first');const $=this.$,id=add?Math.max(0,...c.structure.sections.map(s=>s.id))+1:this.selected;
  const section={id,tick:sectionTick($('section-bar').value,$('section-beat').value),label:$('section-label').value,intensity:$('section-intensity').value===''?null:Number($('section-intensity').value)};return this.send(editSection(c.structure,kind,section));
 }
 update(s){if(!this.sketch.visible)return;const c=this.composition(),signature=JSON.stringify([c?.id,c?.structure,s.session?.busy]);if(signature===this.signature)return;this.signature=signature;const $=this.$;
  for(const e of this.host.querySelectorAll('button,input,select'))e.disabled=!c||s.session.busy;
  if(!c){$('section-list').replaceChildren();return}if(this.compositionId!==c.id){this.selected=null;this.compositionId=c.id}
  if(document.activeElement!==$('structure-bars'))$('structure-bars').value=c.structure.bars;$('structure-labels').checked=c.structure.show_labels;
  $('section-list').replaceChildren();for(const section of c.structure.sections){const b=document.createElement('button');b.textContent=`${section.label} | ${Math.floor(section.tick/3840)+1}.${(section.tick%3840)/960+1}`;b.setAttribute('aria-pressed',this.selected===section.id);b.onclick=()=>{this.selected=section.id;$('section-label').value=section.label;$('section-preset').value=SECTION_LABELS.includes(section.label)?section.label:'Custom';$('section-bar').value=Math.floor(section.tick/3840)+1;$('section-beat').value=(section.tick%3840)/960+1;const intensity=$('section-intensity');for(const option of [...intensity.options])if(option.dataset.custom)option.remove();for(const option of emphasisOptions(section.intensity)){const custom=new Option(option.label,option.value);custom.dataset.custom='true';intensity.add(custom)}intensity.value=section.intensity??'';this.signature='';this.update(this.getState())};$('section-list').append(b)}
  const exists=c.structure.sections.some(s=>s.id===this.selected);$('section-remove').disabled=!exists||s.session.busy;$('section-update').disabled=!exists||s.session.busy;
 }
}
