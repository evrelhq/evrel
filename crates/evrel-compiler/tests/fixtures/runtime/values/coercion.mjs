const events = [];
const value = {
    [Symbol.toPrimitive](hint) {
        events.push(hint);
        return hint === "number" ? 20 : "value";
    },
};

const numeric = value * 2;
const string = `${value}`;
const concatenated = value + "!";

__evrel.observe("coercion", numeric, string, concatenated, events.join(","));
