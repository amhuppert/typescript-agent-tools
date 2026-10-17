export {};
interface Healthy { /*#seed*/probe: string; }
declare const healthy: Healthy;
healthy./*#healthy*/probe;
interface Indexed { [name: string]: number; }
declare const indexed: Indexed;
indexed./*#indexed*/probe;
interface Multi { [name: `p${string}`]: number; [name: `${string}e`]: 1; }
declare const multi: Multi;
multi./*#multi*/probe;
declare const unrestricted: any;
unrestricted./*#any*/probe;
declare const base: any;
class AnyDerived extends base {}
declare const anyDerived: AnyDerived;
anyDerived./*#anyBase*/probe;
