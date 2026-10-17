export type Word = "ready";
export type Digit = 7;
export type RecordShape = { readonly title: string; count: number };
export interface Face { readonly title: string; count: number }
export declare const inline: { readonly title: string; count: number };
export declare const literal: Word;
export declare const digit: Digit;
export declare const shape: RecordShape;
export declare const face: Face;
export interface OtherFace { title: number }
export interface EscapedFace { \u0074itle: string }
