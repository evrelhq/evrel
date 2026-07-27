const events = [];
const iterable = {
    [Symbol.iterator]() {
        return {
            next() {
                events.push("next");
                return { value: undefined, done: false };
            },
            return() {
                events.push("return");
                return { done: true };
            },
        };
    },
};

function fail() {
    events.push("default");
    throw new RangeError("abrupt");
}

let name;
try {
    const [value = fail()] = iterable;
    void value;
} catch (error) {
    name = error.name;
}

__evrel.observe("abrupt destructuring close", name, events.join(","));
