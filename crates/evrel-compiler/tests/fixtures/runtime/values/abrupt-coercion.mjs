const events = [];
const value = {
    [Symbol.toPrimitive](hint) {
        events.push(hint);
        throw new TypeError("coercion failed");
    },
};

let name;
try {
    void (value + 1);
} catch (error) {
    name = error.name;
}

__evrel.observe("abrupt coercion", name, events.join(","));
