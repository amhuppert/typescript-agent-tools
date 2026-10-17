export declare let /*#property*/ property: { value?: string };
export declare let /*#method*/ method: { value?(): number };
export declare let /*#accessor*/ accessor: { get value(): string; set value(next: number); };
export declare let /*#ignoredBadSetter*/ ignoredBadSetter: { get value(): string; set value(next: Missing); };
export type Empty = {};
export declare let /*#emptyIdentities*/ emptyIdentities: (first: {}, second: {}, named: Empty) => Empty;
