export {};
type Build<N extends number, T extends unknown[] = []> = T['length'] extends N ? T : Build<N, [...T, unknown]>;
type /*#tailUnder*/TailUnder = Build<999>;
type /*#tailAt*/TailAt = Build<1000>;
type /*#tailOver*/TailOver = Build<1001>;
