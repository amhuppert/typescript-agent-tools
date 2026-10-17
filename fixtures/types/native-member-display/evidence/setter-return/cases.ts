export type Bodyless = { set /*bodyless*/value(next: number); };
export type Paired = { get value(): string; set /*paired*/value(next: Missing); };
export class Implemented { set /*implemented*/value(next: number) {} }
