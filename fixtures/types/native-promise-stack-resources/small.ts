type Layer8_0 = { readonly leaf: number };
type Layer8_1 = Promise<Layer8_0>;
type Layer8_2 = Promise<Layer8_1>;
type Layer8_3 = Promise<Layer8_2>;
type Layer8_4 = Promise<Layer8_3>;
type Layer8_5 = Promise<Layer8_4>;
type Layer8_6 = Promise<Layer8_5>;
type Layer8_7 = Promise<Layer8_6>;
type Layer8_8 = Promise<Layer8_7>;
declare const pending8_1: Layer8_1;
declare const pending8_2: Layer8_2;
declare const pending8_3: Layer8_3;
declare const pending8_4: Layer8_4;
declare const pending8_5: Layer8_5;
declare const pending8_6: Layer8_6;
declare const pending8_7: Layer8_7;
declare const pending8_8: Layer8_8;
async function queryStack8() {
  const chosen = ({
    warm1: pending8_1,
    warm2: pending8_2,
    warm3: pending8_3,
    warm4: pending8_4,
    warm5: pending8_5,
    warm6: pending8_6,
    warm7: pending8_7,
    warm8: pending8_8,
    result: await pending8_8,
  }, 0 as number);
  const healthy = 0 as number;
}
