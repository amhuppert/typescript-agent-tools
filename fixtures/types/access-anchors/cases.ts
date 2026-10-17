interface Box {
  value: number;
  optional?: string;
  readonly fixed: 42;
  get mixed(): string;
  set mixed(value: boolean);
}
declare let box: Box;
interface ReadonlyIndex { readonly [key: string]: number; }
declare const index: ReadonlyIndex;
declare function plain(): number;
box./*#read*/value;
box./*#readAccessor*/mixed;
box./*#writeAccessor*/mixed = true;
box./*#compoundAccessor*/mixed += "x";
box./*#writeOptional*/optional = "x";
box./*#readonlyProperty*/fixed = 42;
index./*#readonlyIndex*/anything = 1;
box./*#missing*/absent;
box["value"/*#element*/];
box[/*#key*/"value"];
plain(/*#call*/);
function unrelated(): void {
  box.value = 1;
  const unrelatedRead = box.optional;
}
function assigned(): void {
  box.optional = "assigned";
  const assignedRead = box.optional;
}
