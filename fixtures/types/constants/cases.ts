export const trueValue = true;
export const falseValue = false;
export let mutableBoolean = false;
export const templatePlain = `ready`;
export const templateEscaped = `A\x42\u{1F600}`;
export const templateLf = `a
b`;
export const templateCr = `ab`;
export const templateCrLf = `a
b`;
export const templateLs = `a b`;
export const templatePs = `a b`;
export const bigZero = 0n;
export const bigDecimal = 123456789012345678901234567890n;
export const bigSeparated = 1_234_567_890n;
export const bigHex = 0x18ee90ff6c373e0ee4e3f0ad2n;
export const bigHexUpper = 0X18EE90FF6C373E0EE4E3F0AD2n;
export const bigBinary = 0b1100011101110100100001111111101101100001101110011111000001110111001001110001111110000101011010010n;
export const bigBinaryUpper = 0B0010_1010n;
export const bigOctal = 0o143564417755415637016711617605322n;
export const bigOctalUpper = 0O000_52n;
export const bigHexZero = 0x00_00n;
export const bigBinaryZero = 0b00_00n;
export const bigOctalZero = 0o00_00n;
export const negativeBigZero = -0n;
export const negativeBigHexZero = -0x00n;
export const negativeBig = -0x18ee90ff6c373e0ee4e3f0ad2n;
export const positiveNumber = +1.5;
export const negativeNumber = -1.5;
export const negativeZero = -0;
export const negativeUnderflow = -1e-400;
export const negativeHex = -0xff;
export const positiveSeparated = +1_000;
export const negativeExponent = -1e21;
export const parenthesizedNegative = -(1);
export const parenthesizedPositive = +(1);
export const twiceNegative = -(-1);
export const negativeBoolean = -true;
export const negativeString = -"1";
export const tildeNumber = ~1;
export const parenthesizedNegativeBig = -(1n);
export const tildeBig = ~1n;
export let mutableNegative = -1;
export let mutableBig = 1n;
export const notTrue = !true;
export const notFalse = !false;
export const notZero = !0;
export const notNegativeZero = !(-0);
export const notNonzero = !1;
export const notEmpty = !"";
export const notString = !"ready";
export const notBigZero = !0n;
export const notBigNonzero = !1n;
export const notObject = !{};
