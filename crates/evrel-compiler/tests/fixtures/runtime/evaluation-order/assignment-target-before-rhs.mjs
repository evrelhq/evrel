const events = [];
function base() {
    events.push("base");
    return target;
}
function key() {
    events.push("key-expression");
    return {
        [Symbol.toPrimitive]() {
            events.push("key-coercion");
            return "value";
        },
    };
}
function rhs() {
    events.push("rhs");
    return 42;
}

const target = {};
base()[key()] = rhs();
__evrel.observe("assignment target order", target.value, events.join(","));
