interface Object {}
interface Function {}
interface Array<T> { length: number; [n: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }
interface String { readonly length: number }
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
function literalCacheAsciiSmall() {
  function nonconstant(): number { return 0; }
  const enum CacheProbe { value = `${nonconstant()}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}${"aaaaaaaaaaaaaaaa"}` }
  const asciiCheap = ("aaaaaaaaaaaaaaaa", "q", CacheProbe.value, 0 as number);
  const asciiChosen = ("aaaaaaaaaaaaaaaa", "q", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaa", 0 as number);
}
function literalCacheUnicodeSmall() {
  function nonconstant(): number { return 0; }
  const enum CacheProbe { value = `${nonconstant()}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}${"ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"}` }
  const unicodeCheap = ("ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "q", CacheProbe.value, 0 as number);
  const unicodeChosen = ("ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "q", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", "ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ", 0 as number);
}
function literalCacheSurrogateSmall() {
  function nonconstant(): number { return 0; }
  const enum CacheProbe { value = `${nonconstant()}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}${"\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800"}` }
  const surrogateCheap = ("\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "q", CacheProbe.value, 0 as number);
  const surrogateChosen = ("\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "q", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", "\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800\ud800", 0 as number);
}
function literalCacheTemplateSmall() {
  function nonconstant(): number { return 0; }
  const enum CacheProbe { value = `${nonconstant()}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}${`aaaaaaaaaaaaaaaa`}` }
  const templateCheap = (`aaaaaaaaaaaaaaaa`, "q", CacheProbe.value, 0 as number);
  const templateChosen = (`aaaaaaaaaaaaaaaa`, "q", `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, `aaaaaaaaaaaaaaaa`, 0 as number);
}
