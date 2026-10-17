export {};
interface Shape { /*@field*/probe: 11; }
declare const shape: Shape;
shape./*@access*/probe;
shape[/*@literalAccess*/"probe"];
type Indexed = Shape[/*@indexedType*/"probe"];
const { /*@bindingProperty*/probe: renamed } = shape;
const /*#bindingValue*/bindingValue = renamed;
function predicate(probe: unknown): /*@predicate*/probe is Shape { return true; }
namespace probe { export type Item = 12; }
type NamespaceItem = /*@namespacePrefix*/probe.Item;
type probe = Shape;
type Named = /*@typeReference*/probe;
const ordinary = /*@ordinaryLiteral*/"probe";
const /*#surrogateValue*/surrogateValue = /*@surrogateLiteral*/"\uD800";
const /*#templateSurrogate*/templateSurrogate = /*@surrogateTemplate*/`\uD800`;

class Owner {
    /*@privateDeclaration*/#field = 13 as const;
    read() { return this./*@privateRead*/#field; }
    has(value: object) { return /*@privateIn*/#field in value; }
}
declare const owner: Owner;
const /*#privateValue*/privateValue = owner.read();
const /*#privateMembership*/privateMembership = owner.has({});

namespace Parts { export namespace Inner { export class Value { field: 14 = 14; } } }
import Pick = /*@importHead*/Parts./*@importMiddle*/Inner./*@importTail*/Value;
const /*#importValue*/importValue = new Pick().field;

declare const lateName: "member";
declare class Merged { static [lateName]: 41; }
declare namespace Merged { export var member: 41; }
namespace Merged {
    export function read() { return /*@lateVariable*/member; }
    export function readArrow() { return () => /*@lateClosure*/member; }
}
const /*#lateVariable*/lateVariable = Merged.read();
const /*#lateClosure*/lateClosure = Merged.readArrow()();

declare namespace Duplicate { export var value: 42; }
declare namespace Duplicate { export var value: 42; }
namespace Duplicate { export function read() { return /*@mergedVariable*/value; } }
const /*#mergedVariable*/mergedVariable = Duplicate.read();

class Keywords { /*@constructorKeyword*/constructor() {} }
const keywordFunction = /*@functionKeyword*/function () { return 1; };
const keywordArrow = () /*@arrowKeyword*/=> 1;
const KeywordClass = /*@classKeyword*/class {};
