const object = { removable: 1 };
const missingType = typeof missingRuntimeGlobal;
const removed = delete object.removable;
const discarded = void (() => 42)();

__evrel.observe(
    "unary operations",
    +"12",
    -"12",
    !0,
    !!"value",
    ~5,
    missingType,
    removed,
    "removable" in object,
    discarded,
    typeof 1n,
    typeof Symbol(),
);
