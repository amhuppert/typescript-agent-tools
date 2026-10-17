export class ParameterBox {
  constructor(/*#memberModifier*/public readonly /*#parameterName*/slot: string) {
    /*#parameterUse*/slot;
    this./*#constructorMember*/slot;
  }
}
declare const parameterBox: ParameterBox;
parameterBox./*#memberUse*/slot;

export interface Shape { /*#shapeKey*/task: number }
const /*#localName*/task = 1;
const object: Shape = { /*#shorthand*/task };
object./*#shapeUse*/task;
const { /*#bindingKey*/task: /*#bindingName*/copy } = object;
function read(shape: Shape) {
  const { /*#bindingShort*/task } = shape;
  return /*#bindingShortUse*/task;
}

export interface Keyed { /*#literalDeclaration*/"lit": number }
declare const keyed: Keyed;
keyed[/*#literalUse*/"lit"];
keyed[/*#templateUse*/`lit`];
keyed./*#dotUse*/lit;
const /*#computedLocal*/lit = "lit";
keyed[/*#computedUse*/lit];
const arbitrary = "lit";
keyed[/*#otherComputedUse*/arbitrary];
const computedObject = { [/*#computedName*/arbitrary]: 2 };
