export {};
import Same = Exposed.Named;
class Cell {
    #value: number = 1;
    static #static: string = "static";
    #method() { return "method" as const; }
    get #both(): string { return "read"; }
    set #both(value: number) {}
    set #only(value: boolean) {}
    readonly #fixed: 7 = 7;
    "#value": boolean = true;
    constructor() { this./*#constructorReadonly*/#fixed = 7; }
    read() { return this./*#instance*/#value; }
    peer(other: Cell) { return other./*#peer*/#value; }
    generic<T extends Cell>(other: T) { return other./*#generic*/#value; }
    intersect(other: Cell & { extra: string }) { return other./*#intersection*/#value; }
    union(other: Cell | Derived) { return other./*#union*/#value; }
    derived(other: Derived) { return other./*#derived*/#value; }
    getter() { return this./*#getter*/#both; }
    setter() { this./*#setter*/#both = 1; }
    setonly() { return this./*#setonlyRead*/#only; }
    setonlyWrite() { this./*#setonlyWrite*/#only = true; }
    write() { this./*#definite*/#value = 2; }
    compound() { this./*#compound*/#value += 2; }
    methodRead() { return this./*#methodRead*/#method; }
    methodCall() { return this./*#methodCall*/#method(); }
    methodWrite() { this./*#methodWrite*/#method = () => "method"; }
    static staticThis() { return this./*#staticThis*/#static; }
    static staticName() { return Cell./*#staticName*/#static; }
    nestedFunction(other: Cell) {
        function throughFunction() { return other./*#nestedFunction*/#value; }
        return throughFunction;
    }
    nestedClass(other: Cell) {
        class Inner { read() { return other./*#outerLexical*/#value; } }
        return Inner;
    }
    genuineAny(other: any) { return other./*#genuineAny*/#value; }
    literalCollision() { return this["#value"/*#literalCollision*/]; }
    localClass() {
        class Local { #value = false; read() { return this./*#localClass*/#value; } }
        return Local;
    }
}
class Derived extends Cell { #value: boolean = false; }
namespace Exposed {
    export class Named {
        static #token = "namespace";
        static qualified() { return Exposed.Named./*#qualifiedSelf*/#token; }
        static alias() { return Same./*#aliasSelf*/#token; }
    }
}
const anonymous = class { #value: number = 9; read() { return this./*#anonymousClass*/#value; } };
