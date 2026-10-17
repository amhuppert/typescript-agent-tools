interface NativeInventory<T> {
  /*#value*/value: T;
  /*#propertyT*/T: string;
  "": string;
  "a.b": number;
  0x10: boolean;
  \u0065scaped: number;
}
interface NativeInventory<T> {
  later: boolean;
  value: number;
}
export declare let actualInventory: /*#inventory*/NativeInventory<number>;
