function* values() {
    const first = yield 1;
    const second = yield first + 1;
    return second + 1;
}

const iterator = values();
const first = iterator.next();
const second = iterator.next(10);
const third = iterator.next(20);

__evrel.observe(
    "basic generator",
    first.value,
    first.done,
    second.value,
    second.done,
    third.value,
    third.done,
);
