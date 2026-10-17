export function shared(value: { n: number }) { return value; }
export function optional(value?: number) { return value; }
export function initialized(value: number = MissingInitializer) { return value; }
