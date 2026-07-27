class Base {
    static value = 40;
    static read() {
        return this.value;
    }
}

class Derived extends Base {
    static value = 41;
    static read() {
        return super.read() + 1;
    }
}

__evrel.observe("static super", Derived.read(), Base.read());
