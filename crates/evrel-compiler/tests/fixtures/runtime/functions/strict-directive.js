"use strict";

function ordinary() {
    return this;
}

__evrel.observe(
    "strict this",
    this === globalThis,
    ordinary(),
    ordinary.call(null),
    ordinary.call(1),
);
