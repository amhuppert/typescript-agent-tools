interface Box<T> {
  /*#first*/value: T;
  T: number;
  "": string;
  "a.b": number;
  0x10: number;
  ["computed"]: string;
  [17]: number;
  \u0065scaped: string;
  run(x: T): T;
  run(x: T, y: number): T;
  get paired(): string;
  set paired(value: string);
  (): T;
  new(): T;
  [index: number]: T;
}
interface Box<T> {
  /*#second*/value: T;
  later: T;
}
declare class C {
  instance: number;
  static instance: string;
  #private: boolean;
  private ordinary: number;
  accessor automatic: string;
  get paired(): string;
  set paired(value: string);
  constructor(public parameter: number, readonly protectedParameter: string);
}
type Query = /*#query*/Box<number>;
interface FirstNumber { /*#numberFirst*/value: number; }
interface FirstNumber { /*#numberSecond*/value: string; }
interface FirstString { /*#stringFirst*/value: string; }
interface FirstString { /*#stringSecond*/value: number; }
