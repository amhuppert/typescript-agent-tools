export function scalarJoin(condition: boolean): void {
    let value: string | number = 1;
    if (condition) value = "later";
    /*scalar-join*/ value;
}
export function booleanJoin(condition: boolean): void {
    let value: boolean = true;
    if (condition) value = false;
    /*boolean-join*/ value;
}
export function unionAssignment(candidate: string | number): void {
    let value: string | number = 1;
    value = candidate;
    /*union-assigned*/ value;
}
export function wrongAssignment(): void {
    let value: string | number = 1;
    value = true;
    /*wrong-assigned*/ value;
}
export function missingAssignment(): void {
    let value: string | number = 1;
    value = Missing;
    /*missing-assigned*/ value;
}
export function unusedAssignment(): void {
    let value: number = 1;
    value = Missing;
    /*unused-assigned*/ value;
}
