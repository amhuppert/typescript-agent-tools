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
interface InterfaceA { first: number; second: string }
interface InterfaceB { first: boolean; second: number }
class ClassA { first!: number; second!: string }
class ClassB { first!: boolean; second!: number }
type AliasA = { first: number; second: string };
type AliasB = { first: boolean; second: number };
function interfaceCase() {
  let seed!: keyof InterfaceA;
  const interfaceChosen = (seed, undefined as unknown as (keyof InterfaceB));
}
function reverseCase() {
  let seed!: keyof InterfaceB;
  const reverseChosen = (seed, undefined as unknown as (keyof InterfaceA));
}
function classCase() {
  let seed!: keyof ClassA;
  const classChosen = (seed, undefined as unknown as (keyof ClassB));
}
function aliasCase() {
  let seed!: keyof AliasA;
  const aliasChosen = (seed, undefined as unknown as (keyof AliasB));
}
function plainFirstCase() {
  let seed!: "first" | "second";
  const plainFirstChosen = (seed, undefined as unknown as (keyof InterfaceB));
}
function indexFirstCase() {
  let seed!: keyof InterfaceA;
  const indexFirstChosen = (seed, undefined as unknown as ("first" | "second"));
}
