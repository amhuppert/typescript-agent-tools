declare function pick<T>(value: T): T;
export const /*#typeArgument*/picked = pick<import("./lib.js").Name>("a");
export const /*#genericTypeArgument*/limited = pick<import("./lib.js").Limited<"a">>({ limited: "a" });
export const /*#violatedConstraint*/violated = pick<import("./lib.js").Limited<number>>({ limited: 1 });
