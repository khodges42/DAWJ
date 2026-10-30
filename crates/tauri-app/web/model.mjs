// SPDX-License-Identifier: GPL-3.0-or-later
export const names=['kick','snare','hats','bass','chord','arp','shaker','fill'];
export function action(kind,deck,slot){if(!['launch','stop_part'].includes(kind)||!Number.isInteger(deck)||deck<0||deck>1||!Number.isInteger(slot)||slot<0||slot>7)throw Error('Invalid action');return {kind,deck,slot}}
export function label(slot){return slot?.state??'UNAVAILABLE'}
export function cross(value){value=Number(value);if(!Number.isFinite(value)||value<0||value>1)throw Error('Invalid crossfader');return {kind:'crossfader',value}}
export function summary(samples){const a=[...samples].sort((x,y)=>x-y);return {n:a.length,p50:a[Math.floor(a.length*.5)],p95:a[Math.min(a.length-1,Math.floor(a.length*.95))],max:a.at(-1)}}
export function isActive(slot){return slot?.active !== null && slot?.active !== undefined}
