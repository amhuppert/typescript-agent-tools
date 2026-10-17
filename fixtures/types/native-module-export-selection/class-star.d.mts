declare class ClassStar {}
declare namespace ClassStar { export type * from "./remote.mjs"; }
export = ClassStar;
