import type {
    Shape as firstVar,
    Shape as firstLet,
    Shape as firstConst,
    Shape as firstUninitialized,
    Shape as captured,
    Shape as nested,
    Shape as exported,
    Shape as stableConst,
    Shape as stableLet,
    Shape as assignedLet,
    Shape as conditionValue,
    Shape as assignedCondition,
} from "./types";

var firstVar: number = 1;
let firstLet: number = 1;
const firstConst: number = 1;
let firstUninitialized: number;
/*#firstVar*/firstVar;
/*#firstLet*/firstLet;
/*#firstConst*/firstConst;
/*#firstUninitialized*/firstUninitialized;

var lastVar: number = 1;
let lastLet: number = 1;
const lastConst: number = 1;
let lastUninitialized: number;
import type {
    Shape as lastVar,
    Shape as lastLet,
    Shape as lastConst,
    Shape as lastUninitialized,
} from "./types";
/*#lastVar*/lastVar;
/*#lastLet*/lastLet;
/*#lastConst*/lastConst;
/*#lastUninitialized*/lastUninitialized;

let directInitialized: number = 1;
let directUninitialized: number;
/*#directInitialized*/directInitialized;
/*#directUninitialized*/directUninitialized;

let captured: string | number = 1;
const before = () => /*#captureBefore*/captured;
captured = "later";
const after = () => /*#captureAfter*/captured;

let nested: string | number = 1;
function mutateNested() { nested = "later"; }
const nestedRead = () => /*#nestedAssignment*/nested;

let exported: string | number = 1;
const exportedRead = () => /*#exportedAssignment*/exported;
export { exported };

declare const records: { [key: string]: string | number };
declare const unknownKey: string;
const stableConst: string = unknownKey;
let stableLet: string = unknownKey;
let assignedLet: string = unknownKey;
if (typeof records[stableConst] === "string") {
    const /*#stableConstElement*/constElement = records[stableConst];
}
if (typeof records[stableLet] === "string") {
    const /*#stableLetElement*/letElement = records[stableLet];
}
if (typeof records[assignedLet] === "string") {
    assignedLet = "different";
    const /*#assignedLetElement*/assignedElement = records[assignedLet];
}

declare const input: string | number;
let conditionValue: string | number = input;
const guard = typeof conditionValue === "string";
if (guard) {
    /*#mutableAliasCondition*/conditionValue;
}
let assignedCondition: string | number = input;
const staleGuard = typeof assignedCondition === "string";
assignedCondition = input;
if (staleGuard) {
    /*#assignedAliasCondition*/assignedCondition;
}
