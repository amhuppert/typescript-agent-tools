import type { importedKey } from "./names";
declare const key: "stable";
type Property = { [/*#property*/key]: number };
type Method = { [/*#method*/key](): number };
interface Shape { [/*#interface*/key]: number }
class Example { [/*#class*/key]: number = 0; }
const object = { [/*#object*/key]: 0 };
type Binding = ({ [/*#binding*/key]: value }: { stable: number }) => void;
type Initialized = { value: string = /*#initializer*/key };
type Parameter = (value: string = /*#parameter*/key) => void;
type Query = typeof /*#query*/key;
type Nested = { [(/*#nestedExpression*/key as typeof /*#nestedQuery*/key)]: number };
type ImportedProperty = { [/*#importedExpression*/importedKey]: number };
type ImportedQuery = typeof /*#importedQuery*/importedKey;
type MissingProperty = { [/*#missingComputed*/MissingComputedAttachment]: number };
type MissingInitializer = { value: number = /*#missingInitializer*/MissingInitializerAttachment };
