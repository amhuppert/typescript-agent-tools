declare const Symbol: { readonly iterator: 'iter'; readonly asyncIterator: 'asyncIter' };
interface Iterable<T, TReturn = any, TNext = any> { iter(): Iterator<T, TReturn, TNext> }
interface Iterator<T, TReturn = any, TNext = any> { next(...args: [] | [TNext]): IteratorResult<T, TReturn>; return?(value?: TReturn): IteratorResult<T, TReturn> }
interface IterableIterator<T, TReturn = any, TNext = any> extends Iterator<T, TReturn, TNext> { iter(): IterableIterator<T, TReturn, TNext> }
interface IteratorObject<T, TReturn = unknown, TNext = unknown> extends Iterator<T, TReturn, TNext> { iter(): IteratorObject<T, TReturn, TNext> }
interface Generator<T = unknown, TReturn = any, TNext = any> extends Iterator<T, TReturn, TNext> { iter(): Generator<T, TReturn, TNext> }
interface ArrayIterator<T> extends IteratorObject<T, undefined, unknown> {}
interface IteratorYieldResult<T> { done?: false; value: T }
interface IteratorReturnResult<T> { done: true; value: T }
type IteratorResult<T, TReturn> = IteratorYieldResult<T> | IteratorReturnResult<TReturn>;
interface Array<T> { iter(): Iterator<T, undefined, unknown> }
interface ReadonlyArray<T> { iter(): Iterator<T, undefined, unknown> }
interface AsyncIterable<T, TReturn = any, TNext = any> { asyncIter(): AsyncIterator<T, TReturn, TNext> }
interface AsyncIterator<T, TReturn = any, TNext = any> { next(...args: [] | [TNext]): Promise<IteratorResult<T, TReturn>> }
interface AsyncIterableIterator<T, TReturn = any, TNext = any> extends AsyncIterator<T, TReturn, TNext> { asyncIter(): AsyncIterableIterator<T, TReturn, TNext> }
interface AsyncIteratorObject<T, TReturn = unknown, TNext = unknown> extends AsyncIterator<T, TReturn, TNext> { asyncIter(): AsyncIteratorObject<T, TReturn, TNext> }
interface AsyncGenerator<T = unknown, TReturn = any, TNext = any> extends AsyncIterator<T, TReturn, TNext> { asyncIter(): AsyncGenerator<T, TReturn, TNext> }
interface Promise<T> { then(onfulfilled: (value: T) => unknown): unknown }
type Awaited<T> = T extends Promise<infer U> ? Awaited<U> : T;
