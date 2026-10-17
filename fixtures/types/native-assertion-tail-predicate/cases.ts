export {};
declare function broken(value: unknown): asserts value is MissingTailPredicate;
declare function broken(value: never): asserts value is never;
function statement(value: unknown) { broken(value/*#statement*/); }
function returned(value: unknown) { return broken(value/*#returned*/); }
