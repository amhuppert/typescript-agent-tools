export function collision(loop: boolean, obj: { [name: string]: string | number }, key: string) {
    obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = 0;
    obj[key] = "value";
    while (loop) {
        obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = 0;
        obj[key] = "value";
    }
    const /*#answer*/answer = {
        warm: obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"],
        result: obj[key]
    };
    return answer;
}
