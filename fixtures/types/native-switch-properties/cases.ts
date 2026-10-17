export {};
type Small = { kind: "a"; value: number } | { kind: "b"; value: string };
type Nine =
    { kind: "k0"; value: 0 } |
    { kind: "k1"; value: 1 } |
    { kind: "k2"; value: 2 } |
    { kind: "k3"; value: 3 } |
    { kind: "k4"; value: 4 } |
    { kind: "k5"; value: 5 } |
    { kind: "k6"; value: 6 } |
    { kind: "k7"; value: 7 } |
    { kind: "k8"; value: 8 };
type Ten =
    { kind: "k0"; value: 0 } |
    { kind: "k1"; value: 1 } |
    { kind: "k2"; value: 2 } |
    { kind: "k3"; value: 3 } |
    { kind: "k4"; value: 4 } |
    { kind: "k5"; value: 5 } |
    { kind: "k6"; value: 6 } |
    { kind: "k7"; value: 7 } |
    { kind: "k8"; value: 8 } |
    { kind: "k9"; value: 9 };
type Eleven =
    { kind: "k0"; value: 0 } |
    { kind: "k1"; value: 1 } |
    { kind: "k2"; value: 2 } |
    { kind: "k3"; value: 3 } |
    { kind: "k4"; value: 4 } |
    { kind: "k5"; value: 5 } |
    { kind: "k6"; value: 6 } |
    { kind: "k7"; value: 7 } |
    { kind: "k8"; value: 8 } |
    { kind: "k9"; value: 9 } |
    { kind: "k10"; value: 10 };
type OtherKey =
    { other: 0; kind: "k0"; value: 0 } |
    { other: 1; kind: "k1"; value: 1 } |
    { other: 2; kind: "k2"; value: 2 } |
    { other: 3; kind: "k3"; value: 3 } |
    { other: 4; kind: "k4"; value: 4 } |
    { other: 5; kind: "k5"; value: 5 } |
    { other: 6; kind: "k6"; value: 6 } |
    { other: 7; kind: "k7"; value: 7 } |
    { other: 8; kind: "k8"; value: 8 } |
    { other: 9; kind: "k9"; value: 9 };
type DuplicateKey = Ten | { kind: "k0"; value: "duplicate" };
type UnionKey =
    { kind: "k0" | "z0"; value: 0 } |
    { kind: "k1" | "z1"; value: 1 } |
    { kind: "k2" | "z2"; value: 2 } |
    { kind: "k3" | "z3"; value: 3 } |
    { kind: "k4" | "z4"; value: 4 } |
    { kind: "k5" | "z5"; value: 5 } |
    { kind: "k6" | "z6"; value: 6 } |
    { kind: "k7" | "z7"; value: 7 } |
    { kind: "k8" | "z8"; value: 8 } |
    { kind: "k9" | "z9"; value: 9 };
