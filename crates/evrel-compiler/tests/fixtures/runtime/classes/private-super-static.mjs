const events = [];

class Base {
    constructor(value) {
        this.value = value;
    }

    read() {
        return this.value;
    }
}

class Derived extends Base {
    static initialized = 1;
    static {
        this.initialized += 1;
        events.push("static");
    }

    #offset = 2;

    read() {
        return super.read() + this.#offset;
    }

    hasOffset(value) {
        return #offset in value;
    }
}

const instance = new Derived(40);
__evrel.observe(
    "classes",
    instance.read(),
    instance.hasOffset(instance),
    instance.hasOffset({}),
    Derived.initialized,
    events.join(","),
);
