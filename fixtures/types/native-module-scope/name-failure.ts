interface Outside { value: 91; }
declare class Carrier { static "\ud800": 0; }
declare namespace Carrier {
    type /*#nameFailure*/Read = Outside["value"];
}
export {};
