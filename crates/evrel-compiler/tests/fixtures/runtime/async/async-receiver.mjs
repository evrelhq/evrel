const object = {
    value: 40,
    async read(offset) {
        await null;
        return this.value + offset;
    },
};

const result = await object.read(2);
__evrel.observe("async receiver", result);
