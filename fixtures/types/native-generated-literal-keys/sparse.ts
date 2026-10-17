export {};
class Owner {
    #field1 = 1;
    recoveryNumber(v: Owner | {[key: number]: MissingNumberIndex}) {
        return v./*#recoveryNumber*/#field1;
    }
}
