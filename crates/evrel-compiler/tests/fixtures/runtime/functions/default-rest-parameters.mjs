const events = [];
function mark(name, value) {
    events.push(name);
    return value;
}

function collect(first = mark("first-default", 1), second = mark("second-default", first + 1), ...rest) {
    return [first, second, rest.length, rest[0], rest[1]];
}

const defaults = collect(undefined, undefined, 3, 4);
const supplied = collect(mark("first-value", 5), mark("second-value", 6), 7);

__evrel.observe("parameter defaults", ...defaults, ...supplied, events.join(","));
