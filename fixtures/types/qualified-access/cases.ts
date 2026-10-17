export {};
interface Inner { count: number; optional?: string }
interface Holder { inner: Inner; readonly fixed: 42 }
declare let box: Holder;
type Whole = /*#whole*/typeof box.inner.count;
type Segments = typeof /*#root*/box./*#middle*/inner./*#leaf*/count;
type Fixed = /*#fixed*/typeof box.fixed;
type Optional = /*#optional*/typeof box.inner.optional;
declare let indexed: { [name: string]: number };
type Indexed = /*#indexed*/typeof indexed.missing;
declare let anything: any;
type Anything = /*#any*/typeof anything.missing;
type Missing = /*#missing*/typeof box.missing;
declare let nullable: Holder | undefined;
type Nullable = /*#nullable*/typeof nullable.fixed;
function afterUnrelated(box: Holder): void {
  box.inner.count = 1;
  type Unrelated = /*#unrelated*/typeof box.inner.optional;
}
function afterAssignment(box: Holder): void {
  box.inner.optional = "ready";
  type Assigned = /*#assigned*/typeof box.inner.optional;
}
function afterLoop(box: Holder, again: boolean): void {
  while (again) { box.inner.count = 1; }
  type Loop = /*#loop*/typeof box.inner.optional;
}
