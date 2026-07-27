const events = [];
const target = { own: 1 };
const proxy = new Proxy(target, {
    has(object, key) {
        events.push(`has:${String(key)}`);
        return Reflect.has(object, key);
    },
    ownKeys(object) {
        events.push("ownKeys");
        return Reflect.ownKeys(object);
    },
    getOwnPropertyDescriptor(object, key) {
        events.push(`descriptor:${String(key)}`);
        return Reflect.getOwnPropertyDescriptor(object, key);
    },
});

const has = "own" in proxy;
const keys = Object.keys(proxy);
__evrel.observe("proxy property traps", has, keys.join(","), events.join(","));
