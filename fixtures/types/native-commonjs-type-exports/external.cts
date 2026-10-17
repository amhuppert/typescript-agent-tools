export interface Shape { size: 11; }
export type Code = 12;
export class Preferred { tag: 13 = 13; }
export enum State { Ready = 14 }
export namespace Bag { export const tag = 15; }
export const onlyValue = 999;
declare const factory: { Shape: new () => Shape; Code: () => 12; Preferred: MissingUnusedClass; State: MissingUnusedEnum; Bag: MissingUnusedNamespace; onlyValue: 16 };
export = factory;
