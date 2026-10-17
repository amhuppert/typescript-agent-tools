declare const numeric: { 1: number; 1.5: string; 1e3: boolean; 0: null };
/*#numeric*/numeric;
declare const stringNamed: { "1": number; "1.5": string; "1e3": boolean; "01": null; "-1": undefined };
/*#stringNamed*/stringNamed;
declare const singleQuoted: { '1': number; 'with space': string; 'line\nend': boolean };
/*#singleQuoted*/singleQuoted;
declare const identifiers: { "plain": number; 'other': string; 'é': boolean };
/*#identifiers*/identifiers;
declare const methods: { new(): number; "1"(): string; 2(): boolean; 'with space'(): null };
/*#methods*/methods;
declare const mappedNumbers: { [P in -1 | 0 | 1.5 | 1000]: P };
/*#mappedNumbers*/mappedNumbers;
declare const mappedStrings: { [P in "-1" | "1" | "01" | "1e3" | "with space"]: P };
/*#mappedStrings*/mappedStrings;
declare const computedNumber: { [1]: number; [-1]: string };
/*#computedNumber*/computedNumber;
declare const computedString: { ["1"]: number; ["-1"]: string; ["with space"]: boolean };
/*#computedString*/computedString;
interface Mixed { 1: number }
interface Mixed { "1": number }
declare const mixed: { [P in keyof Mixed]: Mixed[P] };
/*#mixed*/mixed;
interface Quoted { 'with space': number }
interface Quoted { "with space": number }
declare const quoted: { [P in keyof Quoted]: Quoted[P] };
/*#quoted*/quoted;
declare const methodNew: { "new"(): number; 'new'?(): number };
/*#methodNew*/methodNew;
