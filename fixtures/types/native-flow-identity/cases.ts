export function parameter(loop: boolean, obj: { [name: string]: string | number }, key: string, value: string | number) {
    obj[key] = value;
    while (loop) obj[key] = value;
    const /*#stableParameter*/stableParameter = obj[key];
    return stableParameter;
}

export function constant(loop: boolean, obj: { [name: string]: string | number }, input: string, value: string | number) {
    const key = input;
    obj[key] = value;
    while (loop) obj[key] = value;
    const /*#stableConst*/stableConst = obj[key];
    return stableConst;
}

export function mutable(loop: boolean, obj: { [name: string]: string | number }, input: string, value: string | number) {
    let key = input;
    obj[key] = value;
    while (loop) obj[key] = value;
    const /*#stableMutable*/stableMutable = obj[key];
    return stableMutable;
}

export function assigned(loop: boolean, obj: { [name: string]: string | number }, input: string, value: string | number) {
    let key = input;
    obj[key] = value;
    while (loop) { key = input; obj[key] = value; }
    const /*#assignedMutable*/assignedMutable = obj[key];
    return assignedMutable;
}

export function nested(loop: boolean, obj: { [name: string]: { value: string | number } }, key: string, value: string | number) {
    obj[key].value = value;
    while (loop) obj[key].value = value;
    const /*#nestedSuffix*/nestedSuffix = (obj)[key]["value"];
    return nestedSuffix;
}

export function named(loop: boolean, obj: { "a.b": string | number; a: { b: string | number } }, value: string | number) {
    obj["a.b"] = value;
    obj.a.b = "value";
    while (loop) { obj["a.b"] = value; obj.a.b = "value"; }
    const /*#namedDots*/namedDots = { warm: obj["a.b"], result: obj.a.b };
    return namedDots;
}

export function binary(loop: boolean, obj: { [name: string]: string | number }, key: string, value: string | number) {
    obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = value;
    obj[key] = "value";
    while (loop) {
        obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = value;
        obj[key] = "value";
    }
    const /*#binaryIdentity*/binaryIdentity = {
        warm: obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"],
        result: /*#binaryRead*/obj[key]
    };
    return binaryIdentity;
}
