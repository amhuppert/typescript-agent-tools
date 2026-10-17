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
interface CopyCheapascii64 { ["A"/*AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA*/]: number }
interface CopyCheapascii64<A = unknown> {}
interface CopyChosenascii64 { ["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"]: number }
interface CopyChosenascii64<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA = unknown> {}
interface CopyCheapunicode64 { ["Σ"/*ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ*/]: number }
interface CopyCheapunicode64<Σ = unknown> {}
interface CopyChosenunicode64 { ["ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"]: number }
interface CopyChosenunicode64<ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ = unknown> {}
interface CopyCheapsupplementary64 { ["𝒜"/*𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜*/]: number }
interface CopyCheapsupplementary64<𝒜 = unknown> {}
interface CopyChosensupplementary64 { ["𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜"]: number }
interface CopyChosensupplementary64<𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜 = unknown> {}
interface CopyCheapescaped64 { ["A"/*AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA*/]: number }
interface CopyCheapescaped64<\u0041 = unknown> {}
interface CopyChosenescaped64 { ["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"]: number }
interface CopyChosenescaped64<\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041 = unknown> {}
interface CopyCheapescapedSupplementary64 { ["𝒜"/*𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜*/]: number }
interface CopyCheapescapedSupplementary64<\u{1d49c} = unknown> {}
interface CopyChosenescapedSupplementary64 { ["𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜"]: number }
interface CopyChosenescapedSupplementary64<\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c} = unknown> {}
interface CopyCheapunderscores64 { ["_"/*________________________________________________________________*/]: number }
interface CopyCheapunderscores64<_ = unknown> {}
interface CopyChosenunderscores64 { ["________________________________________________________________"]: number }
interface CopyChosenunderscores64<________________________________________________________________ = unknown> {}
function parameterCopy64() {
  const asciiCheap = (null as unknown as CopyCheapascii64<number>, 0 as number);
  const asciiChosen = (null as unknown as CopyChosenascii64<number>, 0 as number);
  const unicodeCheap = (null as unknown as CopyCheapunicode64<number>, 0 as number);
  const unicodeChosen = (null as unknown as CopyChosenunicode64<number>, 0 as number);
  const supplementaryCheap = (null as unknown as CopyCheapsupplementary64<number>, 0 as number);
  const supplementaryChosen = (null as unknown as CopyChosensupplementary64<number>, 0 as number);
  const escapedCheap = (null as unknown as CopyCheapescaped64<number>, 0 as number);
  const escapedChosen = (null as unknown as CopyChosenescaped64<number>, 0 as number);
  const escapedSupplementaryCheap = (null as unknown as CopyCheapescapedSupplementary64<number>, 0 as number);
  const escapedSupplementaryChosen = (null as unknown as CopyChosenescapedSupplementary64<number>, 0 as number);
  const underscoresCheap = (null as unknown as CopyCheapunderscores64<number>, 0 as number);
  const underscoresChosen = (null as unknown as CopyChosenunderscores64<number>, 0 as number);
}
