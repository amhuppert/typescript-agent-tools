interface Shape<T> { (value: T): T; [key: string]: T extends string ? number : boolean; }
export let /*#query*/result: Shape<string>;
