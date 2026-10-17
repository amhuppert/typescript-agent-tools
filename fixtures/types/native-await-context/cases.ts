export {};
declare function identity<T>(value: T): T;
export const inferred = await identity({ /*#absent*/value: 1 });
const annotated: { value: number } = await identity({ /*#annotation*/value: 1 });
async function returning(): Promise<{ value: number }> {
    return await identity({ /*#return*/value: 1 });
}
interface Recursive { then(onfulfilled: (value: Recursive) => unknown): unknown; }
const recursive: Recursive = await identity({ /*#nil*/value: 1 });
