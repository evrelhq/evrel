const expression = /a+/gy;
expression.lastIndex = 1;
const first = expression.exec("baaaac");
const afterFirst = expression.lastIndex;
const second = expression.exec("baaaac");

__evrel.observe(
    "regexp state",
    first[0],
    first.index,
    afterFirst,
    second,
    expression.lastIndex,
);
