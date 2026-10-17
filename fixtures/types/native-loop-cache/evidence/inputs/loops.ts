export function nested(outer: boolean, inner: boolean) {
    let value: string | number = 0;
    while (outer) { while (inner) { value = "ready"; inner = false; } outer = false; }
    /*#nestedAfter*/value;
}
export function conditional(loop: boolean, branch: boolean) {
    let value: string | number = 0;
    while (loop) { if (branch) { value = "ready"; } else { value = 1; } }
    /*#conditionalAfter*/value;
}
export function continues(loop: boolean, first: boolean, second: boolean) {
    let value: string | number = 0;
    while (loop) {
        if (first) { value = "ready"; continue; }
        if (second) { value = 1; continue; }
        value = "later";
    }
    /*#continuesAfter*/value;
}
export function innerEntry(outer: boolean, inner: boolean) {
    let value: string | number = 0;
    while (outer) {
        while (inner) { /*#innerHead*/value; value = "ready"; }
        /*#innerExit*/value;
    }
    /*#innerEntryAfter*/value;
}
export function referenceAssignment(loop: boolean) {
    let value: string | number = 0;
    while (loop) { value = /*#assignmentRhs*/value; value = "ready"; }
    /*#referenceAssignmentAfter*/value;
}
export function nestedReferenceAssignment(outer: boolean, inner: boolean) {
    let value: string | number = 0;
    while (outer) { while (inner) { value = /*#nestedRhs*/value; value = "ready"; } }
    /*#nestedReferenceAfter*/value;
}
export function outerBackedge(outer: boolean, inner: boolean) {
    let value: string | number = 0;
    while (outer) {
        value = "ready";
        while (inner) { /*#backedgeInnerHead*/value; value = 1; }
    }
    /*#outerBackedgeAfter*/value;
}
export function narrowedLoop(loop: boolean) {
    let value: string | number = 0;
    while (loop) { value = 1; }
    /*#narrowedAfter*/value;
}
export function doLoop(loop: boolean) {
    let value: string | number = 0;
    do { value = "ready"; } while (loop);
    /*#doAfter*/value;
}
export function declaredEntry(loop: boolean) {
    let value: string | number = initial;
    while (loop) { value = Missing; }
    /*#declaredEntryAfter*/value;
}
export declare let initial: string | number;
export function shadowedLoop(loop: boolean) {
    let value: string | number = 0;
    while (loop) { let value: string | number = "ready"; /*#shadowInner*/value; }
    /*#shadowOuter*/value;
}
export function neverLoop() {
    let value: string | number = 0;
    while (true) { value = "ready"; }
    /*#unreachableAfter*/value;
}
