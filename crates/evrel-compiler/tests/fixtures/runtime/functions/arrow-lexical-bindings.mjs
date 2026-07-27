function makeArrow() {
    const arrow = (...values) => [this.marker, arguments[0], values.length, values[0]];
    return arrow;
}

const receiver = { marker: "receiver", makeArrow };
const arrow = receiver.makeArrow("outer-argument");
const result = arrow.call({ marker: "ignored" }, "inner-argument", "extra");

__evrel.observe(
    "arrow lexical bindings",
    result[0],
    result[1],
    result[2],
    result[3],
    arrow.prototype,
);
