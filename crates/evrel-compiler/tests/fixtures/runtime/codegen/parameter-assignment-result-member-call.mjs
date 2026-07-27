function run(iterator, factory) {
    return (iterator = factory()).next();
}

const result = run(undefined, () => ({
    next() {
        return 42;
    },
}));

__evrel.observe("parameter assignment result member call", result);
