const generator = function* (value) {
    yield value;
    return value + 1;
};
const asynchronous = async function (value) {
    await null;
    return value + 2;
};
const asyncArrow = async (value) => {
    await null;
    return value + 3;
};

const iterator = generator(40);
const first = iterator.next();
const second = iterator.next();
__evrel.observe(
    "function expression kinds",
    first.value,
    first.done,
    second.value,
    second.done,
    await asynchronous(40),
    await asyncArrow(40),
);
