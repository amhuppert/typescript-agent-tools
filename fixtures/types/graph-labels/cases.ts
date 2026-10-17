import { Same as A, Named as IA } from "./a";
import { Same as B, Named as IB } from "./b";
type Forward = { first: A<string>; second: B<number>; third: A<number>; repeated: A<string>; nominalA: IA; nominalB: IB };
type Reverse = { second: B<number>; first: A<string>; third: A<number>; repeated: A<string>; nominalB: IB; nominalA: IA };
declare const /*#forward*/forward: Forward;
declare const /*#reverse*/reverse: Reverse;
declare const /*#array*/array: A<string>[];
