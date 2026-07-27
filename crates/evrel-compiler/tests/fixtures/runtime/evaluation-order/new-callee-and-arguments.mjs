const events = [];
class Constructor {
    constructor(first, second) {
        events.push("body");
        this.total = first + second;
    }
}
function callee() {
    events.push("callee");
    return Constructor;
}
function argument(name, value) {
    events.push(name);
    return value;
}

const instance = new (callee())(argument("first", 20), argument("second", 22));
__evrel.observe("new evaluation order", instance.total, events.join(","));
