// Enum declarations merge with each other and with namespaces; enum members
// are unsupported targets.
enum /*@Color:d*/Color { /*@!*/Red, /*@!*/Green = /*@!*/Red + 1 }
enum /*@Color:d*/Color { /*@!*/Blue = 5 }
namespace /*@Color:d*/Color { export const /*@favorite:de*/favorite = /*@Color:u*/Color./*@!*/Red; }
const enum /*@Flags:d*/Flags { /*@!*/None = 0, /*@!*/All = /*@!*/None | 1 }

let /*@color:d*/color: /*@Color:u*/Color = /*@Color:u*/Color./*@!*/Green;
let /*@red:d*/red: /*@Color:u*/Color./*@!*/Red = /*@Color:u*/Color./*@!*/Red;
let /*@flags:d*/flags = /*@Flags:u*/Flags./*@!*/All;
/*@Color:u*/Color./*@favorite:u*/favorite;
/*@color:u*/color; /*@red:u*/red; /*@flags:u*/flags;
export {};

// Inside an enum body, including `Color` above, a member shadows this constant.
const /*@outerRed:d*/Red = 2;
/*@outerRed:u*/Red + /*@Color:u*/Color./*@!*/Red;
