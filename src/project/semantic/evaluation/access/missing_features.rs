//! `getFeatureMap` from TypeScript 7.0.2 checker/utilities.go. Exact names and
//! entry order are compiler data, independent of configured/loaded libraries.
//! Source table SHA-256: 52083306836997e41d2a6859a03552db08e9197d545b6cf8781e8e80f0db6df7

/// A global's name with each feature library and the members it adds.
type Feature = (
    &'static [u8],
    &'static [(&'static str, &'static [&'static str])],
);

const FEATURES: &[Feature] = &[
    (
        b"Array",
        &[
            (
                "es2015",
                &[
                    "find",
                    "findIndex",
                    "fill",
                    "copyWithin",
                    "entries",
                    "keys",
                    "values",
                ],
            ),
            ("es2016", &["includes"]),
            ("es2019", &["flat", "flatMap"]),
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (b"Iterator", &[("es2015", &[])]),
    (b"AsyncIterator", &[("es2015", &[])]),
    (
        b"ArrayBuffer",
        &[(
            "es2024",
            &[
                "maxByteLength",
                "resizable",
                "resize",
                "detached",
                "transfer",
                "transferToFixedLength",
            ],
        )],
    ),
    (
        b"Atomics",
        &[
            (
                "es2017",
                &[
                    "add",
                    "and",
                    "compareExchange",
                    "exchange",
                    "isLockFree",
                    "load",
                    "or",
                    "store",
                    "sub",
                    "wait",
                    "notify",
                    "xor",
                ],
            ),
            ("es2024", &["waitAsync"]),
        ],
    ),
    (
        b"SharedArrayBuffer",
        &[
            ("es2017", &["byteLength", "slice"]),
            ("es2024", &["growable", "maxByteLength", "grow"]),
        ],
    ),
    (b"AsyncIterable", &[("es2018", &[])]),
    (b"AsyncIterableIterator", &[("es2018", &[])]),
    (b"AsyncGenerator", &[("es2018", &[])]),
    (b"AsyncGeneratorFunction", &[("es2018", &[])]),
    (
        b"RegExp",
        &[
            ("es2015", &["flags", "sticky", "unicode"]),
            ("es2018", &["dotAll"]),
            ("es2024", &["unicodeSets"]),
        ],
    ),
    (b"RegExpConstructor", &[("es2025", &["escape"])]),
    (
        b"Reflect",
        &[(
            "es2015",
            &[
                "apply",
                "construct",
                "defineProperty",
                "deleteProperty",
                "get",
                "getOwnPropertyDescriptor",
                "getPrototypeOf",
                "has",
                "isExtensible",
                "ownKeys",
                "preventExtensions",
                "set",
                "setPrototypeOf",
            ],
        )],
    ),
    (
        b"ArrayConstructor",
        &[("es2015", &["from", "of"]), ("esnext", &["fromAsync"])],
    ),
    (
        b"ObjectConstructor",
        &[
            (
                "es2015",
                &[
                    "assign",
                    "getOwnPropertySymbols",
                    "keys",
                    "is",
                    "setPrototypeOf",
                ],
            ),
            (
                "es2017",
                &["values", "entries", "getOwnPropertyDescriptors"],
            ),
            ("es2019", &["fromEntries"]),
            ("es2022", &["hasOwn"]),
            ("es2024", &["groupBy"]),
        ],
    ),
    (
        b"NumberConstructor",
        &[(
            "es2015",
            &[
                "isFinite",
                "isInteger",
                "isNaN",
                "isSafeInteger",
                "parseFloat",
                "parseInt",
            ],
        )],
    ),
    (
        b"Math",
        &[
            (
                "es2015",
                &[
                    "clz32", "imul", "sign", "log10", "log2", "log1p", "expm1", "cosh", "sinh",
                    "tanh", "acosh", "asinh", "atanh", "hypot", "trunc", "fround", "cbrt",
                ],
            ),
            ("es2025", &["f16round"]),
        ],
    ),
    (
        b"Map",
        &[
            ("es2015", &["entries", "keys", "values"]),
            ("esnext", &["getOrInsert", "getOrInsertComputed"]),
        ],
    ),
    (b"MapConstructor", &[("es2024", &["groupBy"])]),
    (
        b"Set",
        &[
            ("es2015", &["entries", "keys", "values"]),
            (
                "es2025",
                &[
                    "union",
                    "intersection",
                    "difference",
                    "symmetricDifference",
                    "isSubsetOf",
                    "isSupersetOf",
                    "isDisjointFrom",
                ],
            ),
        ],
    ),
    (
        b"PromiseConstructor",
        &[
            ("es2015", &["all", "race", "reject", "resolve"]),
            ("es2020", &["allSettled"]),
            ("es2021", &["any"]),
            ("es2024", &["withResolvers"]),
            ("es2025", &["try"]),
        ],
    ),
    (
        b"Symbol",
        &[("es2015", &["for", "keyFor"]), ("es2019", &["description"])],
    ),
    (
        b"WeakMap",
        &[
            ("es2015", &[]),
            ("esnext", &["getOrInsert", "getOrInsertComputed"]),
        ],
    ),
    (b"WeakSet", &[("es2015", &[])]),
    (
        b"String",
        &[
            (
                "es2015",
                &[
                    "codePointAt",
                    "includes",
                    "endsWith",
                    "normalize",
                    "repeat",
                    "startsWith",
                    "anchor",
                    "big",
                    "blink",
                    "bold",
                    "fixed",
                    "fontcolor",
                    "fontsize",
                    "italics",
                    "link",
                    "small",
                    "strike",
                    "sub",
                    "sup",
                ],
            ),
            ("es2017", &["padStart", "padEnd"]),
            ("es2019", &["trimStart", "trimEnd", "trimLeft", "trimRight"]),
            ("es2020", &["matchAll"]),
            ("es2021", &["replaceAll"]),
            ("es2022", &["at"]),
            ("es2024", &["isWellFormed", "toWellFormed"]),
        ],
    ),
    (
        b"StringConstructor",
        &[("es2015", &["fromCodePoint", "raw"])],
    ),
    (b"DateTimeFormat", &[("es2017", &["formatToParts"])]),
    (b"Promise", &[("es2015", &[]), ("es2018", &["finally"])]),
    (b"RegExpMatchArray", &[("es2018", &["groups"])]),
    (b"RegExpExecArray", &[("es2018", &["groups"])]),
    (
        b"Intl",
        &[
            ("es2018", &["PluralRules"]),
            ("es2020", &["RelativeTimeFormat", "Locale", "DisplayNames"]),
            ("es2021", &["ListFormat", "DateTimeFormat"]),
            ("es2022", &["Segmenter"]),
            ("es2025", &["DurationFormat"]),
        ],
    ),
    (b"NumberFormat", &[("es2018", &["formatToParts"])]),
    (
        b"SymbolConstructor",
        &[
            ("es2020", &["matchAll"]),
            ("esnext", &["metadata", "dispose", "asyncDispose"]),
        ],
    ),
    (
        b"DataView",
        &[
            (
                "es2020",
                &["setBigInt64", "setBigUint64", "getBigInt64", "getBigUint64"],
            ),
            ("es2025", &["setFloat16", "getFloat16"]),
        ],
    ),
    (b"BigInt", &[("es2020", &[])]),
    (
        b"RelativeTimeFormat",
        &[("es2020", &["format", "formatToParts", "resolvedOptions"])],
    ),
    (
        b"Int8Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Uint8Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Uint8ClampedArray",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Int16Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Uint16Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Int32Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Uint32Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (b"Float16Array", &[("es2025", &[])]),
    (
        b"Float32Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"Float64Array",
        &[
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"BigInt64Array",
        &[
            ("es2020", &[]),
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (
        b"BigUint64Array",
        &[
            ("es2020", &[]),
            ("es2022", &["at"]),
            (
                "es2023",
                &[
                    "findLastIndex",
                    "findLast",
                    "toReversed",
                    "toSorted",
                    "toSpliced",
                    "with",
                ],
            ),
        ],
    ),
    (b"Error", &[("es2022", &["cause"])]),
    (b"ErrorConstructor", &[("esnext", &["isError"])]),
    (
        b"Uint8ArrayConstructor",
        &[("esnext", &["fromBase64", "fromHex"])],
    ),
    (b"DisposableStack", &[("esnext", &[])]),
    (b"AsyncDisposableStack", &[("esnext", &[])]),
    (b"Date", &[("esnext", &["toTemporalInstant"])]),
];

/// `getSuggestedLibForNonExistentProperty`'s pure feature-map lookup. The
/// caller owns the preceding apparent-type and actual Symbol.Name acquisition.
pub(super) fn library_for_property(container: &[u8], property: &str) -> Option<&'static str> {
    FEATURES
        .iter()
        .find(|(name, _)| *name == container)?
        .1
        .iter()
        .find(|(_, properties)| properties.contains(&property))
        .map(|(library, _)| *library)
}
