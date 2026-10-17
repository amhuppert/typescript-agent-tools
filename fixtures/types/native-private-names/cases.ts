export {};
class /*#base*/Base {
    #same!: number;
    "#same"!: string;
    static #static!: boolean;
    get #access(): string { return "access"; }
    set #access(value: string) {}
    method() { return this.#same; }
}
class /*#derived*/Derived extends Base { #same!: string; }
class /*#merged*/Merged { #secret!: number; }
interface Merged { public: boolean; }
declare let /*#staticSide*/staticSide: typeof Base;
const expression = class /*#namedExpression*/Named { #same!: string; };
const anonymous = class { /*#anonymousMember*/#same!: number; };
class /*#outer*/Outer {
    #same!: number;
    method() {
        class /*#local*/Local { #same!: boolean; }
        return Local;
    }
}
