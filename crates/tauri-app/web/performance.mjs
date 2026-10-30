// SPDX-License-Identifier: GPL-3.0-or-later
export function address(s){return {deck:s.selection?.deck??0,slot:s.selection?.slots[s.selection.deck]??0}}
export function gestureTarget(binding,s){const current=address(s);if(binding&&(binding.deck!==current.deck||binding.slot!==current.slot))throw Error('Selection changed during gesture; release and operate the selected control again.');return binding??current}
export function partAction(deck,slot,action){
  if(!Number.isInteger(deck)||deck<0||deck>1||!Number.isInteger(slot)||slot<0||slot>7)throw Error('Invalid part address');
  return {kind:'part',deck,slot,action};
}
export function contextValue(part,mode){return mode==='Filter'?(part?.cutoff_hz==null?1:Math.log(part.cutoff_hz/20)/Math.log(1000)):(part?.gain??1)}
export function contextAction(deck,slot,mode,value){
  value=Number(value);if(!Number.isFinite(value)||value<0||value>1)throw Error('Invalid parameter value');
  return partAction(deck,slot,mode==='Filter'?{Cutoff:value>=.9999?null:20*1000**value}:{Gain:value});
}
export function slotView(s,deck,slot){
  const p=s.decks?.[deck]?.[slot];return {
    state:p?.state??'UNAVAILABLE',active:p?.active!=null,pending:p?.pending??null,
    selected:s.selection?.deck===deck&&s.selection.slots[deck]===slot,disabled:!s.online,
    feedback:s.feedback?.[deck]?.[slot]??'',name:p?.name??'',
  };
}
export function meterDb(value){return value>0?`${(20*Math.log10(value)).toFixed(1)} dBFS`:'−∞ dBFS'}
// Canvas positioning only: all musical window/markers are already projected by Rust.
export function timelineHits(projection,width,height,selected){
  if(!projection)return [];const left=116,right=width-12,laneHeight=(height-28)/3;
  if(projection.groups)return projection.groups.map(group=>{const width=Math.min(100,Math.max(24,group.label.length*7+4));return {...projection.markers[group.members[0]],label:group.label,members:group.members,width,x:Math.max(left+width/2,Math.min(right-width/2,left+group.fraction*(right-left))),y:28+(group.lane+.5)*laneHeight}});
  return projection.markers.flatMap(m=>{
    const lanes=[m.deck];if(m.deck===selected.deck&&m.slot===selected.slot)lanes.push(2);
    return lanes.map(lane=>({...m,x:Math.max(left+12,Math.min(right-12,left+m.fraction*(right-left))),y:28+(lane+.5)*laneHeight}));
  });
}
export function nearestMarker(hits,x,y){return hits.filter(m=>Math.abs(m.x-x)<=(m.width??34)/2&&Math.abs(m.y-y)<=10).sort((a,b)=>Math.hypot(a.x-x,a.y-y)-Math.hypot(b.x-x,b.y-y))[0]}
