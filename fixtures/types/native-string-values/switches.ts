export {};
function distinct(value: string | object) {
    switch (typeof value) {
        case "\uD800\
\uDC00": break;
        case "𐀀": return /*#distinctSecond*/value;
    }
}
function reversed(value: string | object) {
    switch (typeof value) {
        case "𐀀": break;
        case "\uD800\
\uDC00": return /*#continuedSecond*/value;
    }
}
function paired(value: string | object) {
    switch (typeof value) {
        case "\uD800\uDC00": break;
        case "𐀀": return /*#pairedDuplicate*/value;
    }
}
function halves(value: string | object) {
    switch (typeof value) {
        case "\uD800": break;
        case "\uDC00": return /*#distinctHalves*/value;
    }
}
function standard(value: string | object) {
    switch (typeof value) {
        case "\u0073tring": return /*#standardEscape*/value;
    }
}

function inertDistinct(value: object) {
    switch (typeof value) {
        case "\uD800\
\uDC00": break;
        case "𐀀": return /*#inertDistinctSecond*/value;
    }
}
function inertReversed(value: object) {
    switch (typeof value) {
        case "𐀀": break;
        case "\uD800\
\uDC00": return /*#inertContinuedSecond*/value;
    }
}
function inertPaired(value: object) {
    switch (typeof value) {
        case "\uD800\uDC00": break;
        case "𐀀": return /*#inertPairedDuplicate*/value;
    }
}
function inertHalves(value: object) {
    switch (typeof value) {
        case "\uD800": break;
        case "\uDC00": return /*#inertDistinctHalves*/value;
    }
}
function inertStandard(value: string) {
    switch (typeof value) {
        case "\u0073tring": return /*#inertStandardEscape*/value;
    }
}
