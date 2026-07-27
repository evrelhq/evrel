function strictFunction() {
    "use strict";
    return this;
}

__evrel.observe(
    "nested strict directive",
    strictFunction(),
    strictFunction.call(null),
    strictFunction.call(1),
);
