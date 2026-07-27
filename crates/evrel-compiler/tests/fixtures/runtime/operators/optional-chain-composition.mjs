const object = {
    value: 40,
    method(offset) {
        return this.value + offset;
    },
};

__evrel.observe(
    "optional chains",
    object?.value,
    object?.method?.(2),
    null?.value,
    null?.method?.(2),
);

function readSettings(options) {
    const settings = {
        normalize: false,
        propertyMap: null,
        skipValidation: false,
        ...options,
    };
    const mapped = settings.propertyMap?.get("value");

    return [
        mapped,
        settings.normalize,
        settings.propertyMap,
        settings.skipValidation,
    ];
}

__evrel.observe(
    "optional receiver reused after its chain",
    readSettings({
        normalize: true,
        propertyMap: new Map([["value", 42]]),
    }),
);
