export function optionalIdentity(): void {
    const unchanged: number = 1;
    let other: number = 2;
    let candidate: any;
    candidate?.plain;
    /*after-property-chain*/ unchanged;
    candidate?.[other = 3];
    /*after-element-chain*/ unchanged;
    candidate?.(other = 4);
    /*after-call-chain*/ unchanged;
    candidate?.plain!.value;
    /*after-nonnull-chain*/ unchanged;
    candidate?.plain?.next;
    /*after-nested-chain-roots*/ unchanged;
    (candidate?.plain).value;
    /*after-parenthesized-chain*/ unchanged;
    if (candidate?.plain) {
        /*inside-property-chain-true*/ unchanged;
    } else {
        /*inside-property-chain-false*/ unchanged;
    }
    if (!candidate?.plain) {
        /*inside-negated-chain*/ unchanged;
    }
    candidate?.[typeof other === "number" && (other = 5)];
    /*after-logical-chain-argument*/ unchanged;
    candidate?.[false || (other = 6)];
    /*after-constant-chain-argument*/ unchanged;
    candidate?.(candidate?.plain).value;
    /*after-chain-inside-argument*/ unchanged;
}
export function pureOptionalIdentity(): void {
    let candidate: any;
    candidate?.plain;
    const value: number = 1;
    /*after-pure-property-chain*/ value;
}
