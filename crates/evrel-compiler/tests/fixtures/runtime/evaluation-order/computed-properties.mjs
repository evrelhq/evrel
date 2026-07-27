const events = [];
const key = {
    [Symbol.toPrimitive]() {
        events.push("key");
        return "value";
    },
};

function rhs(value) {
    events.push("rhs");
    return value;
}

function takeEvents() {
    const result = events.join(",");
    events.length = 0;
    return result;
}

let target = { value: 1 };
const read = target[key];
__evrel.observe("read", read, target.value, takeEvents());

target = {
    value() {
        events.push("call");
        return this === target;
    },
};
const call = target[key]();
__evrel.observe("call", call, takeEvents());

target = { value: 1 };
const deleted = delete target[key];
__evrel.observe("delete", deleted, "value" in target, takeEvents());

target = { value: 1 };
const assigned = target[key] = true ? rhs(2) : rhs(0);
__evrel.observe("assign", assigned, target.value, takeEvents());

target = { value: 1 };
const compound = target[key] += rhs(2);
__evrel.observe("compound", compound, target.value, takeEvents());

target = { value: 1 };
const updated = target[key]++;
__evrel.observe("update", updated, target.value, takeEvents());

target = { value: 1 };
const logical = target[key] &&= rhs(7);
__evrel.observe("logical taken", logical, target.value, takeEvents());

target = { value: 0 };
const shortCircuited = target[key] &&= rhs(9);
__evrel.observe("logical skipped", shortCircuited, target.value, takeEvents());
