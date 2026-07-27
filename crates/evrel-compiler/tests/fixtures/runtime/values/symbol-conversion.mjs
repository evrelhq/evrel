const symbol = Symbol("value");
let implicitName;
try {
    void (symbol + "");
} catch (error) {
    implicitName = error.name;
}

__evrel.observe(
    "symbol conversion",
    symbol.description,
    String(symbol),
    Boolean(symbol),
    implicitName,
    Symbol.for("shared") === Symbol.for("shared"),
);
