export {};
declare function consume(value: 0): 0;
declare function consume(value: unknown): 0;
declare function warm(value: 0): 0;
declare function warm(value: unknown): 0;
type M0 = string;
type M1 = Uppercase<M0>;
type M2 = Lowercase<M1>;
type M3 = Uppercase<M2>;
type M4 = Lowercase<M3>;
type M5 = Uppercase<M4>;
type M6 = Lowercase<M5>;
type M7 = Uppercase<M6>;
type M8 = Lowercase<M7>;
type M9 = Uppercase<M8>;
type M10 = Lowercase<M9>;
type M11 = Uppercase<M10>;
type M12 = Lowercase<M11>;
type M13 = Uppercase<M12>;
type M14 = Lowercase<M13>;
type M15 = Uppercase<M14>;
type M16 = Lowercase<M15>;
declare function choose(value: M16): 43;
declare function choose(value: unknown): 44;
const /*#chosen*/chosen = (
    consume([warm(null! as M1),
    warm(null! as M2),
    warm(null! as M3),
    warm(null! as M4),
    warm(null! as M5),
    warm(null! as M6),
    warm(null! as M7),
    warm(null! as M8),
    warm(null! as M9),
    warm(null! as M10),
    warm(null! as M11),
    warm(null! as M12),
    warm(null! as M13),
    warm(null! as M14),
    warm(null! as M15),
    warm(null! as M16)]),
    choose("abc")
);
let healthy: number;
