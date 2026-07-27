const value = "outer";

function makeClosure(input) {
    const captured = input;
    return function readCaptured(suffix) {
        const value = "inner";
        return `${captured}:${value}:${suffix}`;
    };
}

const closure = makeClosure("captured");
__evrel.observe("closures and shadowing", value, closure("result"));
