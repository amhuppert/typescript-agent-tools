// The stars supply `shared` with different targets, so the name is ambiguous
// here and in every module that reaches it through this one. TypeScript
// reports TS2308 and keeps the first target.
export * from "./left";
export * from "./right";
