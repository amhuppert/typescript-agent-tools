// Namespace blocks merge and share exported declarations; non-exported locals
// stay in their own block. Qualified names resolve through export tables.
namespace /*@Outer:d*/Outer {
  export const /*@shared:de*/shared = 1;
  const /*@hidden1:d*/hidden = 2;
  /*@hidden1:u*/hidden;
  export namespace /*@Inner:de*/Inner {
    export type /*@Deep:de*/Deep = number;
    export const /*@deepValue:de*/deepValue = /*@shared:u*/shared;
  }
}

namespace /*@Outer:d*/Outer {
  /*@shared:u*/shared;
  const /*@hidden2:d*/hidden = 3;
  /*@hidden2:u*/hidden;
  /*@Inner:u*/Inner./*@deepValue:u*/deepValue;
}

namespace /*@A:d*/A./*@B:de*/B./*@C:de*/C {
  export const /*@leaf:de*/leaf = 1;
  export interface /*@Leaf:de*/Leaf {}
}

let /*@deep:d*/deep: /*@Outer:u*/Outer./*@Inner:u*/Inner./*@Deep:u*/Deep = /*@Outer:u*/Outer./*@Inner:u*/Inner./*@deepValue:u*/deepValue;
let /*@leafType:d*/leafType: /*@A:u*/A./*@B:u*/B./*@C:u*/C./*@Leaf:u*/Leaf = {};
/*@A:u*/A./*@B:u*/B./*@C:u*/C./*@leaf:u*/leaf;
type /*@SharedType:d*/SharedType = typeof /*@Outer:u*/Outer./*@shared:u*/shared;

function /*@merged:d*/merged() {}
namespace /*@merged:d*/merged { export const /*@prop:de*/prop = 1; }
/*@merged:u*/merged./*@prop:u*/prop;

class /*@Klass:d*/Klass {}
namespace /*@Klass:d*/Klass { export const /*@staticLike:de*/staticLike = 1; }
/*@Klass:u*/Klass./*@staticLike:u*/staticLike;

import /*@Inner:i*/Alias = /*@Outer:u*/Outer./*@Inner:u*/Inner;
/*@Inner:u*/Alias./*@deepValue:u*/deepValue;
let /*@aliased:d*/aliased: /*@Inner:u*/Alias./*@Deep:u*/Deep = /*@deep:u*/deep;
/*@leafType:u*/leafType; /*@aliased:u*/aliased;

// A merged block cannot see another block's locals.
namespace /*@Split:d*/Split { const /*@firstLocal:d*/firstLocal = 1; /*@firstLocal:u*/firstLocal; export const /*@exported:de*/exported = 2; }
namespace /*@Split:d*/Split { /*@?*/firstLocal; /*@exported:u*/exported; }

// An import alias of an entity name needs a namespace meaning.
const /*@plain:d*/plain = 1;
import /*@?*/notNamespace = /*@?*/plain;
/*@?*/notNamespace; /*@plain:u*/plain;
namespace /*@Holder:d*/Holder { export function /*@held:de*/held() {} }
import /*@held:i*/heldAlias = /*@Holder:u*/Holder./*@held:u*/held;
import /*@Holder:i*/HolderAlias = /*@Holder:u*/Holder;
/*@held:u*/heldAlias(); /*@Holder:u*/HolderAlias./*@held:u*/held();

// A member of an unresolved value is a property; an unresolved type
// qualifier leaves its member unresolved.
let /*@unknownValue:d*/unknownValue: typeof /*@?*/Missing./*@!*/member;
let /*@unknownType:d*/unknownType: /*@?*/Missing./*@?*/Type;
/*@unknownValue:u*/unknownValue; /*@unknownType:u*/unknownType;
