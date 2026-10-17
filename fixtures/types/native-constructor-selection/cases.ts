import {Stub, Overloads, Absent} from './declarations';
export declare class Declared {
    /*#privateDeclared*/#private;
    /*#publicDeclared*/value;
    constructor();
}
export class Unimplemented {
    /*#publicUnimplemented*/value;
    constructor();
}
export class Implemented {
    value: number = 1;
    constructor(value: number);
    constructor(value: string);
    constructor(value: number | string) {}
}
declare const stub: Stub;
declare const overloads: Overloads;
declare const absent: Absent;
declare const implemented: Implemented;
const /*#stubRead*/stubRead = stub.value;
const /*#overloadsRead*/overloadsRead = overloads.value;
const /*#absentRead*/absentRead = absent.value;
const /*#staticRead*/staticRead = Stub.value;
const /*#implementedRead*/implementedRead = implemented.value;
