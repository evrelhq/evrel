const events = [];
class Base {
    constructor(value) {
        events.push("base");
        this.value = value;
    }
}

class Derived extends Base {
    field = (events.push("field"), this.value + 1);
    constructor(value) {
        events.push("before-super");
        super(value);
        events.push("after-super");
    }
}

const instance = new Derived(41);
__evrel.observe(
    "derived construction",
    instance.value,
    instance.field,
    instance instanceof Base,
    instance instanceof Derived,
    events.join(","),
);
