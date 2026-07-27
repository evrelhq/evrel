const events = [];
function key(name) {
    events.push(`key:${name}`);
    return name;
}
function initial(name, value) {
    events.push(`initial:${name}`);
    return value;
}

class Example {
    [key("instance")] = initial("instance", 1);
    static [key("static")] = initial("static", 2);
    [key("method")]() {
        return 3;
    }
}

events.push("before-new");
const instance = new Example();
__evrel.observe(
    "computed class elements",
    instance.instance,
    Example.static,
    instance.method(),
    events.join(","),
);
