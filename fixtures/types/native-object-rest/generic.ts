function noOmit<T extends object>(value: T) { const { .../*#noKeys*/rest } = value; return rest; }
function withOmit<T extends { drop: number; kept: string }>(value: T) { const { drop, .../*#omit*/rest } = value; return rest; }
