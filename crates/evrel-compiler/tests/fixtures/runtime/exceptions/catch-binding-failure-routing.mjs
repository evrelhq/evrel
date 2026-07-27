const events = [];

function failBinding() {
    events.push("binding-default");
    throw "binding-error";
}

try {
    try {
        throw {};
    } catch ({ missing = failBinding() }) {
        events.push("catch-body");
    } finally {
        events.push("finally");
    }
} catch (error) {
    events.push(`outer:${error}`);
}

__evrel.observe("catch binding failure", events.join(","));
