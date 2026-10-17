export {};
class C {}
namespace C { export interface prototype { typeOnly: string; } }
declare let instance: C;
C.prototype = instance;
