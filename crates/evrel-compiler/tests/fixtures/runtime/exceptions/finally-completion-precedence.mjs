function returnFromTry() {
    try {
        return 1;
    } finally {
        // A normal finally preserves the pending return.
        void 0;
    }
}

function throwFromFinally() {
    try {
        return 2;
    } finally {
        throw new RangeError("replacement");
    }
}

let errorName;
try {
    throwFromFinally();
} catch (error) {
    errorName = error.name;
}

__evrel.observe("finally precedence", returnFromTry(), errorName);

const controlEvents = [];

outer: for (let index = 0; index < 2; index++) {
    try {
        controlEvents.push(`try:${index}`);
        if (index === 0) continue outer;
        break outer;
    } finally {
        controlEvents.push(`finally:${index}`);
    }
}

try {
    for (;;) {
        controlEvents.push("inner-break");
        break;
    }
    controlEvents.push("after-inner-break");
} finally {
    controlEvents.push("outer-finally");
}

nested: for (;;) {
    try {
        try {
            controlEvents.push("nested-break");
            break nested;
        } finally {
            controlEvents.push("nested-inner-finally");
        }
    } finally {
        controlEvents.push("nested-outer-finally");
    }
}

__evrel.observe("finally control", controlEvents.join(","));
