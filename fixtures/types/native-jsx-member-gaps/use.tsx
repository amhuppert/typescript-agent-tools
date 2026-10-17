import { CollapsibleText } from "./comp";
export const first = <CollapsibleText /*#attrFirst*/maxCollapsedHeight={4} text="x" />;
export const second = <CollapsibleText text="y" /*#attrSecond*/maxCollapsedHeight={5} />;
export const untyped = { maxCollapsedHeight: 6, text: "z" };
