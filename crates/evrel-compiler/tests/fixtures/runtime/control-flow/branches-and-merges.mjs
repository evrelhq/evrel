function classify(value) {
    let result;
    if (value < 0) {
        result = "negative";
    } else if (value === 0) {
        result = "zero";
    } else {
        result = value % 2 === 0 ? "positive-even" : "positive-odd";
    }
    return result;
}

function selected(value) {
    switch (value) {
        case 1:
            return "one";
        case 2:
        case 3:
            return "few";
        default:
            return "many";
    }
}

const logicalOr = 0 || 4;
const logicalAnd = 5 && 6;
const nullish = null ?? 7;

__evrel.observe(
    "branches",
    classify(-1),
    classify(0),
    classify(2),
    classify(3),
    selected(1),
    selected(3),
    selected(8),
    logicalOr,
    logicalAnd,
    nullish,
);
