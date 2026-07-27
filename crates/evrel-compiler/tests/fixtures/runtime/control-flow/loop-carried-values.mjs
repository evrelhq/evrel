function swap() {
    let left = 1;
    let right = 2;
    let count = 0;

    while (count < 3) {
        const temporary = left;
        left = right;
        right = temporary;
        count = count + 1;
    }

    return left * 10 + right;
}

__evrel.observe("loop-carried swap", swap());

function varLoop(limit) {
    let iterations = 0;
    for (var index = 1; index < limit; index++) {
        iterations++;
    }
    return `${iterations}:${index}`;
}

function _arrayLikeToArray(array, length) {
    if (length == null || length > array.length) length = array.length;
    for (var index = 0, copy = new Array(length); index < length; index++) {
        copy[index] = array[index];
    }
    return copy;
}

function _extends(target) {
    for (var index = 1; index < arguments.length; index++) {
        var source = arguments[index];
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                target[key] = source[key];
            }
        }
    }
    return target;
}

__evrel.observe("var induction", varLoop(5));
__evrel.observe("Babel array-like helper", _arrayLikeToArray([1, 2, 3], 2).join(","));
const extended = _extends({}, { first: 1 }, { second: 2 });
__evrel.observe("Babel extends helper", extended.first, extended.second);

function containsWord(attribute, value) {
    let index;
    for (let start = 0; true; start = index + 1) {
        index = attribute.indexOf(value, start);
        if (index === -1) return false;

        const first = index === 0 || attribute[index - 1] === " ";
        const last =
            index + value.length === attribute.length ||
            attribute[index + value.length] === " ";
        if (first && last) return true;
    }
}

__evrel.observe(
    "constant for test",
    containsWord("one two three", "two"),
    containsWord("one two three", "four"),
);

function decrementInShortCircuitTest(limit) {
    let remaining = 4;
    let iterations = 0;

    for (; iterations < limit && --remaining > 0; iterations++) {}

    return `${iterations}:${remaining}`;
}

__evrel.observe(
    "short-circuit test temporaries",
    decrementInShortCircuitTest(10),
);
