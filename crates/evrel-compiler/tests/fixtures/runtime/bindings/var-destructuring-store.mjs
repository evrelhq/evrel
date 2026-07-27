var first;
var second;
var rest;
var [first, second = 2, ...rest] = [1, undefined, 3, 4];

__evrel.observe(
    "var destructuring store",
    first,
    second,
    rest.length,
    rest[0],
    rest[1],
);
