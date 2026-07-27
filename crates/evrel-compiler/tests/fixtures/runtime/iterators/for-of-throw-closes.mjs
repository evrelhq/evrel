const events = [];
const iterable = {
    [Symbol.iterator]() {
        let value = 0;
        return {
            next() {
                events.push(`next:${value}`);
                return { value: value++, done: false };
            },
            return() {
                events.push("return");
                return { done: true };
            },
        };
    },
};

let name;
try {
    for (const value of iterable) {
        events.push(`body:${value}`);
        throw new TypeError("stop");
    }
} catch (error) {
    name = error.name;
}

__evrel.observe("for of throw closes", name, events.join(","));
