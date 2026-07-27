const events = [];
async function run() {
    try {
        events.push("try");
        await Promise.reject(new TypeError("rejected"));
        events.push("unreachable");
    } catch (error) {
        events.push(`catch:${error.name}`);
        return 41;
    } finally {
        await Promise.resolve();
        events.push("finally");
    }
}

const result = await run();
__evrel.observe("async rejection", result, events.join(","));
