export class PrivateBox {
  private /*#privateDeclaration*/scoped: number = 1;
  read() { return this./*#privateUse*/scoped; }
}
declare const outsideUnknown: unknown;
outsideUnknown./*#outsidePrivateUnknown*/scoped;
export class OtherPrivateBox {
  private /*#otherPrivateDeclaration*/scoped: string = "other";
  read() { return this./*#otherPrivateUse*/scoped; }
}
