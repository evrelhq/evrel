var first;
var second;
var rest;
var [first, second = 2, ...rest] = [1, undefined, 3, 4];
var iterations = 0;

for (
    var { value: loopObject } = { value: 5 }, loopArray = [6];
    iterations < 1;
    iterations += 1
) {
    __evrel.observe(
        "var destructuring for initializer",
        loopObject,
        loopArray[0],
    );
}

__evrel.observe(
    "var destructuring store",
    first,
    second,
    rest.length,
    rest[0],
    rest[1],
);
__evrel.observe(
    "var destructuring for scope",
    loopObject,
    loopArray[0],
    iterations,
);
