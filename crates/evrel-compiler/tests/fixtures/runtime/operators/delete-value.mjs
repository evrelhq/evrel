const events = [];
function value() {
    events.push("value");
    return 42;
}

const result = delete value();
__evrel.observe("delete value", result, events.join(","));
