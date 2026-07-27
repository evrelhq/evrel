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
