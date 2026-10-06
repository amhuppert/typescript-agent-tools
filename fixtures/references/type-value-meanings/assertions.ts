// `const` in a const assertion is spelled as a type name but is not one.
export const /*@assertedA:de*/a = [1] as /*@!*/const;
export const /*@assertedB:de*/b = </*@!*/const>[/*@assertedA:u*/a];
