const events = [];
const receiver = {};
const method = function (value) {
    events.push(this === receiver ? "receiver" : "wrong-receiver");
    return value;
};

Object.defineProperty(method, "call", {
    get() {
        events.push("call-property");
        return Function.prototype.call;
    },
});

Object.defineProperty(receiver, "method", {
    get() {
        events.push("method");
        return method;
    },
});

function key() {
    events.push("key");
    return "method";
}

function argument() {
    events.push("argument");
    return 42;
}

const staticResult = receiver?.method(argument());
const computedResult = receiver?.[key()](argument());
const missingResult = null?.[key()](argument());

__evrel.observe(
    "optional chain body method call",
    staticResult,
    computedResult,
    missingResult,
    events.join(","),
);
