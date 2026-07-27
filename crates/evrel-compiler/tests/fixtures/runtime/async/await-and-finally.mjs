const events = [];

async function run() {
    events.push("start");
    try {
        const value = await Promise.resolve(40);
        events.push("resumed");
        return value + 2;
    } finally {
        events.push("finally");
    }
}

const result = await run();
__evrel.observe("async", result, events.join(","));
