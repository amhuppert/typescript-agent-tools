interface Ranges {
  /*#plain*/"plain": number;
  /*#escaped*/"e\u0073caped": number;
  /*#astral*/"😀": number;
  /*#empty*/"": number;
  /*#number*/16: number;
}
declare const range: Ranges;
range["plain"];
range[`plain`];
range.plain;
range["escaped"];
range[`escaped`];
range["😀"];
range[`😀`];
range[""];
range[``];
range[16];
range[0x10];
