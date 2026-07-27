const events = [];

function mark(name, value) {
    events.push(name);
    return value;
}

const result = mark("base", 2) ** mark("exponent", 3) ** mark("right", 2);
__evrel.observe("exponentiation", result, events.join(","));
