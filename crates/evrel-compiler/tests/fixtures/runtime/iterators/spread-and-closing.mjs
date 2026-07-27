const events = [];

function iterable(limit) {
    return {
        [Symbol.iterator]() {
            let value = 0;
            return {
                next() {
                    events.push(`next:${value}`);
                    return value < limit
                        ? { value: value++, done: false }
                        : { value: undefined, done: true };
                },
                return() {
                    events.push("return");
                    return { value: undefined, done: true };
                },
            };
        },
    };
}

const spread = [...iterable(3)];
for (const value of iterable(4)) {
    if (value === 1) break;
}

__evrel.observe(
    "iterators",
    spread.length,
    spread[0],
    spread[1],
    spread[2],
    events.join(","),
);
