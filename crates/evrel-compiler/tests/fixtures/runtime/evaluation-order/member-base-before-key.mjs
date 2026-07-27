const events = [];
function base() {
    events.push("base");
    return { value: 42 };
}
function key() {
    events.push("key");
    return "value";
}

const result = base()[key()];
__evrel.observe("member order", result, events.join(","));
