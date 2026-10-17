export {};
namespace N {
    export class C {
        static #value = 1;
        static qualified() { return Qualified./*#qualified*/#value; }
        static module() { return Module.C./*#module*/#value; }
        static chained() { return Chained./*#chained*/#value; }
    }
}
import Qualified = N.C;
import Module = N;
import Chained = Module.C;
