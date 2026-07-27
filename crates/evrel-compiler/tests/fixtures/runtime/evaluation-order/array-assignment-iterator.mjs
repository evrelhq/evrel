const events = [];
const iterable = {
    [Symbol.iterator]() {
        let value = 1;
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

let first;
let second;
[first, second] = iterable;
__evrel.observe("array assignment iterator", first, second, events.join(","));
