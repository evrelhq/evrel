function assign() {
    implicitRuntimeFixtureGlobal = 42;
}

assign();
__evrel.observe(
    "sloppy implicit global",
    implicitRuntimeFixtureGlobal,
    globalThis.implicitRuntimeFixtureGlobal,
    delete implicitRuntimeFixtureGlobal,
    typeof implicitRuntimeFixtureGlobal,
);
