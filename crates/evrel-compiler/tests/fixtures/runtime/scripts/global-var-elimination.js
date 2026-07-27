var variable = 1;
function declared() {
    return this;
}

__evrel.observe(
    "sloppy top level",
    this === globalThis,
    globalThis.variable,
    globalThis.declared === declared,
    declared() === globalThis,
);
