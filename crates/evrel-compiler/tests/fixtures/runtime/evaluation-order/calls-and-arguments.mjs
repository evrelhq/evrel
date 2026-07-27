const events = [];

function argument(name, value) {
    events.push(name);
    return value;
}

const receiver = {
    base: 40,
    method(first, second) {
        events.push("body");
        return this.base + first + second;
    },
};

const result = receiver.method(argument("first", 1), ...[argument("second", 1)]);
__evrel.observe("method call", result, events.join(","));
