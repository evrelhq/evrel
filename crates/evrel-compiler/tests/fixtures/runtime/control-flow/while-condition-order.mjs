const events = [];
let value = 0;
function condition() {
    events.push(`condition:${value}`);
    return value < 3;
}

while (condition()) {
    events.push(`body:${value}`);
    value++;
}

__evrel.observe("while condition", value, events.join(","));
