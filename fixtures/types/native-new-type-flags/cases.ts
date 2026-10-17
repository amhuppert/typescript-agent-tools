export function createBox<T>(value: T) { return { value }; }
export function consumeBox<T>(box: { value: T }): T { return box.value; }
export const /*#nested*/nested = consumeBox(createBox("text"));
export const /*#direct*/direct = createBox("text").value;
export function acceptShape<T extends { value: string }>(value: T): T { return value; }
export const /*#constrained*/constrained = acceptShape(createBox("text")).value;
export function createRunner<T>(value: T) { return { run() { return value; } }; }
export const /*#method*/method = createRunner("text").run();
export const box = createBox("text");
export declare const /*#noInferKeys*/noInferKeys: keyof NoInfer<typeof box>;
export function createEmpty<T>(value: T) { return {}; }
export const empty = createEmpty("text");
export declare const /*#emptyKeys*/emptyKeys: keyof NoInfer<typeof empty>;
export const runner = createRunner("text");
export declare const /*#runnerKeys*/runnerKeys: keyof NoInfer<typeof runner>;
export type Copy<T> = { [K in keyof T]: T[K] };
export declare const /*#mappedKeys*/mappedKeys: keyof NoInfer<Copy<typeof box>>;
