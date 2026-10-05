// Class and interface names are symbols; member names are unsupported
// targets, while lexical uses inside members still resolve.
interface /*@Shape*/Shape { /*@!*/area(): number }
interface /*@Named*/Named { /*@!*/name: string }
const /*@computedKey*/computedKey = "key";

abstract class /*@Base*/Base { abstract /*@!*/area(): number; }

class /*@Circle*/Circle extends /*@Base*/Base implements /*@Shape*/Shape, /*@Named*/Named {
  static /*@!*/unit = 1;
  /*@!*/name = "circle";
  /*@!*/radius: number;
  constructor(/*@radius*/radius: number) {
    super();
    this./*@!*/radius = /*@radius*/radius;
  }
  /*@!*/area() {
    const /*@scale*/scale = /*@Circle*/Circle./*@!*/unit;
    return this./*@!*/radius * /*@scale*/scale;
  }
  get /*@!*/diameter() { return this./*@!*/radius * 2; }
  set /*@!*/diameter(/*@diameter*/value: number) { this./*@!*/radius = /*@diameter*/value / 2; }
  [/*@computedKey*/computedKey]() {}
}

class /*@Merged*/Merged {}
interface /*@Merged*/Merged { /*@!*/extra: number }
let /*@merged*/merged: /*@Merged*/Merged = new /*@Merged*/Merged();

function /*@logged*/logged(/*@target*/target: any, /*@context*/context: any) { return /*@target*/target; }

@/*@logged*/logged
class /*@Decorated*/Decorated {
  @/*@logged*/logged /*@!*/method() {}
}

class /*@Counter*/Counter {
  /*@!*/#count = 0;
  /*@!*/#bump() { return ++this./*@!*/#count; }
  static /*@!*/owns(/*@other*/other: object) { return /*@!*/#count in /*@other*/other; }
}
/*@Counter*/Counter./*@!*/owns(new /*@Counter*/Counter());

interface /*@Extended*/Extended extends /*@Shape*/Shape, /*@Named*/Named {}
let /*@shape*/shape: /*@Extended*/Extended = new /*@Circle*/Circle(1);
/*@shape*/shape./*@!*/area();
/*@merged*/merged./*@!*/extra;
new /*@Decorated*/Decorated();
export {};
