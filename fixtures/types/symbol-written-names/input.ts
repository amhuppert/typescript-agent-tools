interface Bare { T: number }
interface Bare</*#bare*/T = unknown> {}
interface Quoted { "T": number }
interface Quoted</*#quoted*/T = unknown> {}
interface QuotedEscape { "\u0054": number }
interface QuotedEscape</*#quotedEscape*/T = unknown> {}
interface IdentifierEscape { \u0054: number }
interface IdentifierEscape</*#identifierEscape*/T = unknown> {}
interface Computed { ["T"]: number }
interface Computed</*#computed*/T = unknown> {}
