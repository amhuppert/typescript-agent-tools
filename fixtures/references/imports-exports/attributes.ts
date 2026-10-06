// Import attribute keys are not references; top-level `await` reads the import.
import { /*@value:i*/value } from "./lib" with { /*@!*/kind: "x" };
const /*@result:d*/result = await /*@value:u*/value;
export const /*@later:de*/later = /*@result:u*/result + await /*@value:u*/value;
