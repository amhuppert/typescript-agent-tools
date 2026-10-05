// Enum declarations merge with each other and with namespaces; enum members
// are unsupported targets.
enum /*@Color*/Color { /*@!*/Red, /*@!*/Green = /*@!*/Red + 1 }
enum /*@Color*/Color { /*@!*/Blue = 5 }
namespace /*@Color*/Color { export const /*@favorite*/favorite = /*@Color*/Color./*@!*/Red; }
const enum /*@Flags*/Flags { /*@!*/None = 0, /*@!*/All = /*@!*/None | 1 }

let /*@color*/color: /*@Color*/Color = /*@Color*/Color./*@!*/Green;
let /*@red*/red: /*@Color*/Color./*@!*/Red = /*@Color*/Color./*@!*/Red;
let /*@flags*/flags = /*@Flags*/Flags./*@!*/All;
/*@Color*/Color./*@favorite*/favorite;
/*@color*/color; /*@red*/red; /*@flags*/flags;
export {};
