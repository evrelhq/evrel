const events = [];
class Base {
    static value = 40;
}

function heritage() {
    events.push("heritage");
    return Base;
}

class Derived extends heritage() {
    static first = (events.push("field"), super.value + 1);
    static {
        events.push("block");
        this.second = this.first + 1;
    }
}

__evrel.observe("extends expression", Derived.first, Derived.second, events.join(","));
