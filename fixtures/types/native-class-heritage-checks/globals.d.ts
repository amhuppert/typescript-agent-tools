interface Object {}
interface Function {}
interface Array<T> { length: number; [n: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }
interface String { readonly length: number }
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface Symbol {}
interface SymbolConstructor { readonly iterator: unique symbol; }
declare const Symbol: SymbolConstructor;

interface ClassDecoratorContext<T = abstract new (...args: any) => any> { kind: "class"; name: string | undefined; addInitializer(initializer: (this: T) => void): void; metadata: unknown; }
declare module "tslib" { export const __setFunctionName: MissingUnusedHelperType; export const __propKey: MissingUnusedHelperType; export const __esDecorate: MissingUnusedHelperType; export const __runInitializers: MissingUnusedHelperType; export const __decorate: MissingUnusedHelperType; }
