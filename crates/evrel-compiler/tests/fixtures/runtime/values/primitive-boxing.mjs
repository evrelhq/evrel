const string = "abc";
const number = 42;
const boolean = true;
const symbol = Symbol("value");

__evrel.observe(
    "primitive boxing",
    string.length,
    string[1],
    number.toFixed(1),
    boolean.valueOf(),
    symbol.description,
    Object(string) instanceof String,
    Object(number) instanceof Number,
);
