const events = [];
function mark(name, value) {
    events.push(name);
    return value;
}

const sequence = (mark("first", 1), mark("second", 2), mark("third", 3));
const taken = mark("condition-true", true)
    ? mark("true-branch", 4)
    : mark("dead-false", 0);
const notTaken = mark("condition-false", false)
    ? mark("dead-true", 0)
    : mark("false-branch", 5);

__evrel.observe("sequence and conditional", sequence, taken, notTaken, events.join(","));
