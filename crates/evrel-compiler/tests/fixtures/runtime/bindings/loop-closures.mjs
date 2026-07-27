const callbacks = [];
for (let index = 0; index < 3; index++) {
    callbacks.push(() => index);
}

__evrel.observe(
    "per-iteration bindings",
    callbacks[0](),
    callbacks[1](),
    callbacks[2](),
);
