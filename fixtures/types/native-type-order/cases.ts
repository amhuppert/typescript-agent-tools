interface Zebra { zebra: string }
interface Alpha { alpha: number }
interface Gamma { gamma: boolean }
interface Box<T> { value: T }

declare let /*#names*/names: Zebra | Alpha;
declare let /*#arguments*/argumentsOrder: Box<"z"> | Box<"a">;
declare let /*#intersectionLists*/intersectionLists: (Zebra & Alpha) | (Alpha & Zebra);
declare let /*#intersectionLengths*/intersectionLengths: (Zebra & Alpha & Gamma) | (Gamma & Alpha);

type ZebraAlias = Zebra & { tag: "z" };
type AlphaAlias = Alpha & { tag: "a" };
declare let /*#aliasNames*/aliasNames: ZebraAlias | AlphaAlias;
type Choice<T> = T | { choice: T };
declare let /*#aliasArguments*/aliasArguments: Choice<"z"> | Choice<"a">;

declare const /*#early*/early: { early: string };
declare const /*#late*/late: { late: number };
declare let /*#anonymous*/anonymous: typeof late | typeof early;

declare function /*#earlyFunction*/earlyFunction(): string;
declare function /*#lateFunction*/lateFunction(): number;
declare let /*#functions*/functions: typeof lateFunction | typeof earlyFunction;

declare let /*#fileOrder*/fileOrder: typeof secondFile | typeof firstFile;

declare let /*#numbers*/numbers: 10 | -2 | 3;
declare let /*#strings*/strings: "z" | "a" | "A";
declare let /*#booleans*/booleans: true | false;
