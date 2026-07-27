class Base {
    method() {
        return this?.marker;
    }
}

class Derived extends Base {
    marker = "derived";

    detached() {
        return (0, super.method)();
    }
}

__evrel.observe("super sequence call", new Derived().detached());
