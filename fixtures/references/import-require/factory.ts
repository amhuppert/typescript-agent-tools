// `export =` of a function merged with a namespace.
function /*@factory*/factory() { return 1; }
namespace /*@factory*/factory { export const /*@version*/version = 1; }
export = /*@factory*/factory;
