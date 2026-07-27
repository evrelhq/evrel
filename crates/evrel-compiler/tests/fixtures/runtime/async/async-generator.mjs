async function* values() {
    yield await Promise.resolve(1);
    yield 2;
    return 3;
}

const iterator = values();
const first = await iterator.next();
const second = await iterator.next();
const third = await iterator.next();

__evrel.observe(
    "async generator",
    first.value,
    first.done,
    second.value,
    second.done,
    third.value,
    third.done,
);
