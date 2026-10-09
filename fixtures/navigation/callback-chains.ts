// Callback labels name the callee as TypeScript's getCallExpressionName
// does: identifiers (decoded), private names, and property-access names,
// joined with `.`; any other left part (`this`, `super`, a call, an element
// access, or parentheses) is dropped with the `.` after it.
declare const chain: any;
declare function make(): any;

class Base {
  run(callback: () => void) {}
}
class Holder extends Base {
  #queue: any;
  tasks: any;
  start() {
    this.#queue.push(() => {});
    this.tasks.add(() => {});
    super.run(() => {});
  }
}

chain.a.b(() => {});
chain[0].b(() => {});
make().then(() => {});
(chain).b(() => {});
chain.e(() => {});
chain.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.aa.bb.cc.dd.ee.ff.gg.hh.ii.jj.kk.ll.mm.nn(() => {});
chain.key('x', `y`, 1)(() => {});
chain.each('x', `y ${chain}`, 1, () => {});
ch\u0061in.\u{65}ach(() => {});
export {};
