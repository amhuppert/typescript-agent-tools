import { Sources, Forms, plainFunction, arrow } from "./cases";
import { Choice, ASSERTED } from "./constants";

const OTHER = "other-value";

export const sources = /*#sourcesCall*/new Sources(OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, 2, false, Choice.First, Choice.First, OTHER, OTHER, ASSERTED, "record-value", OTHER, "shape-value");
export const forms = /*#formsCall*/new Forms(OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER);
export const /*#called*/called = plainFunction(OTHER);
export const /*#arrowed*/arrowed = arrow(OTHER);
export const /*#described*/described = new Forms(OTHER).describe();
forms./*#outsidePublicReadonly*/publicReadonly;
forms./*#outsideReadonlyOnly*/readonlyOnly;
