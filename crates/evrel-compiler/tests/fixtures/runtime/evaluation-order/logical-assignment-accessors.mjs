const events = [];
function target(initial) {
    return {
        _value: initial,
        get value() {
            events.push(`get:${this._value}`);
            return this._value;
        },
        set value(next) {
            events.push(`set:${next}`);
            this._value = next;
        },
    };
}
function rhs(value) {
    events.push(`rhs:${value}`);
    return value;
}

const andTaken = target(1);
andTaken.value &&= rhs(2);
const andSkipped = target(0);
andSkipped.value &&= rhs(3);
const orTaken = target(0);
orTaken.value ||= rhs(4);
const orSkipped = target(1);
orSkipped.value ||= rhs(5);
const nullishTaken = target(null);
nullishTaken.value ??= rhs(6);
const nullishSkipped = target(false);
nullishSkipped.value ??= rhs(7);

__evrel.observe(
    "logical assignment accessors",
    andTaken._value,
    andSkipped._value,
    orTaken._value,
    orSkipped._value,
    nullishTaken._value,
    nullishSkipped._value,
    events.join(","),
);
