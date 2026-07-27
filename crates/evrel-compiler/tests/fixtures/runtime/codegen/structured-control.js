(function () {
    const events = [];

    function mark(label, value) {
        events.push(label);
        return value;
    }

    const nested = mark("condition-a", false)
        ? mark("unreached-a", 1)
        : mark("condition-b", true)
          ? mark("result-b", 2)
          : mark("unreached-b", 3);
    const logical =
        (mark("and-left", true) && mark("and-right", "and")) ||
        mark("or-unreached", "or");
    const coalesced = mark("nullish-left", null) ?? mark("nullish-right", "value");
    __evrel.observe(
        "nested expression control",
        nested,
        logical,
        coalesced,
        events.join(","),
    );

    events.length = 0;
    let total = 0;
    outer: for (let index = 0; index < 4; index++) {
        let inner = 0;
        while (inner < 3) {
            inner++;
            if (index === 1 && inner === 2) {
                continue outer;
            }
            if (index === 3) {
                break outer;
            }
            total += index * 10 + inner;
        }
    }
    __evrel.observe("loop control", total, events.join(","));

    events.length = 0;
    function choose(value) {
        switch (mark("switch", value)) {
            case 1:
                events.push("one");
            case 2:
                events.push("two");
                return "matched";
            default:
                events.push("default");
                return "default";
        }
    }
    const first = choose(1);
    const second = choose(3);
    __evrel.observe("switch control", first, second, events.join(","));

    events.length = 0;
    function completion(shouldThrow) {
        try {
            events.push("try");
            if (shouldThrow) {
                throw 5;
            }
            return "return";
        } catch (error) {
            events.push(`catch:${error}`);
            return "catch";
        } finally {
            events.push("finally");
        }
    }
    const normal = completion(false);
    const thrown = completion(true);
    __evrel.observe("try reconstruction", normal, thrown, events.join(","));
})();
