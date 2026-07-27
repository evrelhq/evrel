const first = Symbol("first");
const second = Symbol.for("second");
const object = {
    [first]: 1,
    [second]: 2,
    first: 3,
};

__evrel.observe(
    "symbol properties",
    object[first],
    object[second],
    object.first,
    Object.getOwnPropertySymbols(object).length,
    first in object,
    delete object[first],
    first in object,
);
