const events = [];
function key(name) {
    events.push(`key:${name}`);
    return name;
}

const object = {
    [key("method")](value) {
        events.push(`method:${value}`);
        return this;
    },
    get [key("value")]() {
        events.push("get");
        return 41;
    },
    set [key("setter")](value) {
        events.push(`set:${value}`);
    },
};

const receiver = object.method(1);
const value = object.value;
object.setter = 42;
__evrel.observe("computed methods", receiver === object, value, events.join(","));
