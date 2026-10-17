export {};
abstract class Base {
    private own: number = 1;
    private static hidden: string = "base";
    protected item: number = 1;
    protected static token: string = "token";
    public field: number = 1;
    abstract required: number;
    abstract missingMethod(): number;
    protected method(): number { return 1; }
    get paired(): number { return 1; }
    protected set paired(value: number) {}
    ownRead() { return this./*#privateOwn*/own; }
    peer(other: Base) { return other./*#privatePeer*/own; }
    nested(other: Base) { class Inner { read() { return other./*#privateNested*/own; } } return Inner; }
    static ownStatic() { return Base./*#privateStaticOwn*/hidden; }
    protectedRead() { return this./*#protectedOwn*/item; }
    abstractRead() { return this./*#abstractPropertyMethod*/required; }
    constructor() { this./*#abstractPropertyConstructor*/required; }
}
class Derived extends Base {
    required: number = 1;
    missingMethod(): number { return 1; }
    privateRead() { return this./*#privateDerived*/own; }
    peer(other: Derived) { return other./*#protectedPeerDerived*/item; }
    throughBase(other: Base) { return other./*#protectedThroughBase*/item; }
    nested(other: Derived) { class Inner { read() { return other./*#protectedNested*/item; } } return Inner; }
    static inherited() { return Derived./*#protectedStaticDerived*/token; }
    superMethod() { return super./*#superProtectedMethod*/method; }
    superField() { return super./*#superPublicField*/field; }
    superAbstract() { return super./*#superAbstractMethod*/missingMethod; }
    thisType() { return this./*#protectedThisType*/item; }
    compositeGood(other: Base & Derived) { return other./*#compositeBoth*/item; }
    compositeBad(other: Base & Other) { return other./*#compositeOneWrong*/item; }
}
class Other { protected item: number = 1; }
declare const base: Base;
const a = base./*#privateOutside*/own;
const b = Base./*#privateStaticOutside*/hidden;
const c = base./*#protectedOutside*/item;
const d = Derived./*#protectedStaticOutside*/token;
const getter = base./*#getter*/paired;
base./*#setter*/paired = 2;
function explicit(this: Derived) { return this./*#explicitThis*/item; }
function generic<T extends Derived>(this: T) { return this./*#explicitThisGeneric*/item; }
const contextual: (this: Derived) => number = function() { return this./*#contextualThis*/item; };
function explicitStatic(this: typeof Derived) { return this./*#explicitThisStatic*/token; }
