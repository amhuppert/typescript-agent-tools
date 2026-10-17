declare const input: string | number;
class FlowSource {
  get value() {
    if (typeof input === "string") return input;
    return 0;
  }
}
declare const dependency: FlowSource;
