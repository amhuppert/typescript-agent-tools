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
interface CopyCheapascii16 { ["A"/*AAAAAAAAAAAAAAAA*/]: number }
interface CopyCheapascii16<A = unknown> {}
interface CopyChosenascii16 { ["AAAAAAAAAAAAAAAA"]: number }
interface CopyChosenascii16<AAAAAAAAAAAAAAAA = unknown> {}
interface CopyCheapunicode16 { ["Σ"/*ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ*/]: number }
interface CopyCheapunicode16<Σ = unknown> {}
interface CopyChosenunicode16 { ["ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ"]: number }
interface CopyChosenunicode16<ΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣΣ = unknown> {}
interface CopyCheapsupplementary16 { ["𝒜"/*𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜*/]: number }
interface CopyCheapsupplementary16<𝒜 = unknown> {}
interface CopyChosensupplementary16 { ["𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜"]: number }
interface CopyChosensupplementary16<𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜 = unknown> {}
interface CopyCheapescaped16 { ["A"/*AAAAAAAAAAAAAAAA*/]: number }
interface CopyCheapescaped16<\u0041 = unknown> {}
interface CopyChosenescaped16 { ["AAAAAAAAAAAAAAAA"]: number }
interface CopyChosenescaped16<\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041\u0041 = unknown> {}
interface CopyCheapescapedSupplementary16 { ["𝒜"/*𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜*/]: number }
interface CopyCheapescapedSupplementary16<\u{1d49c} = unknown> {}
interface CopyChosenescapedSupplementary16 { ["𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜𝒜"]: number }
interface CopyChosenescapedSupplementary16<\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c}\u{1d49c} = unknown> {}
interface CopyCheapunderscores16 { ["_"/*________________*/]: number }
interface CopyCheapunderscores16<_ = unknown> {}
interface CopyChosenunderscores16 { ["________________"]: number }
interface CopyChosenunderscores16<________________ = unknown> {}
function parameterCopy16() {
  const asciiCheap = (null as unknown as CopyCheapascii16<number>, 0 as number);
  const asciiChosen = (null as unknown as CopyChosenascii16<number>, 0 as number);
  const unicodeCheap = (null as unknown as CopyCheapunicode16<number>, 0 as number);
  const unicodeChosen = (null as unknown as CopyChosenunicode16<number>, 0 as number);
  const supplementaryCheap = (null as unknown as CopyCheapsupplementary16<number>, 0 as number);
  const supplementaryChosen = (null as unknown as CopyChosensupplementary16<number>, 0 as number);
  const escapedCheap = (null as unknown as CopyCheapescaped16<number>, 0 as number);
  const escapedChosen = (null as unknown as CopyChosenescaped16<number>, 0 as number);
  const escapedSupplementaryCheap = (null as unknown as CopyCheapescapedSupplementary16<number>, 0 as number);
  const escapedSupplementaryChosen = (null as unknown as CopyChosenescapedSupplementary16<number>, 0 as number);
  const underscoresCheap = (null as unknown as CopyCheapunderscores16<number>, 0 as number);
  const underscoresChosen = (null as unknown as CopyChosenunderscores16<number>, 0 as number);
}
