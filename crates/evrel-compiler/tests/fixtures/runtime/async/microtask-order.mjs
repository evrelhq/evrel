const events = [];

async function run() {
    events.push("async:start");
    await null;
    events.push("async:resume");
}

events.push("script:start");
const promise = run();
Promise.resolve().then(() => events.push("then"));
events.push("script:end");
await promise;
await Promise.resolve();

__evrel.observe("microtask order", events.join(","));
