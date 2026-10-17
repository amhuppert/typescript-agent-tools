export {};
class Owner {
    #field1: number = 1;
    absent(v: Owner | {}) { return v./*#absent*/#field1; }
    stringIndex(v: Owner | {[key: string]: string}) { return v./*#stringIndex*/#field1; }
    numberIndex(v: Owner | {[key: number]: string}) { return v./*#numberIndex*/#field1; }
    symbolIndex(v: Owner | {[key: symbol]: string}) { return v./*#symbolIndex*/#field1; }
    privateSuffix(v: Owner | {[key: `${string}#field1`]: string}) { return v./*#privateSuffix*/#field1; }
    privatePrefix(v: Owner | {[key: `${string}#${number}@#field1`]: string}) { return v./*#privatePrefix*/#field1; }
    replacementPrefix(v: Owner | {[key: `�#${number}@#field1`]: string}) { return v./*#replacementPrefix*/#field1; }
    numericSuffix(v: Owner | {[key: `${string}${number}`]: string}) { return v./*#numericSuffix*/#field1; }
    wrongSuffix(v: Owner | {[key: `${string}#other`]: string}) { return v./*#wrongSuffix*/#field1; }
    recoveryString(v: Owner | {[key: string]: MissingStringIndex}) { return v./*#recoveryString*/#field1; }
    recoveryNumber(v: Owner | {[key: number]: MissingNumberIndex}) { return v./*#recoveryNumber*/#field1; }
    recoveryTemplate(v: Owner | {[key: `${string}#field1`]: MissingTemplateIndex}) { return v./*#recoveryTemplate*/#field1; }
}
