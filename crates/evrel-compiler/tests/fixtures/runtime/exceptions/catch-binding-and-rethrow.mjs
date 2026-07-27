const events = [];

function inner() {
    try {
        throw { code: 41 };
    } catch ({ code }) {
        events.push(`inner:${code}`);
        throw code + 1;
    }
}

try {
    inner();
} catch (value) {
    events.push(`outer:${value}`);
}

try {
    throw "ignored";
} catch {
    events.push("optional-binding");
}

__evrel.observe("catch and rethrow", events.join(","));
