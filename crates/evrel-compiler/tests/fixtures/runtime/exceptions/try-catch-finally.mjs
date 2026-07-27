const events = [];

function recover() {
    try {
        events.push("try");
        throw new TypeError("original message is not an oracle");
    } catch (error) {
        events.push(`catch:${error.name}`);
        return 40;
    } finally {
        events.push("finally");
    }
}

function replaceCompletion() {
    try {
        return 1;
    } finally {
        return 2;
    }
}

__evrel.observe("exceptions", recover(), replaceCompletion(), events.join(","));
