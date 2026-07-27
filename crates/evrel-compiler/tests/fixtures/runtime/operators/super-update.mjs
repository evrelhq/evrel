const events = [];
class Base {
    get value() {
        events.push(`get:${this.stored}`);
        return this.stored;
    }
    set value(next) {
        events.push(`set:${next}`);
        this.stored = next;
    }
}
class Derived extends Base {
    decrement() {
        const postfix = super.value--;
        const prefix = --super.value;
        return [postfix, prefix];
    }
}

const instance = new Derived();
instance.stored = 4;
const result = instance.decrement();
__evrel.observe("super update", result[0], result[1], instance.stored, events.join(","));
