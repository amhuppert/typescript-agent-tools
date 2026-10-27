// The dependency declares namespaces with the `module` keyword, which
// TypeScript 7 parses as namespaces; skipLibCheck leaves its TS1540 unreported.
import /*@emailAddresses:i*/address, { /*@Options:i*/Options, /*@parseOneAddress:i*/parseOneAddress } from "email-addresses";
import * as /*@emailAddresses:i*/addresses from "email-addresses";

const /*@options:d*/options: /*@Options:u*/Options = { /*@!*/input: "a@b.c" };
/*@emailAddresses:u*/address(/*@options:u*/options);
/*@parseOneAddress:u*/parseOneAddress(/*@options:u*/options);
/*@emailAddresses:u*/addresses./*@parseOneAddress:u*/parseOneAddress("a@b.c");
let /*@typed:d*/typed: /*@emailAddresses:u*/addresses./*@Options:u*/Options = /*@options:u*/options;
/*@typed:u*/typed;
/*@Outer:u*/Outer./*@Inner:u*/Inner./*@depth:u*/depth;
