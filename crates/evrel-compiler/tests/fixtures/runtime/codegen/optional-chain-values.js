(function () {
    const events = [];
    const receiver = {
        value: 40,
        get method() {
            events.push("get-method");
            return callable;
        },
    };
    const callable = function (offset) {
        events.push(this === receiver ? `call:${offset}` : "wrong-receiver");
        return this.value + offset;
    };

    Object.defineProperty(callable, "call", {
        get() {
            events.push("observable-call-property");
            return Function.prototype.call;
        },
    });

    function load(value, label) {
        events.push(label);
        return value;
    }

    function key() {
        events.push("key");
        return "method";
    }

    function argument() {
        events.push("argument");
        return 2;
    }

    const objectCall = load(receiver, "load-object")?.method(argument());
    const optionalCall = receiver.method?.(argument());
    const computedCall = receiver?.[key()](argument());
    const missing = load(null, "load-null")?.[key()](argument());
    __evrel.observe(
        "receiver-preserving optional calls",
        objectCall,
        optionalCall,
        computedCall,
        missing,
        events.join(","),
    );

    events.length = 0;
    const retainedReceiver = load(receiver, "retained-load");
    const retainedResult = retainedReceiver?.method(argument());
    __evrel.observe(
        "receiver remains usable after chain",
        retainedResult,
        retainedReceiver === receiver,
        retainedReceiver.value,
        events.join(","),
    );

    events.length = 0;
    function nested(value, condition) {
        return condition
            ? value?.method?.(argument())
            : value?.missing?.(argument());
    }
    const nestedPresent = nested(receiver, true);
    const nestedMissing = nested(null, false);
    __evrel.observe(
        "optional chains nested in conditional regions",
        nestedPresent,
        nestedMissing,
        events.join(","),
    );

    events.length = 0;
    let discarded = 0;
    receiver.method?.(discarded++);
    null?.method?.(discarded++);
    __evrel.observe("discarded optional chains", discarded, events.join(","));

    events.length = 0;
    function read(value) {
        const count = value?.count ?? 0;
        if (count > 0) {
            return value?.items?.() ?? [];
        }
        return [];
    }
    const presentItems = read({
        count: 1,
        items() {
            events.push("items");
            return [1, 2];
        },
    });
    const missingItems = read(null);
    __evrel.observe(
        "optional predicates with later control flow",
        presentItems.length,
        presentItems[0],
        missingItems.length,
        events.join(","),
    );

    events.length = 0;
    function iterate(value) {
        const result = [];
        for (const item of value?.items ?? ["fallback"]) {
            result.push(item);
        }
        return result;
    }
    const iterated = iterate({ items: [1, 2] });
    const missingIteration = iterate(null);
    __evrel.observe(
        "optional chains in iterator headers",
        iterated.length,
        missingIteration.length,
        missingIteration[0],
        events.join(","),
    );
})();
