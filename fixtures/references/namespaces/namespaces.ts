// Namespace blocks merge and share exported declarations; non-exported locals
// stay in their own block. Qualified names resolve through export tables.
namespace /*@Outer*/Outer {
  export const /*@shared*/shared = 1;
  const /*@hidden1*/hidden = 2;
  /*@hidden1*/hidden;
  export namespace /*@Inner*/Inner {
    export type /*@Deep*/Deep = number;
    export const /*@deepValue*/deepValue = /*@shared*/shared;
  }
}

namespace /*@Outer*/Outer {
  /*@shared*/shared;
  const /*@hidden2*/hidden = 3;
  /*@hidden2*/hidden;
  /*@Inner*/Inner./*@deepValue*/deepValue;
}

namespace /*@A*/A./*@B*/B./*@C*/C {
  export const /*@leaf*/leaf = 1;
  export interface /*@Leaf*/Leaf {}
}

let /*@deep*/deep: /*@Outer*/Outer./*@Inner*/Inner./*@Deep*/Deep = /*@Outer*/Outer./*@Inner*/Inner./*@deepValue*/deepValue;
let /*@leafType*/leafType: /*@A*/A./*@B*/B./*@C*/C./*@Leaf*/Leaf = {};
/*@A*/A./*@B*/B./*@C*/C./*@leaf*/leaf;
type /*@SharedType*/SharedType = typeof /*@Outer*/Outer./*@shared*/shared;

function /*@merged*/merged() {}
namespace /*@merged*/merged { export const /*@prop*/prop = 1; }
/*@merged*/merged./*@prop*/prop;

class /*@Klass*/Klass {}
namespace /*@Klass*/Klass { export const /*@staticLike*/staticLike = 1; }
/*@Klass*/Klass./*@staticLike*/staticLike;

import /*@Inner*/Alias = /*@Outer*/Outer./*@Inner*/Inner;
/*@Inner*/Alias./*@deepValue*/deepValue;
let /*@aliased*/aliased: /*@Inner*/Alias./*@Deep*/Deep = /*@deep*/deep;
/*@leafType*/leafType; /*@aliased*/aliased;
