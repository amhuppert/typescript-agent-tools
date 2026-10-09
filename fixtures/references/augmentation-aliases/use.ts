// Consumers reach the merged declarations through every route.
import type { /*@Commands:i*/Commands as /*@Commands:i*/C } from "./commands";
import type { /*@Options:i*/Options } from "./options";
import type { /*@Renamed:i*/Renamed } from "./renamed";
import type { /*@Base:i*/Base, /*@Widget:i*/Widget, /*@Starred:i*/Starred } from "kit";
import { /*@Shape:i*/Shape } from "./shape";
import { /*@ModeValue,ModeType:i*/Mode } from "./mode";
export declare const /*@u:de*/u: /*@Commands:u*/C<string>;
export const /*@s:de*/s: string = /*@u:u*/u./*@!*/a + /*@u:u*/u./*@!*/b + /*@u:u*/u./*@!*/base;
export declare const /*@o:de*/o: /*@Options:u*/Options;
export declare const /*@r:de*/r: /*@Renamed:u*/Renamed<number>;
export declare const /*@bw:de*/bw: /*@Base:u*/Base & /*@Widget:u*/Widget & /*@Starred:u*/Starred<number>;
export const /*@shape:de*/shape = new /*@Shape:u*/Shape();
export const /*@mode:de*/mode: /*@ModeType:u*/Mode = { /*@!*/m: /*@ModeValue:u*/Mode };
