const object = {};
const same = object;

__evrel.observe(
    "comparison and equality",
    1 == "1",
    1 === "1",
    null == undefined,
    null === undefined,
    NaN === NaN,
    Object.is(NaN, NaN),
    Object.is(0, -0),
    object === same,
    object === {},
    "10" < "2",
    "10" < 2,
    3n == 3,
    3n === 3,
);
