function run(generatorFactory) {
    return new Promise((resolve, reject) => {
        function advance(result) {
            if (result.done) {
                resolve(result.value);
            } else {
                Promise.resolve(result.value).then(next, reject);
            }
        }

        function next(value) {
            try {
                advance(iterator.next(value));
            } catch (error) {
                reject(error);
            }
        }

        let iterator;
        advance((iterator = generatorFactory()).next());
    });
}

const result = await run(function* () {
    yield 1;
    return 42;
});

__evrel.observe("assignment result member call", result);
