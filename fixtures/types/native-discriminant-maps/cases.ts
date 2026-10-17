interface A0 { tag: "a0"; value: 0; }
interface A1 { tag: "a1"; value: 0; }
interface A2 { tag: "a2"; value: 0; }
interface A3 { tag: "a3"; value: 0; }
interface A4 { tag: "a4"; value: 0; }
interface A5 { tag: "a5"; value: 0; }
interface A6 { tag: "a6"; value: 0; }
interface A7 { tag: "a7"; value: 0; }
interface A8 { tag: "a8"; value: 0; }
interface A9 { tag: "a9"; value: 0; }
interface A10 { tag: "a10"; value: 0; }
interface Source { tag: "a0"; value: 0; }
interface Broken { tag: MissingDiscriminant; value: 0; }
interface Broad { tag: string; value: 0; }
interface Duplicate { tag: "a0"; value: 0; }
interface DuplicateAgain { tag: "a0"; value: 0; }
interface Empty0 { "": "empty"; tag: "e0"; }
interface Empty1 { "": "empty"; tag: "e1"; }
interface Empty2 { "": "empty"; tag: "e2"; }
interface Empty3 { "": "empty"; tag: "e3"; }
interface Empty4 { "": "empty"; tag: "e4"; }
interface Empty5 { "": "empty"; tag: "e5"; }
interface Empty6 { "": "empty"; tag: "e6"; }
interface Empty7 { "": "empty"; tag: "e7"; }
interface Empty8 { "": "empty"; tag: "e8"; }
interface Empty9 { "": "empty"; tag: "e9"; }
interface EmptySource { "": "empty"; tag: "e0"; }
type underNineTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | Broken;
declare function underNineCheck<T extends underNineTarget>(): "matched";
const /*#underNine*/underNine = underNineCheck<Source>();
type atTenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | Broken;
declare function atTenCheck<T extends atTenTarget>(): "matched";
const /*#atTen*/atTen = atTenCheck<Source>();
type overElevenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | Broken;
declare function overElevenCheck<T extends overElevenTarget>(): "matched";
const /*#overEleven*/overEleven = overElevenCheck<Source>();
type healthyNineTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8;
declare function healthyNineCheck<T extends healthyNineTarget>(): "matched";
const /*#healthyNine*/healthyNine = healthyNineCheck<Source>();
type healthyTenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9;
declare function healthyTenCheck<T extends healthyTenTarget>(): "matched";
const /*#healthyTen*/healthyTen = healthyTenCheck<Source>();
type healthyElevenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | A10;
declare function healthyElevenCheck<T extends healthyElevenTarget>(): "matched";
const /*#healthyEleven*/healthyEleven = healthyElevenCheck<Source>();
type nonLiteralTenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | Broad;
declare function nonLiteralTenCheck<T extends nonLiteralTenTarget>(): "matched";
const /*#nonLiteralTen*/nonLiteralTen = nonLiteralTenCheck<Source>();
type duplicateTenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | Duplicate;
declare function duplicateTenCheck<T extends duplicateTenTarget>(): "matched";
const /*#duplicateTen*/duplicateTen = duplicateTenCheck<Source>();
type duplicateElevenTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | Duplicate;
declare function duplicateElevenCheck<T extends duplicateElevenTarget>(): "matched";
const /*#duplicateEleven*/duplicateEleven = duplicateElevenCheck<Source>();
type duplicateThirdTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | Duplicate | DuplicateAgain;
declare function duplicateThirdCheck<T extends duplicateThirdTarget>(): "matched";
const /*#duplicateThird*/duplicateThird = duplicateThirdCheck<Source>();
type aboveHalfTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | "primitive0" | "primitive1" | "primitive2" | "primitive3" | "primitive4" | "primitive5" | "primitive6" | "primitive7" | "primitive8";
declare function aboveHalfCheck<T extends aboveHalfTarget>(): "matched";
const /*#aboveHalf*/aboveHalf = aboveHalfCheck<Source>();
type halfTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | "primitive0" | "primitive1" | "primitive2" | "primitive3" | "primitive4" | "primitive5" | "primitive6" | "primitive7" | "primitive8" | "primitive9";
declare function halfCheck<T extends halfTarget>(): "matched";
const /*#half*/half = halfCheck<Source>();
type belowHalfTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | A8 | A9 | "primitive0" | "primitive1" | "primitive2" | "primitive3" | "primitive4" | "primitive5" | "primitive6" | "primitive7" | "primitive8" | "primitive9" | "primitive10";
declare function belowHalfCheck<T extends belowHalfTarget>(): "matched";
const /*#belowHalf*/belowHalf = belowHalfCheck<Source>();
type objectCountNineTarget = A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 | Broken | "primitive";
declare function objectCountNineCheck<T extends objectCountNineTarget>(): "matched";
const /*#objectCountNine*/objectCountNine = objectCountNineCheck<Source>();
type emptyNameTarget = Empty0 | Empty1 | Empty2 | Empty3 | Empty4 | Empty5 | Empty6 | Empty7 | Empty8 | Empty9;
declare function emptyNameCheck<T extends emptyNameTarget>(): "matched";
const /*#emptyName*/emptyName = emptyNameCheck<EmptySource>();
interface P0 { payload: number; tag: "p0"; }
interface P1 { payload: MissingPayload; tag: "p1"; }
interface P2 { payload: number; tag: "p2"; }
interface P3 { payload: number; tag: "p3"; }
interface P4 { payload: number; tag: "p4"; }
interface P5 { payload: number; tag: "p5"; }
interface P6 { payload: number; tag: "p6"; }
interface P7 { payload: number; tag: "p7"; }
interface P8 { payload: number; tag: "p8"; }
interface P9 { payload: number; tag: "p9"; }
interface P10 { payload: number; tag: "p10"; }
interface PSource8 { payload: 0; tag: "p8"; }
interface PSource9 { payload: 0; tag: "p9"; }
interface PSource10 { payload: 0; tag: "p10"; }
interface PDuplicate0 { payload: number; tag: "p0"; }
interface PDuplicateAgain0 { payload: number; tag: "p0"; }
interface PDuplicate9 { payload: number; tag: "p9"; }
type payloadUnderNineTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8;
declare function payloadUnderNineCheck<T extends payloadUnderNineTarget>(): "matched";
const /*#payloadUnderNine*/payloadUnderNine = payloadUnderNineCheck<PSource8>();
type payloadAtTenTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9;
declare function payloadAtTenCheck<T extends payloadAtTenTarget>(): "matched";
const /*#payloadAtTen*/payloadAtTen = payloadAtTenCheck<PSource9>();
type payloadOverElevenTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | P10;
declare function payloadOverElevenCheck<T extends payloadOverElevenTarget>(): "matched";
const /*#payloadOverEleven*/payloadOverEleven = payloadOverElevenCheck<PSource10>();
type payloadHalfTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | "pPrimitive0" | "pPrimitive1" | "pPrimitive2" | "pPrimitive3" | "pPrimitive4" | "pPrimitive5" | "pPrimitive6" | "pPrimitive7" | "pPrimitive8" | "pPrimitive9";
declare function payloadHalfCheck<T extends payloadHalfTarget>(): "matched";
const /*#payloadHalf*/payloadHalf = payloadHalfCheck<PSource9>();
type payloadBelowHalfTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | "pPrimitive0" | "pPrimitive1" | "pPrimitive2" | "pPrimitive3" | "pPrimitive4" | "pPrimitive5" | "pPrimitive6" | "pPrimitive7" | "pPrimitive8" | "pPrimitive9" | "pPrimitive10";
declare function payloadBelowHalfCheck<T extends payloadBelowHalfTarget>(): "matched";
const /*#payloadBelowHalf*/payloadBelowHalf = payloadBelowHalfCheck<PSource9>();
type payloadDuplicateTenTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | PDuplicate0;
declare function payloadDuplicateTenCheck<T extends payloadDuplicateTenTarget>(): "matched";
const /*#payloadDuplicateTen*/payloadDuplicateTen = payloadDuplicateTenCheck<PSource8>();
type payloadDuplicateElevenTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | PDuplicate0;
declare function payloadDuplicateElevenCheck<T extends payloadDuplicateElevenTarget>(): "matched";
const /*#payloadDuplicateEleven*/payloadDuplicateEleven = payloadDuplicateElevenCheck<PSource9>();
type payloadAmbiguousTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | PDuplicate9;
declare function payloadAmbiguousCheck<T extends payloadAmbiguousTarget>(): "matched";
const /*#payloadAmbiguous*/payloadAmbiguous = payloadAmbiguousCheck<PSource9>();
type payloadThirdDuplicateTarget = P0 | P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | PDuplicate0 | PDuplicateAgain0;
declare function payloadThirdDuplicateCheck<T extends payloadThirdDuplicateTarget>(): "matched";
const /*#payloadThirdDuplicate*/payloadThirdDuplicate = payloadThirdDuplicateCheck<PSource8>();
