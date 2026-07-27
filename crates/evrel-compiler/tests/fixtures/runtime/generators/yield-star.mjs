function* inner() {
    yield 1;
    yield 2;
    return 3;
}

function* outer() {
    const result = yield* inner();
    yield result + 1;
}

const values = [...outer()];
__evrel.observe("yield star", values.length, values[0], values[1], values[2]);
