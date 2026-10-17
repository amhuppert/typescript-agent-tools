export function /*#top*/top(): typeof top { return top; }
function /*#privateTop*/privateTop(): typeof privateTop { return privateTop; }
export function containing() {
    function /*#local*/local(): typeof local { return local; }
    return local;
}
export namespace Space {
    export function /*#member*/member(): typeof member { return member; }
}
export let /*#arrow*/arrow = () => arrow;
export let /*#expression*/expression = function (): typeof expression { return expression; };
export function /*#escaped*/\u0052aw(): typeof Raw { return Raw; }
