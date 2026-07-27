const events = [];
const iterable = {
    [Symbol.asyncIterator]() {
        let value = 0;
        return {
            async next() {
                events.push(`next:${value}`);
                return value < 3
                    ? { value: value++, done: false }
                    : { value: undefined, done: true };
            },
            async return() {
                events.push("return");
                return { done: true };
            },
        };
    },
};

const values = [];
for await (const value of iterable) {
    values.push(value);
    if (value === 1) break;
}

__evrel.observe("for await of", values.join(","), events.join(","));
