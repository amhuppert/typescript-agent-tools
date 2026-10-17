declare let guarded: string | number;
if (typeof guarded === "string") {}
interface Array<T> { length: number; [index: number]: typeof guarded; }
