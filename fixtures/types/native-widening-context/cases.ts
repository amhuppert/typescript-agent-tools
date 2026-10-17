let /*#object*/object = { value: 1, nested: { text: "x" } };
let /*#array*/array = [{ value: 1 }, { value: 2 }];
function /*#siblings*/siblings(flag: boolean) { if (flag) return { left: 1 }; return { right: "x" }; }
function /*#nested*/nested(flag: boolean) { if (flag) return { value: { left: 1 } }; return { value: { right: "x" } }; }
function /*#nullable*/nullable(flag: boolean) { if (flag) return undefined; return { value: 1 }; }
function /*#shared*/shared(flag: boolean) { if (flag) return { left: 1, same: { x: "a" } }; return { right: "x", same: { y: 2 } }; }
const frozen = [1, "x"] as const;
let /*#readonly*/readonly = frozen;
let /*#literals*/literals = [1 as const, 2 as const];
