export function variables(): void {
    let value: string | number = 1;
    const write = value = "s", read = () => /*variable-inside*/ value;
    const after = () => /*variable-after*/ value;
}
export function expressions(): void {
    let value: string | number = 1;
    (value = "s", () => /*expression-inside*/ value);
    const after = () => /*expression-after*/ value;
}
export function ifs(condition: boolean): void {
    let value: string | number = 1;
    if (condition) {
        value = "s";
        const inside = () => /*if-inside*/ value;
    }
    const after = () => /*if-after*/ value;
}
export function dos(condition: boolean): void {
    let value: string | number = 1;
    do {
        value = "s";
        const inside = () => /*do-inside*/ value;
    } while (condition);
    const after = () => /*do-after*/ value;
}
export function whiles(condition: boolean): void {
    let value: string | number = 1;
    while (condition) {
        value = "s";
        const inside = () => /*while-inside*/ value;
    }
    const after = () => /*while-after*/ value;
}
export function fors(condition: boolean): void {
    let value: string | number = 1;
    for (;condition;) {
        value = "s";
        const inside = () => /*for-inside*/ value;
    }
    const after = () => /*for-after*/ value;
}
export function forIns(): void {
    let value: string | number = 1;
    for (let key in { x: 1 }) {
        value = "s";
        const inside = () => /*for-in-inside*/ value;
    }
    const after = () => /*for-in-after*/ value;
}
export function forOfs(): void {
    let value: string | number = 1;
    for (const key of [1]) {
        value = "s";
        const inside = () => /*for-of-inside*/ value;
    }
    const after = () => /*for-of-after*/ value;
}
export function switches(condition: boolean): void {
    let value: string | number = 1;
    switch (condition) {
        case true:
            value = "s";
            const inside = () => /*switch-inside*/ value;
            break;
        case false: value = "s"; break;
    }
    const after = () => /*switch-after*/ value;
}
export function tries(): void {
    let value: string | number = 1;
    try {
        value = "s";
        const inside = () => /*try-inside*/ value;
    } finally {}
    const after = () => /*try-after*/ value;
}
export function classes(): void {
    let value: string | number = 1;
    class C {
        static {
            value = "s";
            const inside = () => /*class-inside*/ value;
        }
    }
    const after = () => /*class-after*/ value;
}
export function nestedRoots(): void {
    let value: string | number = 1;
    const arrow = () => () => /*nested-arrow*/ value;
    function declaration() { return () => /*nested-declaration*/ value; }
}
export function shadow(): void {
    let value: string | number = 1;
    function write() { let value = "s"; value = "t"; }
    const read = () => /*shadow-read*/ value;
}
export function nestedCompound(): void {
    let value: number;
    function write() { value += 1; }
    const read = () => /*nested-compound*/ value;
}
export function nestedDefinite(): void {
    let value: number;
    function write() { value = 1; }
    const read = () => /*nested-definite*/ value;
}
