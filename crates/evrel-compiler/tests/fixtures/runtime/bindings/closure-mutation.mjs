function counter(start) {
    let value = start;
    return {
        read() {
            return value;
        },
        increment() {
            return ++value;
        },
        add(amount) {
            value += amount;
            return value;
        },
    };
}

const first = counter(1);
const second = counter(10);
__evrel.observe(
    "closure mutation",
    first.read(),
    first.increment(),
    first.add(5),
    first.read(),
    second.increment(),
    second.read(),
);
