export {};
namespace N { export interface T {} }
namespace C { export import prototype = N.T; }
class C {}
