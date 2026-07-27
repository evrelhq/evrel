const events = [];
function mark(name, value) {
    events.push(name);
    return value;
}

const source = { first: 1, missing: undefined, rest: 3 };
const {
    [mark("key", "first")]: first,
    missing = mark("default", 2),
    ...remaining
} = source;

const values = [mark("element", 4), undefined, 6, 7];
const [head, second = mark("array-default", 5), ...tail] = values;

__evrel.observe(
    "destructuring",
    first,
    missing,
    remaining.rest,
    head,
    second,
    tail.length,
    tail[0],
    tail[1],
    events.join(","),
);
