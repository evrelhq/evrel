class Base {}
class ReturnsObject extends Base {
    constructor() {
        return { marker: 42 };
    }
}
class ReturnsUndefined extends Base {
    constructor() {
        super();
        return undefined;
    }
}

const object = new ReturnsObject();
const normal = new ReturnsUndefined();
__evrel.observe(
    "derived return",
    object.marker,
    object instanceof ReturnsObject,
    normal instanceof ReturnsUndefined,
    normal instanceof Base,
);
