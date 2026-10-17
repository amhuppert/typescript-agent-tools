export {};
class Owner {
    #field1: number = 1;
    whole(v: Owner | {[key: `${string}#${number}@#field1`]: string}) { return v./*#whole*/#field1; }
    split(v: Owner | {[key: `${string}#${number}${number}@#field1`]: string}) { return v./*#split*/#field1; }
    suffix(v: Owner | {[key: `${string}#field1`]: string}) { return v./*#suffix*/#field1; }
}
