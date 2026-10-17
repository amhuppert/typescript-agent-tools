const \u0041lias = 1;
type Alias = { value: string };
declare const /*#valueFirst*/first: Alias;
type \u0042efore = { value: number };
const Before = 1;
declare const /*#aliasFirst*/second: Before;
function \u0046unctionAlias(): void {}
type FunctionAlias = { value: boolean };
declare const /*#functionFirst*/third: FunctionAlias;
type Plain = { value: string };
const \u0050lain = 1;
declare const /*#plainFirst*/fourth: Plain;
const \u0049face = 1;
interface Iface { value: string }
declare const /*#interfaceValueFirst*/ifaceFirst: Iface;
interface \u0042ack { value: number }
const Back = 1;
declare const /*#interfaceTypeFirst*/ifaceSecond: Back;
function \u0043allableNamed(): void {}
interface CallableNamed { value: boolean }
declare const /*#interfaceFunctionFirst*/ifaceThird: CallableNamed;
interface \u0043allableAfter { value: string }
function CallableAfter(): void {}
declare const /*#interfaceBeforeFunction*/ifaceFourth: CallableAfter;
namespace \u004doduleOnly { export interface Member {} }
type ModuleOnly = { value: number };
declare const /*#namespaceFirst*/namespaceFirst: ModuleOnly;
type \u004doduleBefore = { value: string };
namespace ModuleBefore { export interface Member {} }
declare const /*#namespaceAfter*/namespaceAfter: ModuleBefore;
interface \u0052ecursive {}
function Recursive(): typeof Recursive { throw 0; }
declare const /*#recursiveValue*/recursiveValue: typeof Recursive;
