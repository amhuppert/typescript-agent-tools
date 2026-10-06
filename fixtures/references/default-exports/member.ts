// A default export of a property access exports a property, not a
// declaration; a namespace qualifier keeps its namespace meaning.
const /*@object:d*/object = { /*@!*/x: 1 };
export default /*@object:u*/object./*@!*/x;
namespace /*@Space:d*/Space { export const /*@spaceX:de*/x = 1; }
export const /*@qualified:de*/qualified = /*@Space:u*/Space./*@spaceX:u*/x;
