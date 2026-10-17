namespace Across {
    export function read() { return /*@transientVariable*/value; }
    export function closure() { return () => /*@transientClosure*/value; }
}
const /*#transientVariable*/transientVariable = Across.read();
const /*#transientClosure*/transientClosure = Across.closure()();
