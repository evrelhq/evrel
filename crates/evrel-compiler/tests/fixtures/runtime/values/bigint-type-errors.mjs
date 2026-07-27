let mixedName;
let unsignedName;
try {
    void (1n + 1);
} catch (error) {
    mixedName = error.name;
}
try {
    void (1n >>> 1n);
} catch (error) {
    unsignedName = error.name;
}

__evrel.observe("bigint type errors", mixedName, unsignedName);
