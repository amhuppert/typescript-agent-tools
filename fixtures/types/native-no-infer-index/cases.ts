export {};
declare const /*#objectKeys*/objectKeys: keyof NoInfer<{ alpha: 1; 7: 2 }>;
declare const /*#unionKeys*/unionKeys: keyof NoInfer<{ shared: 1; left: 2 } | { shared: 3; right: 4 }>;
declare const /*#intersectionKeys*/intersectionKeys: keyof NoInfer<{ alpha: 1 } & { beta: 2 }>;
declare const /*#anyKeys*/anyKeys: keyof NoInfer<any>;
declare const /*#unknownKeys*/unknownKeys: keyof NoInfer<unknown>;
declare const /*#neverKeys*/neverKeys: keyof NoInfer<never>;
declare const /*#emptyKeys*/emptyKeys: keyof NoInfer<{}>;
declare const token: unique symbol;
declare const /*#symbolKeys*/symbolKeys: keyof NoInfer<{ [token]: 1 }>;
declare const /*#unusedMember*/unusedMember: keyof NoInfer<{ present: MissingUnusedMember }>;
function generic<T>(key: keyof NoInfer<T>) {
    const /*#genericKeys*/selected = key;
    return selected;
}
declare function choose<T>(value: T, key: keyof NoInfer<T>): 1;
declare function choose(value: unknown, key: unknown): 2;
const /*#chosen*/chosen = choose({ alpha: 1 }, 'alpha');
const /*#rejected*/rejected = choose({ alpha: 1 }, 'beta');
type FlowKeys<T> = T extends { alpha: unknown } ? keyof T : 'none';
declare const /*#ordinaryTrue*/ordinaryTrue: FlowKeys<{ alpha: 1; beta: 2 }>;
declare const /*#ordinaryFalse*/ordinaryFalse: FlowKeys<{ other: 1 }>;
