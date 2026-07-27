const events = [];
const key = {
    [Symbol.toPrimitive]() {
        events.push("key");
        return "value";
    },
};
const target = {
    _value: 1,
    get value() {
        events.push("get");
        return this._value;
    },
    set value(next) {
        events.push(`set:${next}`);
        this._value = next;
    },
};

function rhs(name, value) {
    events.push(name);
    return value;
}

function mutate() {
    events.push("mutate");
    target._value = 10;
}

const sequence = target[key] += (mutate(), rhs("sequence", 2));
__evrel.observe("compound sequence", sequence, target._value, events.join(","));

events.length = 0;
target._value = 5;
const conditional = target.value += true ? rhs("left", 2) : rhs("right", 3);
__evrel.observe("compound conditional", conditional, target._value, events.join(","));

events.length = 0;
target._value = 0;
const logicalWrite = target[key] ||= rhs("logical", 7);
__evrel.observe("logical write", logicalWrite, target._value, events.join(","));

events.length = 0;
target._value = 8;
const logicalSkip = target[key] ||= rhs("unexpected", 9);
__evrel.observe("logical skip", logicalSkip, target._value, events.join(","));

events.length = 0;
target._value = 1;
target.value += true
    ? (false ? rhs("nested-left", 2) : rhs("nested-middle", 3))
    : rhs("nested-right", 4);
__evrel.observe("fallback", target._value, events.join(","));
