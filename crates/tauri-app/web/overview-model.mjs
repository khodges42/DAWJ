// SPDX-License-Identifier: GPL-3.0-or-later
// Pixel/material bookkeeping only. Rust supplies musical geometry and authoritative cursors.
export const VISUALIZATIONS=['structure','waveform','hybrid'];
export function visualization(value){if(!VISUALIZATIONS.includes(value))throw Error('Unknown visualization');return value}
export function visibleParts(s){return [0,1].flatMap(deck=>s.session?.project.decks[deck]!=null?Array.from({length:8},(_,slot)=>({deck,slot})):[])}
export function resolution(width,dpr=1){return Math.max(32,Math.min(1024,Math.ceil(width*Math.min(4,dpr)/64)*64))}
export function requestKey(s,style,columns,detail){return JSON.stringify([s.overview_revision,visualization(style),columns,detail,visibleParts(s)])}
export function overlayKey(s){return JSON.stringify([s.generation,s.frame,s.online,s.failed,s.selection])}
export class MaterialCache {
 constructor(){this.key='';this.data=null;this.pending=false;this.control=-1;this.revision='';this.builds=0;this.dropped=0}
 snapshot(s){if((s.control_revision??0)<this.control)return false;this.control=s.control_revision??0;this.revision=s.overview_revision??'';return true}
 accept(reply,key,currentKey){if(key!==currentKey||reply.revision!==this.revision){this.dropped++;return false}if(reply.data){this.data=reply.data;this.builds++}this.key=key;return true}
}
export function materialSummary(deck){if(deck.composition==null)return deck.notice??'Empty deck';return `${deck.name}: ${deck.structure.bars} bars; `+deck.structure.sections.map(s=>`${s.label} at bar ${Math.floor(s.tick/3840)+1}`).join('; ')+'; '+deck.parts.map(p=>`${p.slot+1} ${p.name}, ${p.kind}, ${p.period_ticks?p.period_ticks/3840+' bar pattern':p.source_seconds.toFixed(2)+' second source'}${p.error?', '+p.error:''}`).join('; ')}
export function frameFraction(part,cursor,source){if(cursor?.source==null||!source?.frames)return null;return Math.max(0,Math.min(1,(part.source_start+cursor.source*(part.source_end-part.source_start))/source.frames))}
export function loopFit(seconds,bpm,nominalTicks){const beats=seconds*bpm/60,expected=nominalTicks/960;return Math.abs(beats-expected)>.05?`${beats.toFixed(2)} natural beats differ from the ${expected.toFixed(2)} beat reference. No stretching.`:`Fits the ${expected.toFixed(2)} beat reference at current tempo.`}
