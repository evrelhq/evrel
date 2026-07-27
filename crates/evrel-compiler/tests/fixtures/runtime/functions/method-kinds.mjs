const object = {
    *generator(value) {
        yield value;
        return this;
    },
    async asynchronous(value) {
        await null;
        return this.value + value;
    },
    async *asyncGenerator(value) {
        yield await Promise.resolve(value);
        return this.value;
    },
    value: 40,
};

const generator = object.generator(1);
const generatorFirst = generator.next();
const generatorReturn = generator.next();
const asyncIterator = object.asyncGenerator(2);
const asyncFirst = await asyncIterator.next();
const asyncReturn = await asyncIterator.next();

__evrel.observe(
    "method kinds",
    generatorFirst.value,
    generatorReturn.value === object,
    await object.asynchronous(2),
    asyncFirst.value,
    asyncReturn.value,
);
