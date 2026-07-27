const events = [];
function thrown() {
    events.push("evaluate-throw");
    return {
        get value() {
            events.push("get-value");
            return 42;
        },
    };
}

let caught;
try {
    throw thrown();
} catch (error) {
    events.push("catch");
    caught = error.value;
}

__evrel.observe("throw order", caught, events.join(","));
