export {};
class Padding0 { #padding = 0; }
class Padding1 { #padding = 0; }
class Padding2 { #padding = 0; }
class Padding3 { #padding = 0; }
class Padding4 { #padding = 0; }
class Padding5 { #padding = 0; }
class Padding6 { #padding = 0; }
class Padding7 { #padding = 0; }
class Padding8 { #padding = 0; }
class Owner {
    #field1: number = 1;
    whole(v: Owner | {[key: `${string}#${number}@#field1`]: string}) { return v./*#whole*/#field1; }
    split(v: Owner | {[key: `${string}#${number}${number}@#field1`]: string}) { return v./*#split*/#field1; }
    suffix(v: Owner | {[key: `${string}#field1`]: string}) { return v./*#suffix*/#field1; }
}
