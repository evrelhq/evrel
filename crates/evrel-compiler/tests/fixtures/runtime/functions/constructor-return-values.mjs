function PrimitiveReturn(value) {
    this.value = value;
    return 1;
}

function ObjectReturn(value) {
    this.ignored = value;
    return { value };
}

const primitive = new PrimitiveReturn(41);
const object = new ObjectReturn(42);

__evrel.observe(
    "constructor return values",
    primitive.value,
    primitive instanceof PrimitiveReturn,
    object.value,
    object.ignored,
    object instanceof ObjectReturn,
);
