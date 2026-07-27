const events = [];
class Example {
    constructor(first, second) {
        events.push("body");
        this.total = first + second;
    }
}
function argumentsList() {
    events.push("spread");
    return [20, 22];
}

const instance = new Example(...argumentsList());
__evrel.observe("construct spread", instance.total, events.join(","));
