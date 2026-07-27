const events = [];
function mark(name, value) {
    events.push(name);
    return value;
}

const falsyAnd = mark("and-left", 0) && mark("and-right", 1);
const truthyAnd = mark("and2-left", "left") && mark("and2-right", "right");
const truthyOr = mark("or-left", "left") || mark("or-right", "right");
const falsyOr = mark("or2-left", "") || mark("or2-right", "right");
const definedNullish = mark("nullish-left", 0) ?? mark("nullish-right", 1);
const nullNullish = mark("nullish2-left", null) ?? mark("nullish2-right", 2);

__evrel.observe(
    "logical values",
    falsyAnd,
    truthyAnd,
    truthyOr,
    falsyOr,
    definedNullish,
    nullNullish,
    events.join(","),
);
