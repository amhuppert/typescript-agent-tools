export declare function accept(value: number): number;
export declare function outer(value: number): number;
export declare function inner(value: number): number;
export declare function hiddenReturn(value: number): MissingReturn;
export declare function hiddenParameter(value: MissingParameter): number;
export declare function wrongThis(this: number, value: number): number;
export declare function generic<T>(value: T): T;
export declare class Made { constructor(value: number); }
export declare function tag(value: TemplateStringsArray): string;
