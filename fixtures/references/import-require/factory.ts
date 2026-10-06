// `export =` of a function merged with a namespace.
function /*@factory:d*/factory() { return 1; }
namespace /*@factory:d*/factory { export const /*@version:de*/version = 1; }
export = /*@factory:e*/factory;
