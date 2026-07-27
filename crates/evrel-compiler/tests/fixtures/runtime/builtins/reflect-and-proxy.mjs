const events = [];
function Target(value) {
    this.value = value;
}
const proxy = new Proxy(Target, {
    apply(target, receiver, argumentsList) {
        events.push(`apply:${receiver.marker}:${argumentsList[0]}`);
        return Reflect.apply(target, receiver, argumentsList);
    },
    construct(target, argumentsList, newTarget) {
        events.push(`construct:${argumentsList[0]}:${newTarget === proxy}`);
        return Reflect.construct(target, argumentsList, newTarget);
    },
});

const receiver = { marker: "receiver" };
proxy.call(receiver, 41);
const instance = new proxy(42);
__evrel.observe(
    "reflect and proxy",
    receiver.value,
    instance.value,
    instance instanceof Target,
    events.join(","),
);
