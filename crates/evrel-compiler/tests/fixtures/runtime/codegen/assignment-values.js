(function () {
    const events = [];

    function makeTarget(initial) {
        return {
            _value: initial,
            get value() {
                events.push(`get:${this._value}`);
                return this._value;
            },
            set value(next) {
                events.push(`set:${next}`);
                this._value = next;
            },
        };
    }

    function base(target) {
        events.push("base");
        return target;
    }

    function key() {
        events.push("key");
        return "value";
    }

    function rhs(label, value) {
        events.push(label);
        return value;
    }

    const target = makeTarget(2);
    const compound = base(target)[key()] += rhs("rhs", 3);
    const postfix = target.value++;
    const prefix = ++target.value;
    __evrel.observe(
        "assignment forms",
        compound,
        postfix,
        prefix,
        target._value,
        events.join(","),
    );

    events.length = 0;
    target._value = 10;
    const loaded = target.value;
    const computed = loaded + rhs("fallback-rhs", 4);
    target.value = computed;
    __evrel.observe(
        "extra load use keeps primitive fallback",
        loaded,
        computed,
        target._value,
        events.join(","),
    );

    events.length = 0;
    target._value = 1;
    target.value += true
        ? (false ? rhs("unreached", 2) : rhs("nested", 3))
        : rhs("alternate", 4);
    __evrel.observe(
        "nested conditional compound fallback",
        target._value,
        events.join(","),
    );

    events.length = 0;
    target._value = 0;
    target.value ||= true
        ? (false ? rhs("logical-unreached", 2) : rhs("logical-nested", 6))
        : rhs("logical-alternate", 4);
    __evrel.observe(
        "nested conditional logical fallback",
        target._value,
        events.join(","),
    );

    events.length = 0;
    const andTarget = makeTarget(1);
    const andResult = (andTarget.value &&= rhs("and-rhs", 7));
    const orTarget = makeTarget(1);
    const orResult = (orTarget.value ||= rhs("or-unreached", 8));
    const nullishTarget = makeTarget(null);
    const nullishResult = (nullishTarget.value ??= rhs("nullish-rhs", 9));
    __evrel.observe(
        "logical assignment values",
        andResult,
        andTarget._value,
        orResult,
        orTarget._value,
        nullishResult,
        nullishTarget._value,
        events.join(","),
    );

    events.length = 0;
    const coercingKey = {
        [Symbol.toPrimitive]() {
            events.push("coerce-key");
            return "value";
        },
    };
    target._value = 3;
    const keyedPostfix = target[coercingKey]++;
    __evrel.observe(
        "update converts a computed key once",
        keyedPostfix,
        target._value,
        events.join(","),
    );

    class PrivateCounter {
        #value = 4;

        update() {
            const old = this.#value++;
            const added = (this.#value += 2);
            return [old, added, this.#value];
        }
    }

    const [privateOld, privateAdded, privateValue] = new PrivateCounter().update();
    __evrel.observe("private assignment reconstruction", privateOld, privateAdded, privateValue);
})();
