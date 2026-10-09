// Merges the compiler's document symbols make and the catalog does not. A
// declaration file such as `@tiptap/core`'s repeats one ambient module, and
// `declare global` can repeat too: the compiler merges each later block into
// the first. A property beside two same-named variables merges into both,
// so the compiler lists its contents under each.
declare module "pkg" {
  interface Commands<ReturnType> {
    first: () => ReturnType;
  }
}

declare module "pkg" {
  interface Commands<ReturnType> {
    second: () => ReturnType;
  }
}

declare global {
  interface Window {
    first: string;
  }
}

declare global {
  interface Window {
    second: string;
  }
}

export function publish(kind: string, evidence: { records: { body: string }[] }) {
  let result: unknown;
  if (kind === "restart") {
    const { records } = evidence;
    result = { records: records.map(({ body }) => ({ size: body.length })) };
  } else {
    const { records } = evidence;
    result = { records: records.map(({ body }) => ({ size: body.length })) };
  }
  return result;
}
