export type Loop = Loop;
export type First = Second;
export type Second = First;
export type Outer = First;
export type Healthy = { next: Healthy; value: string };
export let broken: Loop;
export let healthy: Healthy;
