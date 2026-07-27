const localSymbol = Symbol("local");
const otherSymbol = Symbol("local");
const sharedObject = {};

__evrel.observe(
    "value encoding",
    undefined,
    null,
    false,
    "text",
    NaN,
    -0,
    Infinity,
    -Infinity,
    42n,
    localSymbol,
    localSymbol,
    otherSymbol,
    Symbol.for("global"),
    sharedObject,
    sharedObject,
    {},
);
