import { Foreign, externalNumber, externalString } from "./other";

enum /*#numericDeclaration*/Numeric { Zero, One, Jump = 5, Next }
const /*#numericConst*/numericConst = Numeric.One;
let /*#numericLet*/numericLet = Numeric.One;
const /*#numericObject*/numericObject = Numeric;
enum One { /*#oneMember*/Only = 3 }
enum /*#oneDeclaration*/OneNamed { Only = 3 }
enum /*#duplicateValues*/Duplicate { First = 7, Same = 7, Last = 8 }
enum /*#stringDeclaration*/Words { First = "first", Second = "second" }
const /*#stringMember*/stringMember = Words.First;
enum Quoted { "not-key" = "value", "" = 1 }
const /*#quotedMember*/quotedMember = Quoted["not-key"];
enum /*#emptyDeclaration*/Empty {}
const /*#emptyObject*/emptyObject = Empty;

declare function numberSource(): number;
enum /*#computedDeclaration*/Computed { First = numberSource(), Second = 2 }
const /*#computedFresh*/computedFresh = Computed.First;
let /*#computedWidened*/computedWidened = Computed.First;
enum /*#afterComputed*/AfterComputed { First = numberSource(), Next }
declare enum /*#ambientDeclaration*/Ambient { First, Second = 4, Third }
declare const enum /*#ambientConst*/AmbientConst { First, Second }
const enum /*#constValues*/ConstValues { Zero, Negative = -2, Next, Text = "text" }

enum /*#localReferences*/Local { A = 1, B = A + 2, C = Local.B + 1, D = Local["C"] + 1 }
enum /*#laterReference*/Later { A = B, B = 7 }
enum /*#selfReference*/Self { A = A }
enum /*#foreignReference*/ForeignReference { A = Foreign.Number, B = Foreign.Text }
enum /*#externalConstants*/ExternalConstants { A = externalNumber, B, C = externalString }
enum /*#constantArithmetic*/Arithmetic { A = 1 << 3, B = (A | 2) ** 2, C = `value${A}` }
enum /*#specialNumbers*/SpecialNumbers { InfinityValue = Infinity, NaNValue = NaN, NaNAgain = NaN, Zero = -0 }

declare function badNumber(): MissingEnumReturn;
enum /*#computedBadType*/BadComputed { A = badNumber(), B = 1 }
const enum /*#constSkipsBadType*/ConstSkipped { A = badNumber() }
declare enum /*#ambientSkipsBadType*/AmbientSkipped { A = badNumber() }
enum /*#badThenLater*/BadThenLater { A = badNumber(), B = laterBadNumber() }
declare function laterBadNumber(): MissingLaterEnumReturn;

enum /*#mergedFirst*/Merged { A = 1 }
enum Merged { B = 2 }
enum /*#namespaceMerge*/Augmented { A = 1 }
namespace Augmented { export const extra = "extra"; }
const /*#augmentedObject*/augmentedObject = Augmented;
const /*#arrayWidening*/arrayWidening = [Numeric.One, Words.First];
const /*#objectWidening*/objectWidening = { number: Numeric.One, text: Words.First };
const /*#readonlyPreserves*/readonlyPreserves = { member: Numeric.One } as const;
declare function choose(value: Numeric): "enum";
declare function choose(value: string): "text";
const /*#literalAssignable*/literalAssignable = choose(1);
const /*#literalMismatch*/literalMismatch = choose(99);
