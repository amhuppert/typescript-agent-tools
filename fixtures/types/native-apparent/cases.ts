export declare let selected: string | number;
export function stringCase(input: string): void { ({ selected } = input); /*#string*/selected; }
export function numberCase(input: number): void { ({ selected } = input); /*#number*/selected; }
export function booleanCase(input: boolean): void { ({ selected } = input); /*#boolean*/selected; }
export function bigintCase(input: bigint): void { ({ selected } = input); /*#bigint*/selected; }
export function symbolCase(input: symbol): void { ({ selected } = input); /*#symbol*/selected; }
export function stringLiteralCase(input: string): void { ({ selected } = "literal"); /*#stringLiteral*/selected; }
export function numberLiteralCase(input: number): void { ({ selected } = 123); /*#numberLiteral*/selected; }
export function booleanLiteralCase(input: boolean): void { ({ selected } = true); /*#booleanLiteral*/selected; }
export function bigintLiteralCase(input: bigint): void { ({ selected } = 123n); /*#bigintLiteral*/selected; }
interface Box { selected: string | number; latent: Missing; }
export function constrained<T extends Box>(input: T): void { ({ selected } = input); /*#constrained*/selected; }
