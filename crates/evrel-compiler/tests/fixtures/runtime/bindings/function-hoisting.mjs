const before = hoisted(21);

function hoisted(value) {
    return value * 2;
}

function outer() {
    return inner();
    function inner() {
        return "inner";
    }
}

__evrel.observe("function hoisting", before, outer());
