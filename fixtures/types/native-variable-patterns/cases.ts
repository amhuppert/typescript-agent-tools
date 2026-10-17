declare const record: { x: string; count: number; nested: { leaf: boolean }; optional?: number };
const { /*#object*/x } = record;
const { x: /*#renamed*/renamed } = record;
const { nested: { /*#nested*/leaf } } = record;
const { optional: /*#defaultInferred*/defaultInferred = 1 } = record;
const { optional: /*#defaultAnnotated*/defaultAnnotated = 1 }: { optional?: number } = record;
declare const tuple: [string, number, boolean?];
const [/*#arrayHead*/arrayHead, , /*#arrayOptional*/arrayOptional] = tuple;
const [, .../*#arrayRest*/arrayRest] = tuple;
declare const anyValue: any;
const { missing: /*#anyParent*/anyParent } = anyValue;
try {} catch (/*#caught*/caught) {}
for (let /*#forIn*/forIn in record) {}
for (let /*#forOf*/forOf of tuple) {}
class Empty { /*#classNil*/value; }
function ordinary(/*#parameter*/parameter = 1, .../*#restParameter*/restParameter: string[]) {}
const { nested: { leaf: /*#nestedDefault*/nestedDefault = false } } = record;
const { optional: /*#optionalAnnotation*/optionalAnnotation }: { optional?: number } = record;
