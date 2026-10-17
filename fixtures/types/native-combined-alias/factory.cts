export interface Point { x: 11; }
export type Code = { code: 12 };
export class Preferred { tag: 13 = 13; }
declare const factory: { Point: new () => Point; Code: () => 14; Preferred: MissingUnusedProperty };
export = factory;
