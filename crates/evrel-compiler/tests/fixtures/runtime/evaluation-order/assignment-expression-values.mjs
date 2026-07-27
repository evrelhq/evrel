let value = 1;
const simple = value = 2;
const add = value += 3;
const postfix = value++;
const prefix = ++value;
let logicalValue = 0;
const logical = logicalValue ||= 8;

__evrel.observe(
    "assignment values",
    simple,
    add,
    postfix,
    prefix,
    value,
    logical,
    logicalValue,
);
