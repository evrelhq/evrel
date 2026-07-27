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

const postfix = target[key]++;
const prefix = ++target[key];
__evrel.observe("update order", postfix, prefix, target._value, events.join(","));
