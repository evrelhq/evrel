const events = [];
const target = {
    _value: 1,
    get value() {
        events.push("get");
        return this._value;
    },
    set value(next) {
        events.push("set");
        this._value = next;
    },
};
const proxy = new Proxy(target, {
    get(object, property, receiver) {
        events.push(`proxy-get:${String(property)}`);
        return Reflect.get(object, property, receiver);
    },
    set(object, property, value, receiver) {
        events.push(`proxy-set:${String(property)}`);
        return Reflect.set(object, property, value, receiver);
    },
    deleteProperty(object, property) {
        events.push(`proxy-delete:${String(property)}`);
        return Reflect.deleteProperty(object, property);
    },
});

const before = proxy.value;
proxy.value = before + 2;
const after = proxy.value;
const deleted = delete proxy.value;

__evrel.observe("accessors and proxies", before, after, deleted, events.join(","));