type EmptyKey = { "": "a"; value: number } | { "": "b"; value: string };
const kindKey = "kind";
function smallAll(x: Small) { switch (x.kind) { case "a": case "b": return /*#smallAll*/x; } }
function smallCase(x: Small) { switch (x.kind) { case "a": return /*#smallCase*/x; } }
function smallDefault(x: Small) { switch (x.kind) { case "a": break; default: return /*#smallDefault*/x; } }
function smallDefaultGrouped(x: Small) { switch (x.kind) { default: case "a": case "b": return /*#smallDefaultGrouped*/x; } }
function smallJoined(x: Small) { switch (x.kind) { case "a": break; case "b": break; } return /*#smallJoined*/x; }
function smallImplicit(x: Small) { switch (x.kind) { case "a": return; } return /*#smallImplicit*/x; }
function nineAll(x: Nine) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": return /*#nineAll*/x; } }
function nineCase(x: Nine) { switch (x.kind) { case "k0": return /*#nineCase*/x; } }
function tenAll(x: Ten) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#tenAll*/x; } }
function tenCase(x: Ten) { switch (x.kind) { case "k0": return /*#tenCase*/x; } }
function elevenAll(x: Eleven) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": case "k10": return /*#elevenAll*/x; } }
function elevenCase(x: Eleven) { switch (x.kind) { case "k0": return /*#elevenCase*/x; } }
function tenGroup(x: Ten) { switch (x.kind) { case "k0": case "k1": return /*#tenGroup*/x; } }
function tenAbsent(x: Ten) { switch (x.kind) { case "absent": return /*#tenAbsent*/x; } }
function tenDefault(x: Ten) { switch (x.kind) { case "k0": break; default: return /*#tenDefault*/x; } }
function tenGroupedDefault(x: Ten) { switch (x.kind) { default: case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#tenGroupedDefault*/x; } }
function otherKey(x: OtherKey) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#otherKey*/x; } }
function duplicateKey(x: DuplicateKey) { switch (x.kind) { case "k0": return /*#duplicateKey*/x; } }
function unionKey(x: UnionKey) { switch (x.kind) { case "z0": return /*#unionKey*/x; } }
function emptyName(x: EmptyKey) { switch (x[""]) { case "a": case "b": return /*#emptyName*/x; } }
function bracket(x: Ten) { switch (x["kind"]) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#bracket*/x; } }
function escapedName(x: Ten) { switch (x["\u006bind"]) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#escapedName*/x; } }
function constantName(x: Ten) { switch (x[kindKey]) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#constantName*/x; } }
function optionalAll(x: Ten | undefined) { switch (x?.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#optionalAll*/x; } }
function optionalDefault(x: Ten | undefined) { switch (x?.kind) { default: case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#optionalDefault*/x; } }
function optionalUndefined(x: Ten | undefined) { switch (x?.kind) { case undefined: return /*#optionalUndefined*/x; } }
function alreadyNarrowed(x: Ten) { if (x.kind === "k0") { switch(x.kind) { case "k0": return /*#alreadyNarrowed*/x; } } }
function genericProperty<T extends "a" | "b">(x: { kind: T; value: number } | { kind: "b"; value: string }) { switch (x.kind) { case "a": case "b": return /*#genericProperty*/x; } }
function caseRecovery(x: Ten, a: MissingCaseFirst, b: MissingCaseSecond) { switch (x.kind) { case "k0": return /*#caseRecovery*/x; case /*@caseFirst*/a: break; case /*@caseSecond*/b: break; } }
function groupRecovery(x: Ten, a: MissingGroupFirst, b: MissingGroupSecond) { switch (x.kind) { case "k0": case /*@groupFirst*/a: case /*@groupSecond*/b: return /*#groupRecovery*/x; } }
function defaultRecovery(x: Ten, a: MissingDefaultFirst, b: MissingDefaultSecond) { switch (x.kind) { case /*@defaultFirst*/a: break; case /*@defaultSecond*/b: break; default: return /*#defaultRecovery*/x; } }
function smallRecovery(x: Small, a: MissingSmallFirst, b: MissingSmallSecond) { switch (x.kind) { case /*@smallFirst*/a: case /*@smallSecond*/b: return /*#smallRecovery*/x; } }
function unrelatedSkips(x: Small, other: Small, a: MissingUnrelated) { switch (other.kind) { case /*@unrelatedSkipped*/a: return /*#unrelatedSkips*/x; } }
function dynamicNameSkips(x: Ten, key: string, a: MissingDynamicCase) { switch (x[key]) { case /*@dynamicSkipped*/a: return /*#dynamicNameSkips*/x; } }
function emptyNameRecovery(x: EmptyKey, a: MissingEmptyFirst, b: MissingEmptySecond) { switch (x[""]) { case /*@emptyFirst*/a: case /*@emptySecond*/b: return /*#emptyNameRecovery*/x; } }

type Noise0 = { noise: MissingKeyNoise0; kind: "k0"; value: 0 };
type Noise1 = { noise: MissingKeyNoise1; kind: "k1"; value: 1 };
type Noise2 = { noise: MissingKeyNoise2; kind: "k2"; value: 2 };
type Noise3 = { noise: MissingKeyNoise3; kind: "k3"; value: 3 };
type Noise4 = { noise: MissingKeyNoise4; kind: "k4"; value: 4 };
type Noise5 = { noise: MissingKeyNoise5; kind: "k5"; value: 5 };
type Noise6 = { noise: MissingKeyNoise6; kind: "k6"; value: 6 };
type Noise7 = { noise: MissingKeyNoise7; kind: "k7"; value: 7 };
type Noise8 = { noise: MissingKeyNoise8; kind: "k8"; value: 8 };
type Noise9 = { noise: MissingKeyNoise9; kind: "k9"; value: 9 };
type Noise10 = { noise: MissingKeyNoise10; kind: "k10"; value: 10 };
type NoiseNine = Noise0 | Noise1 | Noise2 | Noise3 | Noise4 | Noise5 | Noise6 | Noise7 | Noise8;
function noiseNine(x: NoiseNine) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": return /*#noiseNine*/x; } }
type NoiseTen = Noise0 | Noise1 | Noise2 | Noise3 | Noise4 | Noise5 | Noise6 | Noise7 | Noise8 | Noise9;
function noiseTen(x: NoiseTen) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#noiseTen*/x; } }
type NoiseEleven = Noise0 | Noise1 | Noise2 | Noise3 | Noise4 | Noise5 | Noise6 | Noise7 | Noise8 | Noise9 | Noise10;
function noiseEleven(x: NoiseEleven) { switch (x.kind) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": case "k10": return /*#noiseEleven*/x; } }
function keyBeforeCases(x: NoiseTen, a: MissingKeyCaseFirst, b: MissingKeyCaseSecond) { switch (x.kind) { case "k0": return /*#keyBeforeCases*/x; case /*@keyFirst*/a: break; case /*@keySecond*/b: break; } }
type EmptyTenNoise =
    { noise: MissingEmptyKeyNoise; "": "k0"; value: 0 } |
    { noise: MissingEmptyKeyNoise; "": "k1"; value: 1 } |
    { noise: MissingEmptyKeyNoise; "": "k2"; value: 2 } |
    { noise: MissingEmptyKeyNoise; "": "k3"; value: 3 } |
    { noise: MissingEmptyKeyNoise; "": "k4"; value: 4 } |
    { noise: MissingEmptyKeyNoise; "": "k5"; value: 5 } |
    { noise: MissingEmptyKeyNoise; "": "k6"; value: 6 } |
    { noise: MissingEmptyKeyNoise; "": "k7"; value: 7 } |
    { noise: MissingEmptyKeyNoise; "": "k8"; value: 8 } |
    { noise: MissingEmptyKeyNoise; "": "k9"; value: 9 };
function emptyNameSkipsMap(x: EmptyTenNoise) { switch (x[""]) { case "k0": case "k1": case "k2": case "k3": case "k4": case "k5": case "k6": case "k7": case "k8": case "k9": return /*#emptyNameSkipsMap*/x; } }
