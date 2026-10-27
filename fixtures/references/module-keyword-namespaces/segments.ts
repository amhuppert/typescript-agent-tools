// Names after a dot in a namespace declaration are identifier names, so the
// dependency may use keywords there, escaped or not.
import { /*@A:i*/A, /*@B:i*/B } from "keyword-segments";
/*@A:u*/A./*@default:u*/default./*@n:u*/n;
/*@B:u*/B./*@Bdefault:u*/default./*@class:u*/class./*@m:u*/m;
/*@B:u*/B./*@Bdefault:u*/\u0064efault./*@class:u*/class./*@m:u*/m;
