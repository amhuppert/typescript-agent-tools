// Class and interface names are symbols; member names are unsupported
// targets, while lexical uses inside members still resolve.
interface /*@Shape:d*/Shape { /*@!*/area(): number }
interface /*@Named:d*/Named { /*@!*/name: string }
const /*@computedKey:d*/computedKey = "key";

abstract class /*@Base:d*/Base { abstract /*@!*/area(): number; }

class /*@Circle:d*/Circle extends /*@Base:u*/Base implements /*@Shape:u*/Shape, /*@Named:u*/Named {
  static /*@!*/unit = 1;
  /*@!*/name = "circle";
  /*@!*/radius: number;
  constructor(/*@radius:d*/radius: number) {
    super();
    this./*@!*/radius = /*@radius:u*/radius;
  }
  /*@!*/area() {
    const /*@scale:d*/scale = /*@Circle:u*/Circle./*@!*/unit;
    return this./*@!*/radius * /*@scale:u*/scale;
  }
  get /*@!*/diameter() { return this./*@!*/radius * 2; }
  set /*@!*/diameter(/*@diameter:d*/value: number) { this./*@!*/radius = /*@diameter:u*/value / 2; }
  [/*@computedKey:u*/computedKey]() {}
}

class /*@Merged:d*/Merged {}
interface /*@Merged:d*/Merged { /*@!*/extra: number }
let /*@merged:d*/merged: /*@Merged:u*/Merged = new /*@Merged:u*/Merged();

function /*@logged:d*/logged(/*@target:d*/target: any, /*@context:d*/context: any) { return /*@target:u*/target; }

@/*@logged:u*/logged
class /*@Decorated:d*/Decorated {
  @/*@logged:u*/logged /*@!*/method() {}
}

class /*@Counter:d*/Counter {
  /*@!*/#count = 0;
  /*@!*/#bump() { return ++this./*@!*/#count; }
  static /*@!*/owns(/*@other:d*/other: object) { return /*@!*/#count in /*@other:u*/other; }
}
/*@Counter:u*/Counter./*@!*/owns(new /*@Counter:u*/Counter());

interface /*@Extended:d*/Extended extends /*@Shape:u*/Shape, /*@Named:u*/Named {}
let /*@shape:d*/shape: /*@Extended:u*/Extended = new /*@Circle:u*/Circle(1);
/*@shape:u*/shape./*@!*/area();
/*@merged:u*/merged./*@!*/extra;
new /*@Decorated:u*/Decorated();
export {};

// Class, method, and parameter decorators resolve where the class is declared.
function /*@record:d*/record(.../*@records:d*/records: unknown[]) { /*@records:u*/records; }
@/*@record:u*/record class /*@Recorded:d*/Recorded { @/*@record:u*/record /*@!*/handle(@/*@record:u*/record /*@input:d*/input: number) { /*@input:u*/input; } }
/*@Recorded:u*/Recorded;
