export {};
class HelperUse {
    #value: number = 1;
    read() { return this./*#get*/#value; }
    write() { this./*#set*/#value = 2; }
    compound() { this./*#setThenGet*/#value += 2; }
    repeat() { const /*#repeatedGet*/result = (this.#value, this.#value); return result; }
    writeRead() { const /*#writeThenRead*/result = (this.#value = 2, this.#value); return result; }
}
