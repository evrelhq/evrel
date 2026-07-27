class Counter {
    #value = 0;

    get #current() {
        return this.#value;
    }

    set #current(value) {
        this.#value = value;
    }

    #increment(amount) {
        this.#current = this.#current + amount;
        return this.#current;
    }

    increment(amount) {
        return this.#increment(amount);
    }
}

const counter = new Counter();
__evrel.observe("private elements", counter.increment(2), counter.increment(3));
