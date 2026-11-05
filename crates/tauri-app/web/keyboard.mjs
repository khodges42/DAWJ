// SPDX-License-Identifier: GPL-3.0-or-later
export class Keyboard {
  constructor(){this.held=new Map()}
  handle(e,editing=false){
    const key=e.key.toLowerCase();
    if(e.type==='keyup'){
      if(key==='shift'&&this.held.size){this.held.clear();return [{kind:'release_modes',source:'Keyboard'}]}
      const hold=this.held.get(key);if(hold){this.held.delete(key);return [{kind:'hold_mode',...hold,pressed:false}]}
      return [];
    }
    if(editing||e.ctrlKey||e.altKey||e.metaKey)return [];
    if(e.shiftKey){
      if(!e.repeat&&['v','f'].includes(key)&&!this.held.has(key)){
        const hold={token:key==='v'?1:2,mode:key==='v'?'Volume':'Filter'};this.held.set(key,hold);
        return [{kind:'hold_mode',...hold,pressed:true}];
      }
      return [];
    }
    if(e.repeat&&!['arrowleft','arrowright'].includes(key))return [];
    if(key==='h')return [{ui:'help'}];
    if(['a','b'].includes(key))return [{kind:'deck',deck:key==='a'?0:1}];
    if(/^[1-8]$/.test(key))return [{kind:'slot',slot:Number(key)-1}];
    const selected={' ':'Toggle',x:'Stop',escape:'Cancel',r:'Retrigger'};
    if(selected[key])return [{kind:'selected',action:selected[key]}];
    const grid={f1:'Immediate',f2:'Beat',f3:'Bar',f4:'Phrase'};
    if(grid[key])return [{kind:'quantize',grid:grid[key]}];
    if(['v','f'].includes(key))return [{kind:'mode',mode:key==='v'?'Volume':'Filter'}];
    if(['q','w'].includes(key))return [{kind:'cue',deck:key==='q'?0:1}];
    if(['arrowleft','arrowright'].includes(key))return [{kind:'crossfader_step',delta:key==='arrowleft'?-.05:.05}];
    return [];
  }
  release(){this.held.clear();return ['Keyboard','Gui','Midi'].map(source=>({kind:'release_modes',source}))}
}
