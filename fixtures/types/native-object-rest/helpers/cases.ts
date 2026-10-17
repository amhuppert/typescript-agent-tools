export {};
declare const source: { drop: number; kept: string };
let dropped: number;
let rest: { kept: string };
const /*#assignment*/result = ({ drop: dropped, ...rest } = source);
