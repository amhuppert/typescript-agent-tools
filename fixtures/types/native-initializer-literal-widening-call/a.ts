import { K } from "./c";
export class A { constructor(private readonly key = K) {} get(): string { return this.key; } }
